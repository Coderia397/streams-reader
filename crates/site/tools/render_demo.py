"""Render the landing page's hero demo: one passage per voice as MP3 plus word timings JSON.

Uses the timestamped ONNX export of Kokoro-82M (the same model the product's standard voices run on), which returns
the predicted duration of every phoneme token, so the word timings are the model's own, not an estimate.

Run (downloads about 330 MB of model files into --cache, which you can delete afterwards):
    uv run --python 3.12 --no-project --with onnxruntime==1.23.2 --with numpy --with soundfile --with phonemizer-fork \
        --with espeakng-loader python -I crates/site/tools/render_demo.py --cache <dir> --out crates/site/assets/demo
"""
from __future__ import annotations

import argparse
import json
import pathlib
import urllib.request

import espeakng_loader
import numpy as np
import onnxruntime as ort
import soundfile as sf
from phonemizer.backend.espeak.wrapper import EspeakWrapper
from phonemizer import phonemize

REPO = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX-timestamped/resolve/main/"
SAMPLE_RATE = 24_000
# The passage is public domain (Jane Austen, Pride and Prejudice, 1813).
TEXT = (
    "It is a truth universally acknowledged, that a single man in possession of a good fortune, "
    "must be in want of a wife."
)
VOICES = {"af_heart": "a", "bm_george": "b"}


def fetch(cache: pathlib.Path, rel: str) -> pathlib.Path:
    path = cache / rel
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        urllib.request.urlretrieve(REPO + rel, path)
    return path


def phonemes_per_word(words: list[str], british: bool) -> list[str]:
    lang = "en-gb" if british else "en-us"
    out = phonemize(words, language=lang, backend="espeak", preserve_punctuation=True, with_stress=True, strip=True)
    return [p.strip() for p in out]


def render(session: ort.InferenceSession, vocab: dict[str, int], voice_bin: pathlib.Path, british: bool):
    words = TEXT.split(" ")
    word_ph = phonemes_per_word(words, british)
    ids: list[int] = [0]
    spans: list[tuple[int, int]] = []
    for i, ph in enumerate(word_ph):
        if i:
            ids.append(vocab[" "])
        start = len(ids)
        ids.extend(vocab[c] for c in ph if c in vocab)
        spans.append((start, len(ids)))
    ids.append(0)
    styles = np.fromfile(voice_bin, dtype=np.float32).reshape(-1, 1, 256)
    style = styles[min(len(ids) - 2, len(styles) - 1)]
    names = [o.name for o in session.get_outputs()]
    result = dict(zip(names, session.run(None, {
        "input_ids": np.array([ids], dtype=np.int64),
        "style": style.astype(np.float32),
        "speed": np.array([1.0], dtype=np.float32),
    })))
    audio = result["waveform"].reshape(-1)
    durations = result["durations"].reshape(-1).astype(np.float64)
    # Durations are in model frames; scale so they cover the waveform exactly.
    seconds_per_unit = (len(audio) / SAMPLE_RATE) / durations.sum()
    edges = np.concatenate([[0.0], np.cumsum(durations)]) * seconds_per_unit
    timings = [
        {"w": w, "s": round(float(edges[a]), 3), "e": round(float(edges[b]), 3)}
        for w, (a, b) in zip(words, spans)
    ]
    return audio, timings, seconds_per_unit


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument("--model", default="onnx/model.onnx")
    args = parser.parse_args()
    EspeakWrapper.set_library(espeakng_loader.get_library_path())
    EspeakWrapper.set_data_path(espeakng_loader.get_data_path())
    vocab = json.loads(fetch(args.cache, "tokenizer.json").read_text())["model"]["vocab"]
    session = ort.InferenceSession(str(fetch(args.cache, args.model)), providers=["CPUExecutionProvider"])
    args.out.mkdir(parents=True, exist_ok=True)
    for voice, accent in VOICES.items():
        audio, timings, spu = render(session, vocab, fetch(args.cache, f"voices/{voice}.bin"), accent == "b")
        # Trim the trailing silence the model leaves after the last word, keeping 250 ms.
        end = min(len(audio), int((timings[-1]["e"] + 0.25) * SAMPLE_RATE))
        audio = audio[:end]
        sf.write(args.out / f"{voice}.mp3", audio, SAMPLE_RATE, format="MP3", subtype="MPEG_LAYER_III",
                 compression_level=0.35)
        doc = {"voice": voice, "duration": round(len(audio) / SAMPLE_RATE, 3), "words": timings}
        (args.out / f"{voice}.json").write_text(json.dumps(doc, ensure_ascii=False, separators=(",", ":")) + "\n")
        print(voice, f"{doc['duration']} s", f"{spu * 1000:.2f} ms per unit", len(timings), "words")


if __name__ == "__main__":
    main()
