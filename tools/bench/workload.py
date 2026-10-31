"""The benchmark workload shared by local_kokoro.py and space_bench.py: a fixed 1,000-character public-domain passage,
split the way the Worker splits Kokoro text (sentences packed to 120-320 chars), and a contract v1 EngineRequest for it.

Object keys are fixed per segment index, so repeated runs against the live Space overwrite the same handful of R2
objects instead of adding new ones.
"""
from __future__ import annotations

import hashlib
import re

# Lewis Carroll, "Alice's Adventures in Wonderland" (1865), chapter 1; public domain.
PASSAGE = (
    "Alice was beginning to get very tired of sitting by her sister on the bank, and of having nothing to do: once or "
    "twice she had peeped into the book her sister was reading, but it had no pictures or conversations in it, “and "
    "what is the use of a book,” thought Alice “without pictures or conversations?” So she was "
    "considering in her own mind (as well as she could, for the hot day made her feel very sleepy and stupid), whether "
    "the pleasure of making a daisy-chain would be worth the trouble of getting up and picking the daisies, when "
    "suddenly a White Rabbit with pink eyes ran close by her. There was nothing so very remarkable in that; nor did "
    "Alice think it so very much out of the way to hear the Rabbit say to itself, “Oh dear! Oh dear! I shall be "
    "late!” (when she thought it over afterwards, it occurred to her that she ought to have wondered at this, but "
    "at the time it all seemed quite natural); but when the Rabbit actually took a watch out of its waistcoat-pocket, "
    "and looked at it, and then hurried on, Alice started to her feet, for it flashed across her mind that she had "
    "never before seen a rabbit with either a waistcoat-pocket, or a watch to take out of it."
)

MIN_CHARS, MAX_CHARS = 120, 320  # crates/core/src/text/segmenter.rs, SegmentProfile::KOKORO
JOB_ID = "benchbenchbench0"  # 16 chars of lowercase Crockford base32, like crates/core ids
KOKORO_AUDIO = {"id": 1, "bitrate_kbps": 32, "sample_rate": 24000, "target_lufs": -16.0, "max_correction_db": 4.0,
                "peak_ceiling_dbfs": -1.5, "pauses": {"sentence_ms": 250, "paragraph_ms": 600, "section_ms": 1200}}
MING_AUDIO = {**KOKORO_AUDIO, "id": 2, "bitrate_kbps": 56, "sample_rate": 48000, "max_correction_db": 2.0}


def text(chars: int = 1000) -> str:
    """The passage cut to at most `chars` characters at a word boundary (repeated if more are asked for)."""
    source = PASSAGE
    while len(source) < chars:
        source = f"{source} {PASSAGE}"
    if len(source) <= chars:
        return source
    cut = source[:chars]
    return cut[: cut.rfind(" ")] if " " in cut else cut


def segments(body: str) -> list[str]:
    """Sentences packed greedily up to MAX_CHARS; a sentence longer than that is split at clause punctuation, then
    at spaces. Close to the Worker's segmenter for prose; not a copy of it."""
    sentences = [s for s in re.split(r"(?<=[.!?”)])\s+(?=[A-Z“(])", body.strip()) if s]
    pieces: list[str] = []
    for sentence in sentences:
        pieces.extend(_split_long(sentence))
    out: list[str] = []
    for piece in pieces:
        merged = len(out[-1]) + 1 + len(piece) if out else 0
        # Join while it fits; a segment still under MIN_CHARS may run to 5/4 of MAX_CHARS, as the Worker allows.
        if out and (merged <= MAX_CHARS or (len(out[-1]) < MIN_CHARS and merged <= MAX_CHARS * 5 // 4)):
            out[-1] = f"{out[-1]} {piece}"
        else:
            out.append(piece)
    return out


def _split_long(sentence: str) -> list[str]:
    if len(sentence) <= MAX_CHARS:
        return [sentence]
    parts, current = [], ""
    for clause in re.split(r"(?<=[,;:])\s+", sentence):
        while len(clause) > MAX_CHARS:
            at = clause.rfind(" ", 0, MAX_CHARS)
            at = at if at > 0 else MAX_CHARS
            parts.append(clause[:at])
            clause = clause[at:].lstrip()
        if current and len(current) + 1 + len(clause) > MAX_CHARS:
            parts.append(current)
            current = clause
        else:
            current = f"{current} {clause}" if current else clause
    if current:
        parts.append(current)
    return parts


def key(engine: str, index: int) -> str:
    return hashlib.sha256(f"nolabs-bench:{engine}:{index}".encode()).hexdigest()


def request(engine: str = "kokoro", voice: str = "af_heart", chars: int = 1000) -> dict:
    """Contract v1 EngineRequest (crates/core/src/proto/engine.rs) for the benchmark text."""
    mode = "kokoro" if engine == "kokoro" else "ip"
    items = [{"key": key(engine, i), "text": t, "mode": mode, "emotion": "neutral",
              "boundary": "paragraph" if i == len(parts) - 1 else "sentence",
              "object_key": f"audio/{engine}/{key(engine, i)}.webm"}
             for parts in [segments(text(chars))] for i, t in enumerate(parts)]
    return {"v": 1, "job_id": JOB_ID, "engine": engine, "voice": {"kind": "preset", "id": voice}, "items": items,
            "audio": KOKORO_AUDIO if engine == "kokoro" else MING_AUDIO, "marker_key": f"jobs/{JOB_ID}.json"}
