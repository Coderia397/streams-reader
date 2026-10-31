"""Kokoro-82M on CPU with the controls the director mapping needs, driven exactly as the Space drives it
(space/nolabs_engine/engines/kokoro_engine.py: pinned snapshot, KPipeline G2P, one model call per item text).

Controls on top of the stock call:
- speed: the stock `speed` argument (whole call).
- voice blend: the style pack is a weighted mean of voice packs ([510, 1, 256] tensors, indexed by phoneme count).
- per-word duration scale and extra frames before a word: applied to the predicted durations before alignment, so
  the model itself renders the longer word / the gap (no audio edit). One frame = 600 samples = 25 ms.
- per-word F0 shift in semitones: applied to the predicted F0 curve before the decoder (no PSOLA).

The stock forward is `KModel.forward_with_tokens`; `_forward` below is that code with the three hooks added and is
identical to it when no control is set (checked by `selfcheck`).
"""
from __future__ import annotations

import os
from dataclasses import dataclass, field

import numpy as np
import torch

KOKORO_REPO = "hexgrad/Kokoro-82M"
KOKORO_COMMIT = "f3ff3571791e39611d31c381e3a41a3af07b4987"   # notes/13, space/nolabs_engine/revisions.py
SR = 24_000
FRAME = 600   # samples per duration unit


@dataclass
class WordCtrl:
    dur: float = 1.0          # duration multiplier for the word's phonemes
    pre_frames: int = 0       # extra frames on the space before the word
    f0_st: float = 0.0        # F0 shift on the word


@dataclass
class Out:
    wave: np.ndarray
    # (start_s, end_s, word_idx) for every whitespace word of the call text that got phonemes
    words: list[tuple[float, float, int]] = field(default_factory=list)


EXTRA_LEXICON = {"a": {"Martha": "mˈɑɹθə"}, "b": {"Martha": "mˈɑːθə"}}


def _disable_espeak() -> None:
    """The espeak-ng fallback (out-of-dictionary words only) aborts the process on macOS x86_64 with espeakng-loader
    0.2.4 (its dylib looks for data under the CI build path). The test passages need no fallback; `synth` raises if a
    word comes back without phonemes, so a missing fallback can never pass silently."""
    import sys
    import types
    stub = types.ModuleType("misaki.espeak")

    class EspeakFallback:  # noqa: D401
        def __init__(self, *a, **k):
            raise RuntimeError("espeak fallback disabled")
    stub.EspeakFallback = EspeakFallback
    sys.modules["misaki.espeak"] = stub
    import misaki
    misaki.espeak = stub


def words_of(text: str) -> list[tuple[int, int]]:
    import re
    return [(m.start(), m.end()) for m in re.finditer(r"\S+", text)]


