"""Out-of-lexicon fallback for Kokoro's English G2P: the Apache-2.0 BART grapheme-to-phoneme model, not espeak-ng.

misaki 0.9.4 (our pinned G2P) asks its `fallback` for any word missing from its lexicon. kokoro 0.9.4 plugs espeak-ng in
there; misaki's main branch replaced it with this network (`FallbackNetwork`, misaki commit 232a5e32, still unreleased)
and the native apps use the same model (D32, notes/19), so the Space and the apps pronounce rare words alike.

Adapted from misaki `en.py` at commit fba1236595f2d2bf21d414ba6e57d25256afada3 (Apache-2.0, Copyright hexgrad; licence text in LICENSE.misaki):
tokens are `[bos] + grapheme ids (unknown → 3) + [eos]`, greedy `generate`, ids above 3 map to phoneme characters.
Differences: weights load from a pinned local snapshot on the CPU, results are cached per word, and words past the
model's 64 positions are left to misaki's `unk` instead of raising.
"""
from __future__ import annotations

import functools
import threading
from typing import Protocol

# misaki calls `model.generate(input_ids=...)` with the repo's generation config, which sets no length, so the
# transformers default of 20 applies (bos + at most 18 phonemes + eos). Spelled out so a transformers upgrade cannot
# change pronunciations, and so the apps' ONNX greedy loop can stop at the same place.
MAX_LENGTH = 20
BOS, EOS, UNK = 1, 2, 3
CACHE_WORDS = 4096


class TokenLike(Protocol):
    text: str


class BartFallback:
    """misaki's fallback signature: token → (phonemes, rating), or (None, None) when it has no answer."""

    def __init__(self, model_dir: str):
        import torch
        from transformers import BartForConditionalGeneration

        self._torch = torch
        self.model = BartForConditionalGeneration.from_pretrained(model_dir).to("cpu").eval()
        config = self.model.config
        self._grapheme_ids = {g: i for i, g in enumerate(config.grapheme_chars)}
        self._phonemes = dict(enumerate(config.phoneme_chars))
        self._max_graphemes = int(config.max_position_embeddings) - 2
        # misaki already calls the fallback under KokoroEngine's lock; this guards direct use.
        self._lock = threading.Lock()

    def __call__(self, token: TokenLike) -> tuple[str | None, int | None]:
        phonemes = self.phonemize(token.text)
        return (phonemes, 1) if phonemes else (None, None)

    @functools.lru_cache(maxsize=CACHE_WORDS)  # noqa: B019 - one instance per language for the process lifetime
    def phonemize(self, word: str) -> str:
        if not word or len(word) > self._max_graphemes:
            return ""
        ids = [BOS, *(self._grapheme_ids.get(g, UNK) for g in word), EOS]
        with self._lock, self._torch.inference_mode():
            out = self.model.generate(input_ids=self._torch.tensor([ids]), max_length=MAX_LENGTH)
        return decode(out[0].tolist(), self._phonemes)


def decode(ids: list[int], phonemes: dict[int, str]) -> str:
    """Phoneme string from generated ids; special ids (pad, bos, eos, unk) carry no sound."""
    return "".join(phonemes.get(i, "") for i in ids if i > UNK)
