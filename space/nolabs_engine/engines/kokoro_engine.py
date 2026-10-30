"""Kokoro-82M on the Space CPU: the standard narrator. It spends no ZeroGPU quota, so it can read whole books."""
from __future__ import annotations

import os
import threading
from dataclasses import dataclass

import numpy as np
import torch

from ..contracts import WordTiming

REPO = "hexgrad/Kokoro-82M"
SAMPLE_RATE = 24_000


@dataclass(frozen=True)
class PresetVoice:
    id: str
    name: str
    gender: str
    accent: str
    grade: str


# English voices from hexgrad/Kokoro-82M VOICES.md with the authors' quality grades.
VOICES: tuple[PresetVoice, ...] = tuple(
    PresetVoice(f"{prefix}_{name.lower()}", name, gender, accent, grade)
    for prefix, gender, accent, entries in (
        ("af", "female", "American", (("Heart", "A"), ("Bella", "A-"), ("Nicole", "B-"), ("Aoede", "C+"), ("Kore", "C+"),
                                       ("Sarah", "C+"), ("Alloy", "C"), ("Nova", "C"), ("Sky", "C-"), ("Jessica", "D"),
                                       ("River", "D"))),
        ("am", "male", "American", (("Fenrir", "C+"), ("Michael", "C+"), ("Puck", "C+"), ("Echo", "D"), ("Eric", "D"),
                                     ("Liam", "D"), ("Onyx", "D"), ("Santa", "D-"), ("Adam", "F+"))),
        ("bf", "female", "British", (("Emma", "B-"), ("Isabella", "C"), ("Alice", "D"), ("Lily", "D"))),
        ("bm", "male", "British", (("George", "C"), ("Fable", "C"), ("Lewis", "D+"), ("Daniel", "D"))),
    )
    for name, grade in entries
)
VOICE_IDS = frozenset(v.id for v in VOICES)


@dataclass
class Synthesis:
    wave: np.ndarray
    words: list[WordTiming]


class KokoroEngine:
    def __init__(self) -> None:
        from kokoro import KModel, KPipeline

        torch.set_num_threads(max(1, os.cpu_count() or 1))
        self._model = KModel(repo_id=REPO).to("cpu").eval()
        self._pipelines = {code: KPipeline(lang_code=code, repo_id=REPO, model=self._model) for code in "ab"}
        self._lock = threading.Lock()

    def synthesize(self, text: str, voice: str, speed: float) -> Synthesis:
        if voice not in VOICE_IDS:
            raise ValueError(f"unknown Kokoro voice {voice!r}")
        pipeline = self._pipelines[voice[0]]
        waves: list[np.ndarray] = []
        words: list[WordTiming] = []
        offset_s, cursor = 0.0, 0
        with self._lock, torch.inference_mode():
            for result in pipeline(text, voice=voice, speed=speed, split_pattern=None):
                if result.audio is None:
                    continue
                audio = result.audio.detach().cpu().numpy().astype(np.float32).reshape(-1)
                for token in result.tokens or ():
                    cursor = _place(text, token, offset_s, cursor, words)
                waves.append(audio)
                offset_s += audio.size / SAMPLE_RATE
        if not waves:
            raise RuntimeError("Kokoro produced no audio")
        return Synthesis(wave=np.concatenate(waves), words=words)


def _place(text: str, token, offset_s: float, cursor: int, out: list[WordTiming]) -> int:
    """Map one timed token back onto the source text; tokens without timing or letters are skipped."""
    piece = (getattr(token, "text", "") or "").strip()
    start, end = getattr(token, "start_ts", None), getattr(token, "end_ts", None)
    if not piece or start is None or end is None or not any(ch.isalnum() for ch in piece):
        return cursor
    at = text.find(piece, cursor)
    if at < 0:
        return cursor
    out.append(WordTiming(
        start_ms=round((offset_s + start) * 1000), end_ms=round((offset_s + end) * 1000),
        char_start=at, char_end=at + len(piece),
    ))
    return at + len(piece)
