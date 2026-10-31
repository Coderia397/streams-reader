"""Speed of a Kokoro ONNX file on saved token ids (no G2P packages), plus duration parity against the saved int8 run,
plus word timings with a port of kokoro 0.9.4 KPipeline.join_timestamps.
Usage: python -I bench_ids.py <models_dir> <ids.json> <model file> <threads,...>"""
import json
import os
import resource
import sys
import time

import numpy as np
import onnxruntime as ort

M, IDS, FILE, THREADS = sys.argv[1], sys.argv[2], sys.argv[3], [int(t) for t in sys.argv[4].split(",")]
SR = 24_000
rows = json.load(open(IDS))
voices = np.fromfile(os.path.join(M, "af_heart.bin"), dtype=np.float32).reshape(-1, 1, 256)


def join_timestamps(tokens, pred_dur):
    """kokoro 0.9.4 KPipeline.join_timestamps; tokens are [text, phonemes, whitespace]; returns (text, start_s, end_s)."""
    out, left = [], 0
    if not tokens or len(pred_dur) < 3:
        return out
    left = right = 2 * max(0, pred_dur[0] - 3)
    i = 1
    for text, ph, ws in tokens:
        if i >= len(pred_dur) - 1:
            break
        if not ph:
            if ws:
                i += 1
                left = right + pred_dur[i]
                right = left + pred_dur[i]
                i += 1
            continue
        j = i + len(ph)
        if j >= len(pred_dur):
            break
        start = left / 80
        token_dur = sum(pred_dur[i:j])
        space_dur = pred_dur[j] if ws else 0
        left = right + 2 * token_dur + space_dur
        out.append((text, round(start, 3), round(left / 80, 3)))
        right = left + space_dur
        i = j + (1 if ws else 0)
    return out


for n in THREADS:
    so = ort.SessionOptions()
    so.intra_op_num_threads = n
    so.inter_op_num_threads = 1
    t0 = time.perf_counter()
    sess = ort.InferenceSession(os.path.join(M, FILE), so, providers=["CPUExecutionProvider"])
    t_load = time.perf_counter() - t0
    audio = synth = cpu = 0.0
    per, parity = [], []
    for k, r in enumerate(rows):
        feed = {"input_ids": np.array([[0, *r["ids"], 0]], dtype=np.int64), "style": voices[r["style_index"]],
                "speed": np.ones(1, dtype=np.float32)}
        t0, c0 = time.perf_counter(), time.process_time()
        wav, dur = sess.run(None, feed)
        dt, dc = time.perf_counter() - t0, time.process_time() - c0
        cpu += dc
        secs = wav.size / SR
        audio += secs
        synth += dt
        per.append(f"{secs:.1f}s/{dt*1000:.0f}ms")
        d = dur.reshape(-1)
        d8 = np.array(r["int8_durations"])
        parity.append(f"seg{k}: {int((d != d8).sum())}/{d.size} token durations differ, total frames {int(d.sum())} vs int8 {int(d8.sum())}")
        if n == THREADS[0] and k == 3:
            # Tokens whose phonemes contain chars outside the vocab would shift the walk; misaki output here is all in-vocab
            # except the unk marker, which is dropped (the token keeps its other phonemes).
            ws = join_timestamps([[t, (p or "").replace("❓", ""), w] for t, p, w in r["tokens"]], [int(x) for x in d])
            print("  words seg3 (fp from this model):", ws[:8], "... last", ws[-1], "audio", round(secs, 3))
            ws8 = join_timestamps([[t, (p or "").replace("❓", ""), w] for t, p, w in r["tokens"]], [int(x) for x in d8])
            diffs = [abs(a[1] - b[1]) * 1000 for a, b in zip(ws, ws8)]
            print(f"  word start diff vs int8: max {max(diffs):.0f} ms, mean {np.mean(diffs):.1f} ms over {len(diffs)} words")
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1e6
    print(f"{FILE} threads={n}: load {t_load*1000:.0f} ms; {audio:.1f} s audio in {synth:.2f} s -> RTF {synth/audio:.3f} "
          f"({audio/synth:.1f}x realtime); CPU {cpu:.1f} s = {cpu/audio:.2f} CPU-s per audio-s; peak RSS {rss:.0f} MB; {per}")
    if n == THREADS[0]:
        for p in parity:
            print("  ", p)
    del sess
