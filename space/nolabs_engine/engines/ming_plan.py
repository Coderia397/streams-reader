"""Planning a Ming batch on the CPU: which Ming task each item becomes, and how many GPU seconds to request.

Pure (no torch, no `spaces`), so it is unit tested and the GPU function receives only plain data.
"""
from __future__ import annotations

import math
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field

from ..contracts import EngineItem, EngineVoice, PresetVoice

# Ming's EMOTION task takes Chinese labels (vendor/ming3/tts_tasks.py EMOTION_VALUES).
EMOTION_LABELS: dict[str, str] = {
    "happy": "高兴", "sad": "悲伤", "angry": "愤怒", "fearful": "恐惧",
    "disgusted": "厌恶", "surprised": "惊讶", "neutral": "中性",
}
# Spoken once per preset with the IP task; the clip then serves as the reference for EMOTION on that preset, because
# the English presets have no emotion variants of their own.
ANCHOR_TEXT = ("I have always loved the quiet hour before dawn, when the streets are empty and the city seems to hold "
               "its breath, waiting for the day to begin.")
ANCHOR_PREFIX = "anchor:"


@dataclass(frozen=True)
class GpuPolicy:
    """GPU seconds requested per call: `base_s + s_per_char * chars`, rounded up and capped at `max_s`. Measured
    0.052 GPU-s per char (notes/08); 0.06 leaves 15 % margin. The `large` ZeroGPU size multiplies the request by 1.5
    at admission, so the cap stays well under the 300 s daily quota. Mirrors `nolabs_core::budget::GpuPolicy`."""
    base_s: float = 8.0
    s_per_char: float = 0.06
    max_s: int = 180

    @staticmethod
    def from_env(env: Mapping[str, str] | None = None) -> GpuPolicy:
        env = os.environ if env is None else env
        return GpuPolicy(
            base_s=float(env.get("MING_BASE_SECONDS", "8")),
            s_per_char=float(env.get("MING_SECONDS_PER_CHAR", "0.06")),
            max_s=int(env.get("MING_MAX_SECONDS", "180")),
        )

    def needed(self, chars: int) -> float:
        return self.base_s + self.s_per_char * chars

    def request(self, chars: int) -> int:
        return min(self.max_s, math.ceil(self.needed(chars)))


@dataclass
class Job:
    """One synthesis on the GPU. `prompt` is a local wav path, or `anchor:<preset>` for an anchor rendered in the
    same GPU call."""
    task: str
    text: str
    controls: dict = field(default_factory=dict)
    prompt: str = ""
    prompt_text: str = ""

    @property
    def anchor_voice(self) -> str | None:
        return self.prompt[len(ANCHOR_PREFIX):] if self.prompt.startswith(ANCHOR_PREFIX) else None


def plan(item: EngineItem, voice: EngineVoice, ref_path: str, anchors: Mapping[str, str]) -> Job:
    """The Ming task for one item. contracts.py has already checked that mode, voice and emotion fit together."""
    if item.mode == "ip" and isinstance(voice, PresetVoice):
        return Job("IP", item.text, {"ip": voice.id})
    if item.mode == "emotion":
        anchor = anchors.get(voice.id) or f"{ANCHOR_PREFIX}{voice.id}"
        prompt = anchor if isinstance(voice, PresetVoice) else ref_path
        return Job("EMOTION", item.text, {"emotion": EMOTION_LABELS[item.emotion]}, prompt=prompt)
    if item.mode == "tts" and not isinstance(voice, PresetVoice):
        return Job("TTS", item.text, prompt=ref_path, prompt_text=voice.transcript)
    raise ValueError(f"mode {item.mode!r} does not fit voice kind {voice.kind!r}")


def plan_batch(items: Sequence[EngineItem], voice: EngineVoice, ref_path: str,
               anchors: Mapping[str, str]) -> list[Job]:
    return [plan(item, voice, ref_path, anchors) for item in items]


def anchor_chars(jobs: Sequence[Job]) -> int:
    """Extra chars spoken in the call for anchors not cached yet (each preset's anchor is rendered once)."""
    return len(ANCHOR_TEXT) * len({j.anchor_voice for j in jobs if j.anchor_voice})


def item_chars(jobs: Sequence[Job]) -> int:
    return sum(len(j.text) for j in jobs)


def request_seconds(policy: GpuPolicy, jobs: Sequence[Job]) -> int:
    """Duration passed to `@spaces.GPU`: the items plus any anchors still to render, rounded up and capped."""
    return policy.request(item_chars(jobs) + anchor_chars(jobs))
