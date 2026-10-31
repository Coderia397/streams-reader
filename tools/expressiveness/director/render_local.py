"""Render the test passages with local Kokoro under each mapping condition, in the harness layout
(<out>/<condition>/<passage>/NN.webm + meta.json, one clip per app segment, as the player plays them).

Runs in the Kokoro env (see README.md: `uv venv` + `uv pip install -r requirements.txt`):

    ESPEAK fallback is disabled (kokoro_ctrl._disable_espeak).
    <env>/bin/python tools/expressiveness/director/render_local.py --weights <Kokoro-82M snapshot> --out <dir>
        [--conditions k_plain,a_pause,...] [--narrator af_heart]

Post-processing is the Space's own code (space/nolabs_engine/audio: trim, segment levelling with the Kokoro profile,
peak ceiling), so the only differences from the app are the ones a condition asks for. Each edit (call edge,
inserted silence, split join) is logged in meta.json with the signal level at the cut, for artefacts.py.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import time
from dataclasses import asdict, replace
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REPO / "space"))
from kokoro_ctrl import SR, Engine, WordCtrl  # noqa: E402
from kokoro_map import (BY_NAME, CONDITIONS, EMPH_DUR, EMPH_F0_ST, EMPH_PRE_FRAMES, EMPH_SPLIT_MIN_WORDS,  # noqa: E402
                        EMPH_SPLIT_PAUSE_MS, PARTNERS, Call, Condition, blend_of, cast, gain_of, pause_after,
                        register_of, speed_of)
from nolabs_engine.audio import loudness, silence  # noqa: E402

TARGET_LUFS, MAX_CORRECTION_DB, PEAK_CEILING_DBFS, KBPS = -16.0, 4.0, -1.5, 32   # crates/server/src/config.rs
PART_JOIN_MS = 150
SEED = 0      # gap between a quote and its speech tag when they are separate calls


def wspans(text: str, a: int, b: int) -> list[tuple[int, int]]:
    return [(a + m.start(), a + m.end()) for m in re.finditer(r"\S+", text[a:b])]


# ------------------------------------------------------------------------------------------------------- planning
def plan_calls(d: dict, cond: Condition) -> list[Call]:
    sents = all_sents = d["plan"]
    texts = d["segment_texts"]   # from segments.json (load_director)
    voices = cast(cond.narrator, d.get("characters", [])) if cond.characters else {}
    pieces = []   # (segment, a, b, sentence, speaker)
    for s in sents:
        for pt in (s["parts"] if cond.characters else [{"span": None, "speaker": None}]):
            if pt["span"] is None:
                a, b = s["abs"]
            else:
                a, b = pt["span"]
            seg = texts[s["segment"]]
            while a < b and seg[a].isspace():
                a += 1
            while b > a and seg[b - 1].isspace():
                b -= 1
            if a < b:
                pieces.append((s["segment"], a, b, s["idx"], pt["speaker"] if cond.characters else None))
    groups: list[list[tuple]] = []
    for p in pieces:
        if (cond.unit == "segment" and groups and groups[-1][-1][0] == p[0] and groups[-1][-1][4] == p[4]):
            groups[-1].append(p)
        else:
            groups.append([p])
    calls = []
    for g in groups:
        seg, a, b = g[0][0], g[0][1], g[-1][2]
        text = texts[seg]
        ws = wspans(text, a, b)
        word_sentence = []
        for wa, _ in ws:
            word_sentence.append(next(p[3] for p in g if p[1] <= wa < p[2]))
        in_call = list(dict.fromkeys(p[3] for p in g))
        speaker = g[0][4]
        if cond.characters and speaker is None:
            # the speech tag of a quoted sentence ("said Tom."): the narrator reads it plainly
            sents = [x if x["track"] == "narrator" or x["idx"] not in in_call else
                     {**x, "emotion": "neutral", "intensity": 0.0, "pace": "normal", "emphasis": []} for x in all_sents]
        else:
            sents = all_sents
        voice = voices.get(speaker, cond.narrator) if speaker else cond.narrator
        dom = max((sents[i] for i in in_call), key=lambda s: s["intensity"] * (s["emotion"] != "neutral"))
        mix = blend_of(voice, dom) if (cond.blend and voice in PARTNERS) else {voice: 1.0}
        call = Call(segment=seg, text=text[a:b], sentences=in_call, word_sentence=word_sentence, mix=mix,
                    speaker=speaker)
        call.abs = (a, b)  # type: ignore[attr-defined]
        # tempo
        if cond.speed:
            if len(in_call) == 1:
                call.speed = speed_of(sents[in_call[0]])
            else:
                for wi, si in enumerate(word_sentence):
                    sp = speed_of(sents[si])
                    if sp != 1.0:
                        call.ctrl.setdefault(wi, {})["dur"] = round(1.0 / sp, 4)
        # pitch register
        if cond.f0:
            for wi, si in enumerate(word_sentence):
                st = register_of(sents[si])
                if st:
                    call.ctrl.setdefault(wi, {})["f0_st"] = st
        # in-call sentence gaps
        if cond.director_pauses:
            for wi in range(1, len(word_sentence)):
                if word_sentence[wi] != word_sentence[wi - 1]:
                    call.gaps_ms[wi] = pause_after(sents[word_sentence[wi - 1]], True)
        # pause after the call
        last = sents[in_call[-1]]
        ends_sentence = g[-1][2] >= last["abs"][1] - 1 or not text[g[-1][2]:last["abs"][1]].strip()
        call.pause_after_ms = pause_after(last, cond.director_pauses) if ends_sentence else PART_JOIN_MS
        if cond.gain:
            call.gain_db = gain_of(dom)
        # emphasis
        if cond.emphasis == "dur":
            for si in in_call:
                s = sents[si]
                sw = wspans(text, *s["abs"])
                for e in s["emphasis"]:
                    if e >= len(sw):
                        continue
                    try:
                        wi = ws.index(sw[e])
                    except ValueError:
                        continue
                    c = call.ctrl.setdefault(wi, {})
                    c["dur"] = round(c.get("dur", 1.0) * EMPH_DUR, 4)
                    c["pre_frames"] = EMPH_PRE_FRAMES if wi > 0 else 0
                    if cond.f0:
                        c["f0_st"] = round(c.get("f0_st", 0.0) + EMPH_F0_ST, 2)
        calls.extend(split_emphasis(call, ws, sents, text) if cond.emphasis == "split" else [call])
    return calls


def split_emphasis(call: Call, ws: list[tuple[int, int]], sents: list[dict], text: str) -> list[Call]:
    """A: cut the call before its first emphasised word that has enough words before it."""
    if len(call.sentences) != 1:
        return [call]
    s = sents[call.sentences[0]]
    sw = wspans(text, *s["abs"])
    for e in sorted(s["emphasis"]):
        if e >= len(sw) or sw[e] not in ws:
            continue
        wi = ws.index(sw[e])
        if wi < EMPH_SPLIT_MIN_WORDS or wi == len(ws) - 1:
            continue   # too early, or the sentence-final word (already accented; a one-word call sounds cut off)
        a, b = call.abs  # type: ignore[attr-defined]
        cut = ws[wi][0]
        first = replace(call, text=text[a:cut].rstrip(), word_sentence=call.word_sentence[:wi],
                        pause_after_ms=EMPH_SPLIT_PAUSE_MS, ctrl={}, gaps_ms={})
        second = replace(call, text=text[cut:b], word_sentence=call.word_sentence[wi:], ctrl={}, gaps_ms={})
        first.abs, second.abs = (a, cut), (cut, b)  # type: ignore[attr-defined]
        return [first, second]
    return [call]


# ------------------------------------------------------------------------------------------------------- rendering
def db(x: np.ndarray) -> float:
    return float(20 * np.log10(np.sqrt(np.mean(np.square(x, dtype=np.float64))) + 1e-9)) if x.size else -200.0


def insert_gaps(wave: np.ndarray, words: list[tuple[float, float, int]], gaps: dict[int, int],
                edits: list[dict], at0: int) -> np.ndarray:
    """Lengthen the silence between sentences inside one call: zeros at the quietest 10 ms of the model's own gap."""
    if not gaps:
        return wave
    by_word = {w: (a, b) for a, b, w in words}
    order = sorted(by_word)
    cuts = []
    for w, want in gaps.items():
        prev = max((x for x in order if x < w), default=None)
        if prev is None or w not in by_word:
            continue
        t1, t2 = by_word[prev][1], by_word[w][0]
        a, b = int(t1 * SR), int(t2 * SR)
        natural_ms = (b - a) * 1000 / SR
        if b - a < 480:
            a, b = max(0, a - 240), min(wave.size, b + 240)
        hop = 240
        best, best_db = (a + b) // 2, 0.0
        for i in range(a, max(a + 1, b - hop), hop // 2):
            v = db(wave[i:i + hop])
            if v < best_db or i == a:
                best, best_db = i + hop // 2, v
        add = int(round(max(0.0, want - natural_ms) * SR / 1000))
        cuts.append((best, add, best_db, natural_ms, want))
    out, last = [], 0
    shift = 0
    for pos, add, lvl, nat, want in sorted(cuts):
        out.append(wave[last:pos])
        out.append(np.zeros(add, np.float32))
        edits.append({"kind": "gap_insert", "pos": at0 + pos + shift, "added_ms": round(add * 1000 / SR),
                      "cut_dbfs": round(lvl, 1), "natural_ms": round(nat), "want_ms": want})
        shift += add
        last = pos
    out.append(wave[last:])
    return np.concatenate(out)


def encode_webm(wave: np.ndarray, path: Path) -> None:
    import imageio_ffmpeg
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-y", "-f", "f32le", "-ar", str(SR),
           "-ac", "1", "-i", "-", "-c:a", "libopus", "-b:a", f"{KBPS}k", str(path)]
    subprocess.run(cmd, input=wave.astype(np.float32).tobytes(), check=True)


