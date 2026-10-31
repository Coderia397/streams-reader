"""Kokoro-82M on the Space CPU: the standard narrator. It spends no ZeroGPU quota, so it can read whole books.

Unknown words are phonemised by the BART fallback (g2p_fallback.py) instead of espeak-ng. Weights and voices load from
the pinned snapshot (revisions.py) by file path, because `KModel`/`KPipeline` would otherwise fetch the repo's latest
commit through `hf_hub_download`.
"""
from __future__ import annotations

import os
import threading
from dataclasses import dataclass

import numpy as np
import torch

from .. import revisions
from ..voices import KOKORO_VOICES, kokoro_lang_code
from .g2p_fallback import BartFallback
from .timing import Word, place_tokens

SAMPLE_RATE = 24_000


@dataclass(frozen=True)
class Synthesis:
    wave: np.ndarray
    sample_rate: int
    # Relative to the untrimmed wave.
    words: tuple[Word, ...]


class KokoroEngine:
    sample_rate = SAMPLE_RATE

    def __init__(self) -> None:
        from kokoro import KModel, KPipeline

        root = revisions.snapshot(revisions.KOKORO_REPO, revisions.KOKORO_COMMIT, revisions.KOKORO_FILES)
        self._model = KModel(
            repo_id=revisions.KOKORO_REPO,
            config=os.path.join(root, "config.json"),
            model=os.path.join(root, revisions.KOKORO_MODEL_FILE),
        ).to("cpu").eval()
        self._pipelines = {code: KPipeline(lang_code=code, repo_id=revisions.KOKORO_REPO, model=self._model)
                           for code in "ab"}
        for code, pipeline in self._pipelines.items():
            # Out-of-lexicon words go to the BART network, not espeak-ng (D32): same pronunciations as the apps.
            pipeline.g2p.fallback = BartFallback(revisions.snapshot(*revisions.G2P_REPOS[code], revisions.G2P_FILES))
        self._voice_dir = os.path.join(root, "voices")
        # KPipeline caches voices in a plain dict and the model is not re-entrant; one synthesis at a time.
        self._lock = threading.Lock()

    def synthesize(self, text: str, voice: str) -> Synthesis:
        if voice not in KOKORO_VOICES:
            raise ValueError(f"unknown Kokoro voice {voice!r}")
        pipeline = self._pipelines[kokoro_lang_code(voice)]
        voice_path = os.path.join(self._voice_dir, f"{voice}.pt")
        waves: list[np.ndarray] = []
        words: list[Word] = []
        offset_s, cursor = 0.0, 0
        with self._lock, torch.inference_mode():
            # split_pattern=None keeps the text whole and unstripped, so token offsets index the exact item text.
            for result in pipeline(text, voice=voice_path, speed=1, split_pattern=None):
                if result.audio is None:
                    continue
                chunk = result.audio.detach().cpu().numpy().astype(np.float32).reshape(-1)
                cursor = place_tokens(text, result.tokens or (), offset_s, cursor, words)
                waves.append(chunk)
                offset_s += chunk.size / SAMPLE_RATE
        if not waves:
            raise RuntimeError("Kokoro produced no audio")
        return Synthesis(wave=np.concatenate(waves), sample_rate=SAMPLE_RATE, words=tuple(words))
