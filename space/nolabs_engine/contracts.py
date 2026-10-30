"""Wire contract between the Cloudflare Worker and this Space.

Every endpoint takes one JSON string and returns one JSON string, so the Gradio schema stays trivial and the contract is
versioned here rather than in Gradio components. The Rust side mirrors these shapes in `crates/core/src/engine.rs`; bump
`CONTRACT_VERSION` on both sides together.
"""
from __future__ import annotations

import json
import re
from dataclasses import asdict, dataclass, field
from typing import Literal

CONTRACT_VERSION = 1

MAX_SEGMENTS = 64
MAX_SEGMENT_CHARS = 1200
MAX_BATCH_CHARS = 24_000

KEY_RE = re.compile(r"^[0-9a-f]{64}$")
REF_KEY_RE = re.compile(r"^voices/[A-Za-z0-9_\-/]{1,120}\.(wav|ogg|webm|m4a|mp3)$")

Emotion = Literal["neutral", "happy", "sad", "angry", "fearful", "disgusted", "surprised"]
EMOTIONS: tuple[str, ...] = ("neutral", "happy", "sad", "angry", "fearful", "disgusted", "surprised")


class ContractError(ValueError):
    """The request does not match the contract. Reported to the caller as a 400-style error."""


@dataclass(frozen=True)
class Segment:
    key: str
    text: str
    emotion: str = "neutral"


@dataclass(frozen=True)
class Voice:
    """`preset` names a built-in voice of the engine; `reference` points at a clip in R2 plus its exact transcript."""
    kind: Literal["preset", "reference"]
    id: str = ""
    ref_key: str = ""
    transcript: str = ""


@dataclass(frozen=True)
class RenderRequest:
    engine: Literal["kokoro", "ming"]
    voice: Voice
    segments: tuple[Segment, ...]
    speed: float = 1.0
    bitrate_kbps: int = 48


@dataclass(frozen=True)
class DesignRequest:
    description: str
    sample_text: str
    key: str


@dataclass
class WordTiming:
    start_ms: int
    end_ms: int
    char_start: int
    char_end: int


@dataclass
class SegmentResult:
    key: str
    object_key: str = ""
    duration_ms: int = 0
    bytes: int = 0
    words: list[list[int]] = field(default_factory=list)
    error: str = ""


@dataclass
class RenderResponse:
    results: list[SegmentResult]
    engine: str
    elapsed_ms: int
    gpu_ms: int = 0
    v: int = CONTRACT_VERSION

    def to_json(self) -> str:
        return json.dumps(asdict(self), separators=(",", ":"), ensure_ascii=False)


def audio_object_key(key: str) -> str:
    return f"audio/{key}.ogg"


def _obj(raw: str) -> dict:
    try:
        data = json.loads(raw)
    except (TypeError, json.JSONDecodeError) as e:
        raise ContractError(f"request is not JSON: {e}") from None
    if not isinstance(data, dict):
        raise ContractError("request must be a JSON object")
    if data.get("v") != CONTRACT_VERSION:
        raise ContractError(f"contract version {data.get('v')!r} not supported (expected {CONTRACT_VERSION})")
    return data


def _text(value: object, name: str, limit: int) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ContractError(f"{name} must be a non-empty string")
    value = value.strip()
    if len(value) > limit:
        raise ContractError(f"{name} is {len(value)} characters, limit {limit}")
    return value


def _voice(data: object) -> Voice:
    if not isinstance(data, dict):
        raise ContractError("voice must be an object")
    kind = data.get("kind")
    if kind == "preset":
        return Voice(kind="preset", id=_text(data.get("id"), "voice.id", 80))
    if kind == "reference":
        ref_key = _text(data.get("ref_key"), "voice.ref_key", 200)
        if not REF_KEY_RE.match(ref_key):
            raise ContractError("voice.ref_key must be an R2 key under voices/")
        return Voice(kind="reference", ref_key=ref_key, transcript=_text(data.get("transcript"), "voice.transcript", 1000))
    raise ContractError("voice.kind must be 'preset' or 'reference'")


def parse_render(raw: str) -> RenderRequest:
    data = _obj(raw)
    engine = data.get("engine")
    if engine not in ("kokoro", "ming"):
        raise ContractError("engine must be 'kokoro' or 'ming'")
    voice = _voice(data.get("voice"))
    if engine == "kokoro" and voice.kind != "preset":
        raise ContractError("kokoro only has preset voices")
    items = data.get("segments")
    if not isinstance(items, list) or not 1 <= len(items) <= MAX_SEGMENTS:
        raise ContractError(f"segments must be a list of 1 to {MAX_SEGMENTS} items")
    segments, total, seen = [], 0, set()
    for i, item in enumerate(items):
        if not isinstance(item, dict):
            raise ContractError(f"segments[{i}] must be an object")
        key = item.get("key")
        if not isinstance(key, str) or not KEY_RE.match(key):
            raise ContractError(f"segments[{i}].key must be 64 lowercase hex characters")
        if key in seen:
            continue
        seen.add(key)
        text = _text(item.get("text"), f"segments[{i}].text", MAX_SEGMENT_CHARS)
        emotion = item.get("emotion", "neutral")
        if emotion not in EMOTIONS:
            raise ContractError(f"segments[{i}].emotion must be one of {', '.join(EMOTIONS)}")
        total += len(text)
        segments.append(Segment(key=key, text=text, emotion=emotion))
    if total > MAX_BATCH_CHARS:
        raise ContractError(f"batch is {total} characters, limit {MAX_BATCH_CHARS}")
    speed = data.get("speed", 1.0)
    if not isinstance(speed, (int, float)) or not 0.5 <= float(speed) <= 2.0:
        raise ContractError("speed must be between 0.5 and 2.0")
    bitrate = data.get("bitrate_kbps", 48)
    if bitrate not in (24, 32, 48, 64, 96):
        raise ContractError("bitrate_kbps must be one of 24, 32, 48, 64, 96")
    return RenderRequest(engine=engine, voice=voice, segments=tuple(segments), speed=float(speed), bitrate_kbps=bitrate)


def parse_design(raw: str) -> DesignRequest:
    data = _obj(raw)
    key = data.get("key")
    if not isinstance(key, str) or not REF_KEY_RE.match(key):
        raise ContractError("key must be an R2 key under voices/")
    return DesignRequest(
        description=_text(data.get("description"), "description", 600),
        sample_text=_text(data.get("sample_text"), "sample_text", 600),
        key=key,
    )