def render_passage(engine: Engine, name: str, d: dict, cond: Condition, out: Path) -> dict:
    calls = plan_calls(d, cond)
    nseg = len(d["segment_texts"])
    seg_waves: list[list[np.ndarray]] = [[] for _ in range(nseg)]
    seg_edits: list[list[dict]] = [[] for _ in range(nseg)]
    seg_len = [0] * nseg
    call_log = []
    t0 = time.time()
    for c in calls:
        lang = next(iter(c.mix))[0]
        ctrl = {w: WordCtrl(**v) for w, v in c.ctrl.items()}
        # Kokoro's source excitation is random; one seed per (run, text) makes conditions differ only by the mapping
        import torch
        torch.manual_seed(SEED * 1_000_003 + int(hashlib.sha256(c.text.encode()).hexdigest()[:8], 16))
        o = engine.synth(c.text, c.mix, lang=lang, speed=c.speed, ctrl=ctrl)
        raw_peak = float(np.abs(o.wave).max())
        raw_clip = int(np.sum(np.abs(o.wave) >= 0.999))
        tr = silence.trim(silence.sanitise(o.wave), SR)
        lead_s = tr.lead_samples / SR
        words = [(a - lead_s, b - lead_s, w) for a, b, w in o.words]
        wave = tr.wave
        edits = seg_edits[c.segment]
        at = seg_len[c.segment]
        edits.append({"kind": "call_start", "pos": at, "edge_db": round(db(wave[:240]) - db(wave), 1)})
        wave = insert_gaps(wave, words, c.gaps_ms, edits, at)
        if c.gain_db:
            wave = wave * np.float32(10 ** (c.gain_db / 20))
        edits.append({"kind": "call_end", "pos": at + wave.size, "edge_db": round(db(wave[-240:]) - db(wave), 1)})
        pause = np.zeros(int(round(c.pause_after_ms * SR / 1000)), np.float32)
        seg_waves[c.segment] += [wave, pause]
        seg_len[c.segment] += wave.size + pause.size
        call_log.append({"segment": c.segment, "text": c.text, "mix": c.mix, "speed": c.speed, "ctrl": c.ctrl,
                         "gaps_ms": c.gaps_ms, "pause_after_ms": c.pause_after_ms, "gain_db": c.gain_db,
                         "speaker": c.speaker, "raw_peak": round(raw_peak, 3), "raw_clipped": raw_clip,
                         "seconds": round(wave.size / SR, 2)})
    synth_s = time.time() - t0
    # Space levelling: programme loudness over segments in order, per-segment correction, peak ceiling.
    d_out = out / cond.name / name
    d_out.mkdir(parents=True, exist_ok=True)
    prog = loudness.Programme(SR)
    meta_segs = []
    for i in range(nseg):
        w = np.concatenate(seg_waves[i])
        # the trailing pause is appended after levelling in the Space; measure and gain the speech part only
        speech = np.trim_zeros(w, "b")
        prog.add(speech)
        gain = loudness.plan_gains([loudness.measure(speech, SR)], prog.loudness(), TARGET_LUFS, MAX_CORRECTION_DB)[0]
        lev, applied = loudness.apply_gain(w, gain, PEAK_CEILING_DBFS)
        f = f"{i:02d}.webm"
        encode_webm(lev, d_out / f)
        meta_segs.append({"file": f, "text": d["segment_texts"][i], "boundary": d["segment_boundaries"][i],
                          "duration_ms": int(round(lev.size * 1000 / SR)), "gain_db": round(applied, 2),
                          "gain_planned_db": round(gain, 2), "ceiling_limited_db": round(gain - applied, 2),
                          "edits": seg_edits[i]})
    meta = {"voice": f"local:kokoro:{cond.narrator}", "label": cond.name, "passage": name,
            "condition": asdict(cond), "seed": SEED, "segments": meta_segs, "calls": call_log, "synth_s": round(synth_s, 1)}
    (d_out / "meta.json").write_text(json.dumps(meta, indent=1))
    return meta


