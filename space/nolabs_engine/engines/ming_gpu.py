"""Ming-3.0-tts-0.5B on ZeroGPU: model loading and the one `@spaces.GPU` function.

ZeroGPU rules shape this module: the model is built and moved to CUDA at import time (CUDA is emulated outside the
decorated function), all GPU work for one request happens in a single call because a free account gets few runs and
5 GPU-minutes a day, and the call returns float32 PCM only, so encoding and uploads never hold the GPU.
"""
from __future__ import annotations

import os
import sys
import tempfile
import time

import numpy as np
import soundfile as sf
import spaces
import torch

from .. import revisions
from .ming import GpuResult, MingEngine
from .ming_plan import ANCHOR_TEXT, GpuPolicy, Job, request_seconds

VENDOR = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "vendor", "ming3")
if VENDOR not in sys.path:
    sys.path.insert(0, VENDOR)

from ming_tts import MingTTS  # noqa: E402

SEED = 1234
POLICY = GpuPolicy.from_env()
_tts: MingTTS | None = None


def load() -> MingEngine:
    """Build the model on CUDA at import time of app.py and return the engine the service uses."""
    global _tts
    root = revisions.snapshot(revisions.MING_REPO, revisions.MING_COMMIT, revisions.MING_FILES)
    _tts = MingTTS(root, device="cuda", text_normalization=True, cache_prompts=True)
    try:
        _tts.normalizer  # noqa: B018 - load pynini now, not during the first GPU call
    except Exception as e:  # noqa: BLE001 - keep serving without number/date normalisation
        print(f"Ming text normalisation disabled: {e!r}", flush=True)
        _tts.text_normalization = False
    return MingEngine(run, _tts.sample_rate, POLICY)


def _duration(jobs: list[Job]) -> int:
    return request_seconds(POLICY, jobs)


@spaces.GPU(duration=_duration)
def run(jobs: list[Job]) -> GpuResult:
    """Synthesise every job in one GPU call. A failing job becomes an error string so the rest of the batch survives;
    anchors needed by EMOTION jobs on presets are rendered first and returned so the caller can cache them."""
    started = time.monotonic()
    anchors: dict[str, np.ndarray] = {}
    anchor_paths: dict[str, str] = {}
    waves: list[np.ndarray | str] = []
    for job in jobs:
        try:
            prompt = job.prompt
            voice_id = job.anchor_voice
            if voice_id is not None:
                if voice_id not in anchor_paths:
                    anchors[voice_id] = _synthesize(Job("IP", ANCHOR_TEXT, {"ip": voice_id}), "")
                    anchor_paths[voice_id] = _write_temp_wav(voice_id, anchors[voice_id])
                prompt = anchor_paths[voice_id]
            waves.append(_synthesize(job, prompt))
        except Exception as e:  # noqa: BLE001 - one bad item must not lose the rest of the batch
            waves.append(f"{type(e).__name__}: {e}")
    return GpuResult(waves=waves, anchors=anchors, gpu_s=time.monotonic() - started)


def _write_temp_wav(voice_id: str, wave: np.ndarray) -> str:
    if _tts is None:
        raise RuntimeError("Ming is not loaded")
    handle, path = tempfile.mkstemp(prefix=f"anchor-{voice_id}-", suffix=".wav")
    os.close(handle)
    sf.write(path, wave, _tts.sample_rate)
    return path


def _synthesize(job: Job, prompt: str) -> np.ndarray:
    if _tts is None:
        raise RuntimeError("Ming is not loaded")
    kwargs: dict = {"task": job.task, "text": job.text}
    if job.task == "IP":
        kwargs.update(controls=job.controls, use_zero_spk_emb=True)
    elif job.task == "EMOTION":
        kwargs.update(controls=job.controls, prompt_wav_path=prompt, use_spk_emb=True)
    elif job.task == "TTS":
        kwargs.update(language_id="en", prompt_wav_path=prompt, prompt_text=job.prompt_text, use_spk_emb=True)
    else:
        raise ValueError(f"unsupported Ming task {job.task!r}")
    torch.manual_seed(SEED)
    wave = _tts.synthesize(cfg=2.0, temperature=0.0, **kwargs)
    return wave.detach().float().cpu().reshape(-1).numpy().astype(np.float32)