class Engine:
    def __init__(self, weights: str):
        _disable_espeak()
        from kokoro import KModel, KPipeline
        torch.set_num_threads(int(os.environ.get("KOKORO_THREADS", "4")))
        self.weights = weights
        self.model = KModel(repo_id=KOKORO_REPO, config=os.path.join(weights, "config.json"),
                            model=os.path.join(weights, "kokoro-v1_0.pth")).to("cpu").eval()
        self.g2p = {c: KPipeline(lang_code=c, repo_id=KOKORO_REPO, model=False) for c in "ab"}
        # Out-of-dictionary names in the test passages, phonemised by hand in misaki's notation in place of espeak.
        for c, pipe in self.g2p.items():
            pipe.g2p.lexicon.golds.update(EXTRA_LEXICON[c])
        self._packs: dict[str, torch.Tensor] = {}

    def pack(self, voice: str) -> torch.Tensor:
        if voice not in self._packs:
            self._packs[voice] = torch.load(os.path.join(self.weights, "voices", f"{voice}.pt"), weights_only=True)
        return self._packs[voice]

    def blend(self, mix: dict[str, float]) -> torch.Tensor:
        total = sum(mix.values())
        return sum(self.pack(v) * (w / total) for v, w in mix.items())

    # -------------------------------------------------------------------------------------------------- synthesis
    def synth(self, text: str, mix: dict[str, float], lang: str = "a", speed: float = 1.0,
              ctrl: dict[int, WordCtrl] | None = None) -> Out:
        from kokoro import KPipeline
        ctrl = ctrl or {}
        pipe = self.g2p[lang]
        _, tokens = pipe.g2p(text)
        wspans = words_of(text)
        # token -> whitespace word index, by finding token text in the source left to right
        cursor, tok_word = 0, []
        for t in tokens:
            piece = (t.text or "").strip()
            at = text.find(piece, cursor) if piece else -1
            if at >= 0:
                cursor = at + len(piece)
                wi = next((i for i, (a, b) in enumerate(wspans) if a <= at < b), None)
            else:
                wi = None
            tok_word.append(wi)
        oov = [t.text for t in tokens if not t.phonemes and any(ch.isalnum() for ch in (t.text or ""))]
        if oov:
            raise RuntimeError(f"no phonemes for {oov} (espeak fallback disabled)")
        chunks = list(pipe.en_tokenize(tokens))
        assert len(chunks) == 1, f"text needs {len(chunks)} Kokoro chunks; keep calls under 510 phonemes"
        _, ps, tks = chunks[0]
        # phoneme char -> word index (tokens_to_ps joins phonemes + ' ' and strips the ends)
        char_word, space_before = [], {}
        for t, wi in zip(tokens, tok_word):
            ph = t.phonemes or ""
            char_word += [wi] * len(ph)
            if t.whitespace:
                char_word.append(("space", wi))
        lead = 0
        full = "".join((t.phonemes or "") + (" " if t.whitespace else "") for t in tokens)
        lead = len(full) - len(full.lstrip())
        char_word = char_word[lead:lead + len(ps)]
        vocab = self.model.vocab
        ids, id_word = [0], [None]
        for i, ch in enumerate(ps):
            if ch in vocab:
                ids.append(vocab[ch])
                id_word.append(char_word[i] if i < len(char_word) else None)
        ids.append(0)
        id_word.append(None)
        n = len(ids)
        assert n <= self.model.context_length
        scale = torch.ones(n)
        extra = torch.zeros(n)
        f0_st = torch.zeros(n)
        # the space id before word w is the last ("space", w_prev) id preceding w's first id
        first_id = {}
        for k, w in enumerate(id_word):
            if isinstance(w, int) and w not in first_id:
                first_id[w] = k
        for w, c in ctrl.items():
            for k, iw in enumerate(id_word):
                if iw == w:
                    scale[k] = c.dur
                    f0_st[k] = c.f0_st
            if c.pre_frames and w in first_id:
                k = first_id[w] - 1
                while k > 0 and not (isinstance(id_word[k], tuple)):
                    k -= 1
                if k > 0:
                    extra[k] += c.pre_frames
        ref = self.blend(mix)[len(ps) - 1]
        with torch.inference_mode():
            audio, pred_dur = self._forward(torch.LongTensor([ids]), ref, speed, scale, extra, f0_st)
        wave = audio.numpy().astype(np.float32).reshape(-1)
        # word timings from durations
        ends = np.cumsum(pred_dur.numpy())
        starts = ends - pred_dur.numpy()
        words = []
        for w in sorted(first_id):
            ks = [k for k, iw in enumerate(id_word) if iw == w]
            words.append((float(starts[ks[0]] * FRAME / SR), float(ends[ks[-1]] * FRAME / SR), w))
        return Out(wave=wave, words=words)

    def _forward(self, input_ids, ref_s, speed, scale, extra, f0_st):
        m = self.model
        input_lengths = torch.full((input_ids.shape[0],), input_ids.shape[-1], dtype=torch.long)
        text_mask = torch.arange(input_lengths.max()).unsqueeze(0).expand(input_lengths.shape[0], -1).type_as(input_lengths)
        text_mask = torch.gt(text_mask + 1, input_lengths.unsqueeze(1))
        bert_dur = m.bert(input_ids, attention_mask=(~text_mask).int())
        d_en = m.bert_encoder(bert_dur).transpose(-1, -2)
        s = ref_s[:, 128:]
        d = m.predictor.text_encoder(d_en, s, input_lengths, text_mask)
        x, _ = m.predictor.lstm(d)
        duration = m.predictor.duration_proj(x)
        duration = torch.sigmoid(duration).sum(axis=-1) / speed
        duration = duration * scale.unsqueeze(0) + extra.unsqueeze(0)          # hook 1: per-id durations
        pred_dur = torch.round(duration).clamp(min=1).long().squeeze()
        indices = torch.repeat_interleave(torch.arange(input_ids.shape[1]), pred_dur)
        pred_aln_trg = torch.zeros((input_ids.shape[1], indices.shape[0]))
        pred_aln_trg[indices, torch.arange(indices.shape[0])] = 1
        pred_aln_trg = pred_aln_trg.unsqueeze(0)
        en = d.transpose(-1, -2) @ pred_aln_trg
        F0_pred, N_pred = m.predictor.F0Ntrain(en, s)
        if torch.any(f0_st != 0):                                               # hook 2: per-id F0 shift
            per_frame = f0_st[indices]
            per_f0 = torch.repeat_interleave(per_frame, F0_pred.shape[-1] // per_frame.shape[0])
            per_f0 = torch.nn.functional.pad(per_f0, (0, F0_pred.shape[-1] - per_f0.shape[0]))
            # smooth the step so the shift glides over ~50 ms instead of jumping
            k = 9
            per_f0 = torch.nn.functional.avg_pool1d(per_f0.view(1, 1, -1), k, 1, k // 2,
                                                    count_include_pad=False).view(-1)
            voiced = F0_pred > 30
            F0_pred = torch.where(voiced, F0_pred * torch.pow(2.0, per_f0 / 12.0), F0_pred)
        t_en = m.text_encoder(input_ids, input_lengths, text_mask)
        asr = t_en @ pred_aln_trg
        audio = m.decoder(asr, F0_pred, N_pred, ref_s[:, :128]).squeeze()
        return audio, pred_dur

    def selfcheck(self, text: str = "The door handle began, very slowly, to turn.") -> float:
        """Max abs difference between `_forward` without controls and the stock forward."""
        pipe = self.g2p["a"]
        _, tokens = pipe.g2p(text)
        _, ps, _ = next(pipe.en_tokenize(tokens))
        ref = self.pack("af_heart")[len(ps) - 1]
        ids = [0] + [self.model.vocab[c] for c in ps if c in self.model.vocab] + [0]
        n = len(ids)
        with torch.inference_mode():
            torch.manual_seed(0)
            a, _ = self._forward(torch.LongTensor([ids]), ref, 1.0, torch.ones(n), torch.zeros(n), torch.zeros(n))
            torch.manual_seed(0)
            b = self.model(ps, ref, 1.0)
        return float((a - b).abs().max())