def load_director(passage: str, segments: list[dict], ddir: Path) -> dict:
    d = json.loads((ddir / f"{passage}.json").read_text())
    d["segment_texts"] = [s["text"] for s in segments]
    d["segment_boundaries"] = [s.get("boundary", "paragraph") for s in segments]
    for s in d["plan"]:
        a, b = segments[s["segment"]]["sentences"][[x["idx"] for x in d["plan"] if x["segment"] == s["segment"]].index(s["idx"])]
        s["abs"] = (a, b)
    return d


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--weights", required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--conditions", default=",".join(c.name for c in CONDITIONS))
    ap.add_argument("--narrator", default="af_heart")
    ap.add_argument("--suffix", default="", help="appended to condition labels (e.g. _george)")
    ap.add_argument("--passages", type=Path, default=HERE.parent / "passages/segments.json")
    ap.add_argument("--director", type=Path, default=HERE / "out")
    ap.add_argument("--plan-only", action="store_true")
    ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    global SEED
    SEED = a.seed
    data = json.loads(a.passages.read_text())
    engine = None if a.plan_only else Engine(a.weights)
    for cname in a.conditions.split(","):
        cond = replace(BY_NAME[cname], narrator=a.narrator, name=cname + a.suffix)
        for name, segments in data.items():
            d = load_director(name, segments, a.director)
            if a.plan_only:
                for c in plan_calls(d, cond):
                    print(cname, name, c.segment, c.mix, c.speed, c.pause_after_ms, c.gaps_ms, c.ctrl, repr(c.text[:60]))
                continue
            m = render_passage(engine, name, d, cond, a.out)
            dur = sum(s["duration_ms"] for s in m["segments"]) / 1000
            print(f"{cond.name}/{name}: {len(m['calls'])} calls, {dur:.1f} s audio, synth {m['synth_s']} s, "
                  f"max raw peak {max(c['raw_peak'] for c in m['calls'])}", file=sys.stderr, flush=True)


if __name__ == "__main__":
    main()
