"""The Ming engine as the service sees it: plan on the CPU, one GPU call per batch that streams each wave out as it is
done, anchors cached on the CPU side.

The GPU function itself lives in ming_gpu.py; it is injected here so this orchestration is testable without torch or
`spaces`. Exceptions from the GPU call (ZeroGPU quota, runs limit, illegal duration) are deliberately not caught: they
must reach the Worker with their original text.
"""
from __future__ import annotations

import contextlib
import hashlib
import io
import os
import tempfile
import threading
from collections.abc import Callable, Generator, Sequence
from dataclasses import dataclass

import numpy as np
import soundfile as sf

from ..contracts import EngineItem, EngineVoice
from ..errors import ContractError
from . import ming_plan
from .ming_plan import GpuPolicy, Job


# What the GPU generator yields, in this order: per job, the anchor it needed (if rendered in this call) and then its
# wave; finally the GPU seconds spent. Plain dataclasses, because ZeroGPU pickles them across its worker process.
@dataclass(frozen=True)
class GpuAnchor:
    voice_id: str
    wave: np.ndarray


@dataclass(frozen=True)
class GpuWave:
    index: int
    # float32 PCM, or the error text when this job failed.
    wave: np.ndarray | str


@dataclass(frozen=True)
class GpuDone:
    gpu_s: float


GpuEvent = GpuAnchor | GpuWave | GpuDone
# A generator, not any iterator: `close()` is how an early stop reaches the ZeroGPU worker.
GpuRun = Callable[[list[Job]], Generator[GpuEvent, None, None]]


@dataclass(frozen=True)
class MingClip:
    index: int
    wave: np.ndarray | str
    sample_rate: int


@dataclass(frozen=True)
class MingDone:
    gpu_s: float


class MingEngine:
    def __init__(self, gpu_run: GpuRun, sample_rate: int, policy: GpuPolicy | None = None,
                 anchor_dir: str | None = None):
        self._gpu_run = gpu_run
        self.sample_rate = sample_rate
        self.policy = policy or GpuPolicy.from_env()
        self._anchor_dir = anchor_dir or tempfile.mkdtemp(prefix="ming-anchors-")
        # preset id → wav path. Filled after each GPU call; ZeroGPU runs the call in a forked worker, so state set
        # inside it never reaches this process.
        self._anchors: dict[str, str] = {}
        self._lock = threading.Lock()

    @property
    def anchors(self) -> dict[str, str]:
        with self._lock:
            return dict(self._anchors)

    def render_stream(self, items: Sequence[EngineItem], voice: EngineVoice,
                      ref_path: str) -> Generator[MingClip | MingDone, None, None]:
        """Yield each item's wave as soon as the GPU call has it, in item order, then `MingDone`. The GPU call is
        still one call for the whole batch; encoding and uploads overlap it on the CPU side."""
        jobs = ming_plan.plan_batch(items, voice, ref_path, self.anchors)
        chars = ming_plan.item_chars(jobs)
        if self.policy.needed(chars) > self.policy.max_s:
            raise ContractError(
                f"batch of {chars} characters needs about {self.policy.needed(chars):.0f} GPU seconds, more than the "
                f"{self.policy.max_s} s one call may request; send fewer items")
        expected, done = 0, None
        # Closed explicitly: the spaces wrapper stops the GPU worker only when GeneratorExit reaches its generator.
        with contextlib.closing(self._gpu_run(jobs)) as run:
            for event in run:
                if isinstance(event, GpuAnchor):
                    self._save_anchor(event.voice_id, event.wave)
                elif isinstance(event, GpuWave):
                    if event.index != expected:
                        raise RuntimeError(f"GPU call returned item {event.index} where item {expected} was due")
                    expected += 1
                    yield MingClip(event.index, event.wave, self.sample_rate)
                elif isinstance(event, GpuDone):
                    done = event
        if expected != len(jobs) or done is None:
            raise RuntimeError(f"GPU call returned {expected} clips for {len(jobs)} items")
        yield MingDone(done.gpu_s)

    def _save_anchor(self, voice_id: str, wave: np.ndarray) -> None:
        buf = io.BytesIO()
        sf.write(buf, np.asarray(wave, dtype=np.float32).reshape(-1), self.sample_rate, format="WAV", subtype="FLOAT")
        data = buf.getvalue()
        # Content-addressed name: Ming caches prompt features by path, so a path must never change content.
        path = os.path.join(self._anchor_dir, f"{voice_id}-{hashlib.sha256(data).hexdigest()[:16]}.wav")
        with open(path, "wb") as f:
            f.write(data)
        with self._lock:
            self._anchors[voice_id] = path
