"""Kokoro-82M int8 ONNX speed on this CPU: phonemize with espeak-ng (kokoro-onnx tokenizer), run onnxruntime directly.
Usage: python -I bench.py <models_dir> <threads,...> [model files...]"""
import json
import os
import resource
import sys
import time

import numpy as np
import onnxruntime as ort
from misaki import en

MODELS = sys.argv[1]
THREADS = [int(t) for t in sys.argv[2].split(",")]
FILES = sys.argv[3:] or ["model_quantized_ts.onnx"]
SR = 24_000

# Four segments of 220-320 chars, like the KOKORO segment profile (min 120, max 320).
SEGMENTS = [
    "Alice was beginning to get very tired of sitting by her sister on the bank, and of having nothing to do: once "
    "or twice she had peeped into the book her sister was reading, but it had no pictures or conversations in it.",
    "“And what is the use of a book,” thought Alice, “without pictures or conversations?” So she was "
    "considering in her own mind (as well as she could, for the hot day made her feel very sleepy and stupid), "
    "whether the pleasure of making a daisy-chain would be worth the trouble of getting up and picking the daisies.",
    "There was nothing so very remarkable in that; nor did Alice think it so very much out of the way to hear the "
    "Rabbit say to itself, “Oh dear! Oh dear! I shall be late!” (when she thought it over afterwards, it "
    "occurred to her that she ought to have wondered at this, but at the time it all seemed quite natural).",
    "In 2026 the library held 1,250 books, and Dr. Smith read 42 of them aloud at 7:30 p.m. every Tuesday.",
]

vocab = json.load(open(os.path.join(MODELS, "config.json")))["vocab"]
t0 = time.perf_counter()
g2p = en.G2P(trf=False, british=False, fallback=None)
print(f"misaki G2P init {time.perf_counter()-t0:.1f} s")
voices = np.fromfile(os.path.join(MODELS, "af_heart.bin"), dtype=np.float32).reshape(-1, 1, 256)

t0 = time.perf_counter()
g = [g2p(s) for s in SEGMENTS]
t_phon = time.perf_counter() - t0
phon = [p for p, _ in g]
mtoks = [t for _, t in g]
ids = [[vocab[c] for c in p if c in vocab] for p in phon]
print(f"misaki phonemize {len(SEGMENTS)} segments ({sum(map(len, SEGMENTS))} chars): {t_phon*1000:.0f} ms")
for s, p in zip(SEGMENTS[-1:], phon[-1:]):
    print("  sample:", p[:160])
print("  tokens per segment:", [len(i) for i in ids])

for f in FILES:
    for n in THREADS:
        so = ort.SessionOptions()
        so.intra_op_num_threads = n
        so.inter_op_num_threads = 1
        so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        t0 = time.perf_counter()
        sess = ort.InferenceSession(os.path.join(MODELS, f), so, providers=["CPUExecutionProvider"])
        t_load = time.perf_counter() - t0
        outs = [o.name for o in sess.get_outputs()]
        total_audio = total_synth = 0.0
        per = []
        for k, toks in enumerate(ids):
            feed = {"input_ids": np.array([[0, *toks, 0]], dtype=np.int64), "style": voices[len(toks)],
                    "speed": np.ones(1, dtype=np.float32)}
            t0 = time.perf_counter()
            res = sess.run(None, feed)
            dt = time.perf_counter() - t0
            wav = res[0].reshape(-1)
            secs = wav.size / SR
            note = ""
            if "durations" in outs:
                dur = res[1].reshape(-1)
                note = f" dur_sum*600={int(round(dur.sum()))*600} samples={wav.size}"
                if n == THREADS[0] and k == 3:
                    # word timings: walk misaki tokens over the id sequence ([0] + ids + [0]); 1 frame = 600 samples
                    fr = np.concatenate([[0], np.cumsum(dur)])
                    pos, words = 1, []
                    for t in mtoks[k]:
                        n_ph = sum(1 for c in (t.phonemes or "") if c in vocab)
                        if n_ph:
                            words.append((t.text, round(fr[pos] * 25), round(fr[pos + n_ph] * 25)))
                        pos += n_ph + (1 if t.whitespace else 0)
                    print("  word timings ms (segment 4):", words[:12], "... last", words[-1], "of", round(secs*1000))
            if k == 0:
                first = dt
            per.append(f"{secs:.1f}s/{dt*1000:.0f}ms")
            total_audio += secs
            total_synth += dt
            if k == 0 and n == THREADS[0]:
                print(f"  [{f}] outputs={outs}{note}")
        rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1e6  # bytes on macOS -> MB
        print(f"{f} threads={n}: load {t_load*1000:.0f} ms; first segment {first*1000:.0f} ms; "
              f"{total_audio:.1f} s audio in {total_synth:.2f} s -> RTF {total_synth/total_audio:.3f} "
              f"({total_audio/total_synth:.1f}x realtime); peak RSS {rss:.0f} MB; {per}")
        if n == THREADS[-1] and f == FILES[-1]:
            import wave
            out = os.path.join(os.path.dirname(MODELS), "sample.wav")
            with wave.open(out, "wb") as w:
                w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
                w.writeframes((np.clip(wav, -1, 1) * 32767).astype("<i2").tobytes())
        del sess
