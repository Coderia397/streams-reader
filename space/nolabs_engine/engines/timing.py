"""Word timings: Kokoro's timed tokens mapped back onto the exact item text, then moved onto the trimmed clip.

Offsets are Python `str` indices into the item text exactly as the Worker sent it (code points, the same unit as Rust
`char` offsets), so the app can highlight `text[char_start:char_end]` with no mapping table.
"""
from __future__ import annotations

from collections.abc import Iterable
from typing import Protocol

Word = tuple[int, int, int, int]


class TimedToken(Protocol):
    text: str
    start_ts: float | None
    end_ts: float | None


def place_tokens(text: str, tokens: Iterable[TimedToken], offset_s: float, cursor: int, out: list[Word]) -> int:
    """Append `[start_ms, end_ms, char_start, char_end]` for each timed token with a letter or digit, searching the
    text left to right from `cursor`. Tokens without timings, punctuation, and tokens the G2P rewrote (so they no longer
    occur in the text) are skipped. Returns the new cursor."""
    for token in tokens:
        piece = (getattr(token, "text", "") or "").strip()
        start, end = getattr(token, "start_ts", None), getattr(token, "end_ts", None)
        if not piece or start is None or end is None or not any(ch.isalnum() for ch in piece):
            continue
        at = text.find(piece, cursor)
        if at < 0:
            continue
        start_ms = max(0, round((offset_s + start) * 1000))
        out.append((start_ms, max(start_ms, round((offset_s + end) * 1000)), at, at + len(piece)))
        cursor = at + len(piece)
    return cursor


def shift_words(words: Iterable[Word], lead_ms: float, speech_ms: int) -> tuple[Word, ...]:
    """Move timings left by the trimmed lead and clamp them inside the speech part of the clip (the appended pause
    carries no words)."""
    out = []
    for start, end, char_start, char_end in words:
        s = min(speech_ms, max(0, round(start - lead_ms)))
        e = min(speech_ms, max(s, round(end - lead_ms)))
        out.append((s, e, char_start, char_end))
    return tuple(out)
