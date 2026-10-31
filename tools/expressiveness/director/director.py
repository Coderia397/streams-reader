#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""LLM director prototype (notes/06 v2 phase 2): Workers AI gemma-4-26b with JSON-schema output, then the checks the
Rust side would run (anchoring, hysteresis, tag rules). Responses are cached on disk, so re-runs spend no neurons.

    uv run tools/expressiveness/director/director.py [--passages tools/expressiveness/passages/segments.json]
        [--out tools/expressiveness/director/out] [--refresh]

Input: passages/segments.json (the app's own segmenter: segments with boundary and sentence ranges).
Output per passage: out/<passage>.json = {"raw": LLM JSON, "plan": validated sentences, "log": what was changed}.
Cache key: sha256(model, PROMPT_VERSION, prompt.md, schema.json, request text) -> cache/<key>.json.
Credentials: ~/.config/streams/cloudflare.env (CF_EMAIL, CF_KEY, CF_ACCOUNT); never printed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
MODEL = "@cf/google/gemma-4-26b-a4b-it"
PROMPT_VERSION = 1
MAX_TOKENS = 12000
# gemma-4 on Workers AI reasons before answering unless thinking is switched off in the chat template.
EXTRA = {"chat_template_kwargs": {"enable_thinking": False}}
PROMPT = (HERE / "prompt.md").read_text()
SCHEMA = json.loads((HERE / "schema.json").read_text())
CACHE = HERE / "cache"
ALLOWED_TAGS = {"[Breathing]", "[Sigh]", "[Laughter]", "[Surprise-ah]", "[Surprise-oh]", "[Uhm]", "[Cough]"}
EMOTIONS = {"neutral", "happy", "sad", "angry", "fearful", "disgusted", "surprised"}
# Rules from notes/raw/2026-10-30-arch-review.md item 17.
ANCHOR_WINDOW = 3
MAX_TAGS_SENTENCE, MAX_TAGS_SEGMENT = 1, 2
MAX_EMPHASIS = 2
HOLD_SENTENCES = 3          # an emotion change on one track at most once per 3 sentences of that track
STRONG_CUE = (0.85, 0.6)    # confidence, intensity at or above which a change may break the hold (assumed rule)
MIN_CONFIDENCE = 0.5        # below this the emotion falls back to neutral
EMOTION_MODE_CONFIDENCE = 0.75
PAUSE_RANGE_MS = (120, 1800)
PARAGRAPH_MIN_PAUSE_MS = 400
QUOTE_OPEN, QUOTE_CLOSE = "“", "”"


# --------------------------------------------------------------------------------------------------------- text model
def words_of(text: str) -> list[tuple[int, int, str]]:
    return [(m.start(), m.end(), m.group()) for m in re.finditer(r"\S+", text)]


def norm_word(w: str) -> str:
    return re.sub(r"[^\w']", "", w.lower().replace("’", "'"))


def quote_spans(text: str) -> list[tuple[int, int]]:
    """Character spans of quoted speech, quote marks included. Straight quotes toggle; curly ones open and close."""
    spans, start = [], None
    for i, ch in enumerate(text):
        if ch == QUOTE_OPEN or (ch == '"' and start is None):
            start = i
        elif (ch == QUOTE_CLOSE or ch == '"') and start is not None:
            spans.append((start, i + 1))
            start = None
    if start is not None:
        spans.append((start, len(text)))
    return spans


def build_passage(segments: list[dict]) -> dict:
    """Sentences (global index) with their words and quoted/unquoted parts, plus the passage's quotes."""
    sentences, quotes = [], []
    for si, seg in enumerate(segments):
        text = seg["text"]
        qspans = quote_spans(text)
        q_ids = []
        for a, b in qspans:
            q_ids.append(len(quotes))
            quotes.append({"quote_idx": len(quotes), "segment": si, "span": [a, b], "text": text[a:b]})
        ranges = seg["sentences"]
        for k, (a, b) in enumerate(ranges):
            parts, cur = [], a
            for qi, (qa, qb) in zip(q_ids, qspans):
                lo, hi = max(a, qa), min(b, qb)
                if lo >= hi:
                    continue
                if text[cur:lo].strip():
                    parts.append({"span": [cur, lo], "quote_idx": None})
                parts.append({"span": [lo, hi], "quote_idx": qi})
                cur = hi
            if text[cur:b].strip():
                parts.append({"span": [cur, b], "quote_idx": None})
            stext = text[a:b]
            sentences.append({"idx": len(sentences), "segment": si, "span": [a, b], "text": stext,
                              "words": [w for _, _, w in words_of(stext)], "parts": parts,
                              "last_in_segment": k == len(ranges) - 1, "boundary": seg.get("boundary", "paragraph")})
    for q in quotes:
        q["sentences"] = [s["idx"] for s in sentences if s["segment"] == q["segment"]
                          and s["span"][0] < q["span"][1] and q["span"][0] < s["span"][1]]
    return {"segments": segments, "sentences": sentences, "quotes": quotes}


def request_text(p: dict) -> str:
    lines = ["Sentences:"]
    for s in p["sentences"]:
        par = " (ends paragraph)" if s["last_in_segment"] else ""
        lines.append(f"[{s['idx']}]{par} {s['text']}")
        lines.append("    words: " + " | ".join(f"{i}:{w}" for i, w in enumerate(s["words"])))
    lines.append("")
    lines.append("Quotes:" if p["quotes"] else "Quotes: none")
    for q in p["quotes"]:
        lines.append(f"[q{q['quote_idx']}] (sentences {', '.join(map(str, q['sentences']))}) {q['text']}")
    return "\n".join(lines)


# ---------------------------------------------------------------------------------------------------------------- LLM
def _env() -> dict:
    env = {}
    for line in (Path.home() / ".config/streams/cloudflare.env").read_text().splitlines():
        if "=" in line and not line.lstrip().startswith("#"):
            k, v = line.split("=", 1)
            env[k.strip()] = v.strip().strip('"').strip("'")
    return env


def cache_key(text: str) -> str:
    h = hashlib.sha256()
    for part in (MODEL, str(PROMPT_VERSION), PROMPT, json.dumps(SCHEMA, sort_keys=True), text):
        h.update(part.encode())
        h.update(b"\0")
    return h.hexdigest()


def call_llm(text: str, refresh: bool = False) -> tuple[dict, dict]:
    """(parsed JSON, cache record). The record holds usage and timing."""
    key = cache_key(text)
    path = CACHE / f"{key}.json"
    if path.exists() and not refresh:
        rec = json.loads(path.read_text())
        return rec["response"], rec
    env = _env()
    url = f"https://api.cloudflare.com/client/v4/accounts/{env['CF_ACCOUNT']}/ai/run/{MODEL}"
    body = {"messages": [{"role": "system", "content": PROMPT}, {"role": "user", "content": text}],
            "response_format": {"type": "json_schema", "json_schema": SCHEMA},
            "max_tokens": MAX_TOKENS, "temperature": 0.2, **EXTRA}
    req = urllib.request.Request(url, data=json.dumps(body).encode(), method="POST",
                                 headers={"X-Auth-Email": env["CF_EMAIL"], "X-Auth-Key": env["CF_KEY"],
                                          "Content-Type": "application/json"})
    t0 = time.time()
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=300) as r:
                out = json.loads(r.read())
            break
        except urllib.error.HTTPError as e:
            msg = e.read()[:400]
            if e.code < 500 or attempt == 2:
                raise SystemExit(f"Workers AI HTTP {e.code}: {msg!r}")
            time.sleep(5)
    res = out["result"]
    msg = res["choices"][0]["message"]
    content = msg.get("content")
    try:
        parsed = content if isinstance(content, dict) else json.loads(content)
    except (TypeError, json.JSONDecodeError):
        CACHE.mkdir(parents=True, exist_ok=True)
        (CACHE / f"failed-{key}.json").write_text(json.dumps(out, indent=1))
        raise SystemExit(f"no JSON in the reply (finish {res['choices'][0].get('finish_reason')}, "
                         f"usage {res.get('usage')}); saved cache/failed-{key}.json")
    rec = {"model": MODEL, "prompt_version": PROMPT_VERSION, "key": key, "elapsed_s": round(time.time() - t0, 1),
           "usage": res.get("usage"), "finish_reason": res["choices"][0].get("finish_reason"),
           "reasoning_chars": len(msg.get("reasoning_content") or ""), "request": text, "response": parsed}
    CACHE.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(rec, indent=1))
    return parsed, rec


