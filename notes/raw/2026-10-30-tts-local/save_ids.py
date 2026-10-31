"""Phonemize bench segments with misaki 0.9.4; save phonemes, token ids, misaki tokens, and int8 durations, so later
runs need no G2P packages. Usage: python -I save_ids.py <models_dir> <out.json>"""
import json, os, sys
import numpy as np
import onnxruntime as ort
from misaki import en

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

M, OUT = sys.argv[1], sys.argv[2]
vocab = json.load(open(os.path.join(M, "config.json")))["vocab"]
g2p = en.G2P(trf=False, british=False, fallback=None)
voices = np.fromfile(os.path.join(M, "af_heart.bin"), dtype=np.float32).reshape(-1, 1, 256)
so = ort.SessionOptions(); so.intra_op_num_threads = 6
sess = ort.InferenceSession(os.path.join(M, "model_quantized_ts.onnx"), so, providers=["CPUExecutionProvider"])
rows = []
for s in SEGMENTS:
    ps, toks = g2p(s)
    ids = [vocab[c] for c in ps if c in vocab]
    feed = {"input_ids": np.array([[0, *ids, 0]], dtype=np.int64), "style": voices[len(ps) - 1],
            "speed": np.ones(1, dtype=np.float32)}
    w1, d1 = sess.run(None, feed)
    w2, d2 = sess.run(None, feed)
    rows.append({"text": s, "ps": ps, "ids": ids, "style_index": len(ps) - 1,
                 "tokens": [[t.text, t.phonemes, t.whitespace] for t in toks],
                 "int8_durations": d1.reshape(-1).tolist(), "int8_samples": int(w1.size),
                 "int8_rerun_same_wave": bool(np.array_equal(w1, w2)), "int8_rerun_same_dur": bool(np.array_equal(d1, d2)),
                 "int8_rerun_max_abs_diff": float(np.abs(w1 - w2).max()) if w1.shape == w2.shape else None})
json.dump(rows, open(OUT, "w"), ensure_ascii=False)
for r in rows:
    d = np.array(r["int8_durations"])
    print(len(r["ids"]), "ids; durations integral:", bool(np.all(d == np.round(d))), "sum*600:", int(d.sum()) * 600,
          "samples:", r["int8_samples"], "rerun same wave/dur:", r["int8_rerun_same_wave"], r["int8_rerun_same_dur"],
          "max diff", r["int8_rerun_max_abs_diff"])
