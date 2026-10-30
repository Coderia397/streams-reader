"""The Ming engine as the service sees it: plan on the CPU, one GPU call per batch, anchors cached on the CPU side.

The GPU function itself lives in ming_gpu.py; it is injected here so this orchestration is testable without torch or
`spaces`. Exceptions from the GPU call (ZeroGPU quota, runs limit, illegal duration) are deliberately not caught: they
must reach the Worker with their original text.
"""
from __future__ import annotations

import hashlib
import io
import os
import tempfile
import threading
from collections.abc import Callable, Sequence
from dataclasses import dataclass

import numpy as np
import soundfile as sf

from ..contracts import EngineItem, EngineVoice
from ..errors import ContractError
from . import ming_plan
from .ming_plan import GpuPolicy, Job


@dataclass(frozen=True)
class GpuResult:
    """What the GPU function returns: float32 PCM (or an error string) per job, PCM of anchors it rendered by preset
    id, and the GPU seconds spent inside the call."""
    waves: list[np.ndarray | str]
    anchors: dict[str, np.ndarray]
    gpu_s: float


@dataclass(frozen=True)
class MingOutput:
    waves: list[np.ndarray | str]
    sample_rate: int
    gpu_s: float


GpuRun = Callable[[list[Job]], GpuResult]


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

    def render(self, items: Sequence[EngineItem], voice: EngineVoice, ref_path: str) -> MingOutput:
        jobs = ming_plan.plan_batch(items, voice, ref_path, self.anchors)
        chars = ming_plan.item_chars(jobs)
        if self.policy.needed(chars) > self.policy.max_s:
            raise ContractError(
                f"batch of {chars} characters needs about {self.policy.needed(chars):.0f} GPU seconds, more than the "
                f"{self.policy.max_s} s one call may request; send fewer items")
        result = self._gpu_run(jobs)
        if len(result.waves) != len(jobs):
            raise RuntimeError(f"GPU call returned {len(result.waves)} clips for {len(jobs)} items")
        for voice_id, wave in result.anchors.items():
            self._save_anchor(voice_id, wave)
        return MingOutput(waves=list(result.waves), sample_rate=self.sample_rate, gpu_s=result.gpu_s)

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
