"""Ming-3.0-tts-0.5B on ZeroGPU: the expressive tier (cloned voices, emotion, paralinguistic tags, voice design).

ZeroGPU rules shape this module: the model is built and moved to CUDA at import time, and all GPU work for one request
happens inside a single `@spaces.GPU` call because a free account gets few runs and 5 GPU-minutes a day.
"""
from __future__ import annotations

import os
import sys
import tempfile
import time
from dataclasses import dataclass, field

import numpy as np
import spaces
import torch

VENDOR = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "vendor", "ming3")
if VENDOR not in sys.path:
    sys.path.insert(0, VENDOR)

from ming_tts import MingTTS  # noqa: E402

MODEL_ID = "inclusionAI/Ming-3.0-tts-0.5B"
PRESETS: dict[str, tuple[str, str, str]] = {
    # id: (name, gender, accent). The English built-in voices of Ming-3.0.
    "US_Emma": ("Emma", "female", "American"),
    "US_James": ("James", "male", "American"),
    "US_Stella": ("Stella", "female", "American"),
    "UK_George": ("George", "male", "British"),
    "UK_Charlotte": ("Charlotte", "female", "British"),
}
EMOTION_LABELS = {
    "happy": "高兴", "sad": "悲伤", "angry": "愤怒", "fearful": "恐惧",
    "disgusted": "厌恶", "surprised": "惊讶", "neutral": "中性",
}
ANCHOR_TEXT = ("I have always loved the quiet hour before dawn, when the streets are empty and the city seems to hold "
               "its breath, waiting for the day to begin.")

# GPU-seconds requested per call. On the default `large` size the scheduler multiplies this by 1.5 before checking it
# against the remaining quota, so a request must stay well under 300 s. Calibrated from measurements in notes/08.
BASE_SECONDS = float(os.environ.get("MING_BASE_SECONDS", "8"))
SECONDS_PER_CHAR = float(os.environ.get("MING_SECONDS_PER_CHAR", "0.06"))
MAX_SECONDS = int(os.environ.get("MING_MAX_SECONDS", "180"))

_tts: MingTTS | None = None
SAMPLE_RATE = 0


def load() -> None:
    """Build the model at import time, as ZeroGPU requires (CUDA is emulated outside @spaces.GPU)."""
    global _tts, SAMPLE_RATE
    from huggingface_hub import snapshot_download

    _tts = MingTTS(snapshot_download(MODEL_ID), device="cuda", text_normalization=True, cache_prompts=True)
    try:
        _tts.normalizer  # load pynini now, not on the first request
    except Exception as e:  # noqa: BLE001 - keep serving without number/date normalisation
        print(f"Ming text normalisation disabled: {e!r}", flush=True)
        _tts.text_normalization = False
    SAMPLE_RATE = _tts.sample_rate


@dataclass
class Job:
    """One synthesis on the GPU. `prompt` is a local wav path, or `anchor:<preset id>` for an anchor made in the same call."""
    task: str
    text: str
    controls: dict = field(default_factory=dict)
    prompt: str = ""
    prompt_text: str = ""


def plan(text: str, emotion: str, voice_kind: str, voice_id: str, ref_path: str, transcript: str) -> Job:
    """Choose the Ming task for one segment."""
    if emotion != "neutral":
        prompt = ref_path or f"anchor:{voice_id}"
        return Job("EMOTION", text, {"emotion": EMOTION_LABELS[emotion]}, prompt=prompt)
    if voice_kind == "preset":
        return Job("IP", text, {"ip": voice_id})
    return Job("TTS", text, prompt=ref_path, prompt_text=transcript)


def design_job(description: str, sample_text: str) -> Job:
    return Job("VOICE_DESIGN", sample_text, {"style": description})


def estimate_seconds(jobs: list[Job]) -> int:
    chars = sum(len(j.text) for j in jobs) + sum(len(ANCHOR_TEXT) for j in jobs if j.prompt.startswith("anchor:"))
    return int(min(MAX_SECONDS, BASE_SECONDS + SECONDS_PER_CHAR * chars))


def _duration(jobs: list[Job]) -> int:
    return estimate_seconds(jobs)


@spaces.GPU(duration=_duration)
def run(jobs: list[Job]) -> tuple[list[np.ndarray | str], dict[str, str], float]:
    """Synthesise every job in one GPU call. Returns waveforms (or an error string per job), anchors made, GPU seconds."""
    started = time.monotonic()
    anchors: dict[str, str] = {}
    out: list[np.ndarray | str] = []
    for job in jobs:
        try:
            prompt = job.prompt
            if prompt.startswith("anchor:"):
                voice_id = prompt.split(":", 1)[1]
                if voice_id not in anchors:
                    anchors[voice_id] = _render_anchor(voice_id)
                prompt = anchors[voice_id]
            out.append(_synthesize(job, prompt))
        except Exception as e:  # noqa: BLE001 - one bad segment must not lose the rest of the batch
            out.append(f"{type(e).__name__}: {e}")
    return out, anchors, time.monotonic() - started


def _render_anchor(voice_id: str) -> str:
    wave = _synthesize(Job("IP", ANCHOR_TEXT, {"ip": voice_id}), "")
    import soundfile as sf

    handle, path = tempfile.mkstemp(prefix=f"anchor-{voice_id}-", suffix=".wav")
    os.close(handle)
    sf.write(path, wave, SAMPLE_RATE)
    return path


def _synthesize(job: Job, prompt: str) -> np.ndarray:
    assert _tts is not None, "load() was not called"
    kwargs: dict = {"task": job.task, "text": job.text}
    if job.task == "IP":
        kwargs.update(controls=job.controls, use_zero_spk_emb=True)
    elif job.task == "VOICE_DESIGN":
        kwargs.update(controls=job.controls, use_zero_spk_emb=True)
    elif job.task == "EMOTION":
        kwargs.update(controls=job.controls, prompt_wav_path=prompt, use_spk_emb=True)
    else:
        kwargs.update(language_id="en", prompt_wav_path=prompt, prompt_text=job.prompt_text, use_spk_emb=True)
    torch.manual_seed(1234)
    wave = _tts.synthesize(cfg=2.0, temperature=0.0, **kwargs)
    return wave.detach().float().cpu().reshape(-1).numpy()