# --------------------------------------------------------------------------------------------------------- validation
def anchor(words: list[str], idx: int, word: str) -> int | None:
    """Index of `word` nearest to `idx` within ±ANCHOR_WINDOW, or None (the Rust check in notes/06 phase 2)."""
    target = norm_word(word)
    if not target:
        return None
    for d in range(ANCHOR_WINDOW + 1):
        for j in ((idx,) if d == 0 else (idx - d, idx + d)):
            if 0 <= j < len(words) and norm_word(words[j]) == target:
                return j
    return None


def validate(p: dict, raw: dict) -> tuple[list[dict], dict, list[str]]:
    log: list[str] = []
    fiction = raw.get("genre") == "fiction"
    chars = {c["name"]: c for c in raw.get("characters", []) if c.get("name")}
    speakers = {}
    for q in raw.get("quotes", []):
        if 0 <= q.get("quote_idx", -1) < len(p["quotes"]) and q.get("speaker") in chars:
            speakers[q["quote_idx"]] = q["speaker"]
    for q in p["quotes"]:
        if q["quote_idx"] not in speakers:
            log.append(f"q{q['quote_idx']}: no valid speaker")
    by_idx = {e.get("sentence_idx"): e for e in raw.get("sentences", [])}
    plan, last_change, track_count, current = [], {}, {}, {}
    tags_in_segment: dict[int, int] = {}
    for s in p["sentences"]:
        e = by_idx.get(s["idx"])
        if e is None:
            log.append(f"s{s['idx']}: missing, neutral defaults")
            e = {}
        words = s["words"]
        quoted = sum(pt["span"][1] - pt["span"][0] for pt in s["parts"] if pt["quote_idx"] is not None)
        qparts = [pt["quote_idx"] for pt in s["parts"] if pt["quote_idx"] is not None]
        track = speakers.get(qparts[0], "?speaker") if qparts and quoted * 2 > len(s["text"]) else "narrator"
        emotion = e.get("emotion") if e.get("emotion") in EMOTIONS else "neutral"
        conf = min(1.0, max(0.0, float(e.get("confidence", 0))))
        inten = min(1.0, max(0.0, float(e.get("intensity", 0))))
        if emotion != "neutral" and conf < MIN_CONFIDENCE:
            log.append(f"s{s['idx']}: {emotion} at confidence {conf} -> neutral")
            emotion, inten = "neutral", inten * 0.5
        # Hysteresis per track.
        n = track_count.get(track, 0)
        prev = current.get(track, "neutral")
        if emotion != prev:
            since = n - last_change.get(track, -HOLD_SENTENCES)
            strong = conf >= STRONG_CUE[0] and inten >= STRONG_CUE[1]
            if since < HOLD_SENTENCES and not strong:
                log.append(f"s{s['idx']}: {track} {prev}->{emotion} held ({since} sentences since last change)")
                emotion = prev
            else:
                last_change[track] = n
                current[track] = emotion
        track_count[track] = n + 1
        # Emphasis.
        emph = []
        for w in (e.get("emphasis_words") or [])[:MAX_EMPHASIS + 2]:
            j = anchor(words, int(w.get("word_idx", -99)), str(w.get("word", "")))
            if j is None:
                log.append(f"s{s['idx']}: emphasis {w} not anchored, dropped")
            elif j not in emph and len(emph) < MAX_EMPHASIS:
                if j != w.get("word_idx"):
                    log.append(f"s{s['idx']}: emphasis {w.get('word')!r} moved {w.get('word_idx')}->{j}")
                emph.append(j)
        # Tags.
        tags = []
        for t in e.get("tags") or []:
            if not fiction:
                log.append(f"s{s['idx']}: tag {t.get('tag')} dropped (nonfiction)")
                continue
            if t.get("tag") not in ALLOWED_TAGS:
                log.append(f"s{s['idx']}: tag {t.get('tag')} not allowed")
                continue
            if len(tags) >= MAX_TAGS_SENTENCE or tags_in_segment.get(s["segment"], 0) >= MAX_TAGS_SEGMENT:
                log.append(f"s{s['idx']}: tag {t.get('tag')} over the cap")
                continue
            j = anchor(words, int(t.get("before_word_idx", -99)), str(t.get("anchor_word", "")))
            if j is None:
                log.append(f"s{s['idx']}: tag {t} not anchored, dropped")
                continue
            tags.append({"before_word_idx": j, "tag": t["tag"]})
            tags_in_segment[s["segment"]] = tags_in_segment.get(s["segment"], 0) + 1
        mode = e.get("mode", "plain")
        if mode == "emotion" and not (conf >= EMOTION_MODE_CONFIDENCE and track != "narrator" and emotion != "neutral"):
            mode = "plain"
        if mode == "tags" and not tags:
            mode = "plain"
        if tags and mode == "plain":
            mode = "tags"
        pause = int(e.get("pause_after_ms", 0) or 0)
        lo, hi = PAUSE_RANGE_MS
        if s["last_in_segment"]:
            lo = PARAGRAPH_MIN_PAUSE_MS
        pause_c = min(hi, max(lo, pause))
        if pause_c != pause:
            log.append(f"s{s['idx']}: pause {pause} -> {pause_c}")
        pace = e.get("pace") if e.get("pace") in ("slow", "normal", "fast") else "normal"
        parts = [{**pt, "speaker": speakers.get(pt["quote_idx"]) if pt["quote_idx"] is not None else None}
                 for pt in s["parts"]]
        plan.append({"idx": s["idx"], "segment": s["segment"], "text": s["text"], "words": words, "parts": parts,
                     "last_in_segment": s["last_in_segment"], "boundary": s["boundary"], "track": track,
                     "mode": mode, "emotion": emotion, "confidence": conf, "intensity": round(inten, 3),
                     "pace": pace, "pause_after_ms": pause_c, "emphasis": emph, "tags": tags})
    meta = {"genre": raw.get("genre"), "characters": list(chars.values())}
    return plan, meta, log


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--passages", type=Path, default=HERE.parent / "passages/segments.json")
    ap.add_argument("--out", type=Path, default=HERE / "out")
    ap.add_argument("--refresh", action="store_true", help="ignore the cache (spends neurons)")
    a = ap.parse_args()
    data = json.loads(a.passages.read_text())
    a.out.mkdir(parents=True, exist_ok=True)
    for name, segments in data.items():
        p = build_passage(segments)
        text = request_text(p)
        raw, rec = call_llm(text, a.refresh)
        plan, meta, log = validate(p, raw)
        (a.out / f"{name}.json").write_text(json.dumps(
            {"passage": name, "model": MODEL, "prompt_version": PROMPT_VERSION, "cache_key": rec["key"],
             "usage": rec.get("usage"), "elapsed_s": rec.get("elapsed_s"), **meta, "quotes": p["quotes"],
             "raw": raw, "plan": plan, "log": log}, indent=1))
        u = rec.get("usage") or {}
        print(f"{name}: {len(plan)} sentences, genre {meta['genre']}, {len(log)} fixes, "
              f"neurons {u.get('neurons', 0):.0f}, tokens {u.get('total_tokens')}", file=sys.stderr)
        for s in plan:
            print(f"  [{s['idx']:2}] {s['track'][:8]:8} {s['emotion']:9} c{s['confidence']:.2f} i{s['intensity']:.2f} "
                  f"{s['pace']:6} {s['pause_after_ms']:5}ms emph={[s['words'][j] for j in s['emphasis']]} "
                  f"tags={[t['tag'] for t in s['tags']]} | {s['text'][:50]}", file=sys.stderr)
        for line in log:
            print("   fix:", line, file=sys.stderr)


if __name__ == "__main__":
    main()
