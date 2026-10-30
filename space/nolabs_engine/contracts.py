"""Contract v1 between the Worker and this Space, mirroring `crates/core/src/proto/engine.rs` field for field.

Every endpoint takes one JSON string and returns one JSON string. Parsing is strict: serde defaults are honoured,
unknown fields are rejected, R2 keys must have exactly the shapes in notes/09 (so a bad request can never write outside
its own objects), and each item's mode must fit the engine and voice. `to_dict()` produces exactly what serde produces.

Text lengths and word-timing char offsets are Python `str` indices, which are Unicode code points: the same unit as Rust
`char` offsets in `crates/core`.
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any

from . import voices
from ._reader import U64_MAX, Obj, dumps, load_json
from .errors import ContractError

CONTRACT_VERSION = 1

ENGINES = ("kokoro", "ming")
MODES = ("kokoro", "ip", "tts", "emotion")
EMOTIONS = ("neutral", "happy", "sad", "angry", "fearful", "disgusted", "surprised")
BOUNDARIES = ("sentence", "paragraph", "section")
OPUS_SAMPLE_RATES = (8000, 12000, 16000, 24000, 48000)

MAX_ITEMS = 64
# Segments are at most 320 (Kokoro) or 500 (Ming) chars, plus the segmenter's 5/4 merge allowance; this leaves room.
MAX_ITEM_CHARS = 1000
MAX_BATCH_CHARS = 24_000
MAX_VOICE_TRANSCRIPT_CHARS = 1000
MAX_SCRIPT_CHARS = 4000

# Opaque ids from crates/core/src/ids.rs: 16 chars of lowercase Crockford base32.
_ID = r"[0-9a-hjkmnp-tv-z]{16}"
ID_RE = re.compile(_ID)
KEY_RE = re.compile(r"[0-9a-f]{64}")
OBJECT_KEY_RE = re.compile(r"audio/(kokoro|ming)/([0-9a-f]{64})\.webm")
REF_KEY_RE = re.compile(rf"voices/({_ID})/ref\.wav")
RAW_KEY_RE = re.compile(rf"voices/({_ID})/raw\.wav")
MARKER_KEY_RE = re.compile(rf"jobs/({_ID})\.json")
SHA256_RE = re.compile(r"[0-9a-f]{64}")


def audio_object_key(engine: str, key: str) -> str:
    """Mirror of `nolabs_core::cache_key::audio_object_key`."""
    return f"audio/{engine}/{key}.webm"


def sidecar_key(object_key: str) -> str:
    return f"{object_key}.json"


# ------------------------------------------------------------------------------------------------------------- request


@dataclass(frozen=True)
class PresetVoice:
    id: str
    kind: str = "preset"

    def to_dict(self) -> dict[str, Any]:
        return {"kind": "preset", "id": self.id}


@dataclass(frozen=True)
class ReferenceVoice:
    id: str
    ref_key: str
    transcript: str
    kind: str = "reference"

    def to_dict(self) -> dict[str, Any]:
        return {"kind": "reference", "id": self.id, "ref_key": self.ref_key, "transcript": self.transcript}


EngineVoice = PresetVoice | ReferenceVoice


@dataclass(frozen=True)
class EngineItem:
    key: str
    text: str
    mode: str
    object_key: str
    emotion: str = "neutral"
    boundary: str = "sentence"

    def to_dict(self) -> dict[str, Any]:
        return {"key": self.key, "text": self.text, "mode": self.mode, "emotion": self.emotion,
                "boundary": self.boundary, "object_key": self.object_key}


@dataclass(frozen=True)
class PauseProfile:
    sentence_ms: int
    paragraph_ms: int
    section_ms: int

    def for_boundary(self, boundary: str) -> int:
        return {"sentence": self.sentence_ms, "paragraph": self.paragraph_ms, "section": self.section_ms}[boundary]

    def to_dict(self) -> dict[str, Any]:
        return {"sentence_ms": self.sentence_ms, "paragraph_ms": self.paragraph_ms, "section_ms": self.section_ms}


@dataclass(frozen=True)
class AudioProfile:
    id: int
    bitrate_kbps: int
    sample_rate: int
    target_lufs: float
    max_correction_db: float
    peak_ceiling_dbfs: float
    pauses: PauseProfile

    def to_dict(self) -> dict[str, Any]:
        return {"id": self.id, "bitrate_kbps": self.bitrate_kbps, "sample_rate": self.sample_rate,
                "target_lufs": self.target_lufs, "max_correction_db": self.max_correction_db,
                "peak_ceiling_dbfs": self.peak_ceiling_dbfs, "pauses": self.pauses.to_dict()}


@dataclass(frozen=True)
class EngineRequest:
    job_id: str
    engine: str
    voice: EngineVoice
    items: tuple[EngineItem, ...]
    audio: AudioProfile
    marker_key: str
    v: int = CONTRACT_VERSION

    def total_chars(self) -> int:
        return sum(len(item.text) for item in self.items)

    def to_dict(self) -> dict[str, Any]:
        return {"v": self.v, "job_id": self.job_id, "engine": self.engine, "voice": self.voice.to_dict(),
                "items": [i.to_dict() for i in self.items], "audio": self.audio.to_dict(),
                "marker_key": self.marker_key}


@dataclass(frozen=True)
class PrepareReferenceRequest:
    raw_key: str
    ref_key: str
    transcript: str
    v: int = CONTRACT_VERSION

    def to_dict(self) -> dict[str, Any]:
        return {"v": self.v, "raw_key": self.raw_key, "ref_key": self.ref_key, "transcript": self.transcript}


# ------------------------------------------------------------------------------------------------------------ response


@dataclass(frozen=True)
class EngineItemError:
    code: str
    message: str

    def to_dict(self) -> dict[str, Any]:
        return {"code": self.code, "message": self.message}


@dataclass(frozen=True)
class EngineItemResult:
    key: str
    ok: bool
    object_key: str = ""
    duration_ms: int = 0
    bytes: int = 0
    # [start_ms, end_ms, char_start, char_end] per word.
    words: tuple[tuple[int, int, int, int], ...] = ()
    error: EngineItemError | None = None

    @staticmethod
    def failed(key: str, code: str, message: str) -> EngineItemResult:
        return EngineItemResult(key=key, ok=False, error=EngineItemError(code=code, message=message[:500]))

    def to_dict(self) -> dict[str, Any]:
        out: dict[str, Any] = {"key": self.key, "ok": self.ok, "object_key": self.object_key,
                               "duration_ms": self.duration_ms, "bytes": self.bytes,
                               "words": [list(w) for w in self.words]}
        if self.error is not None:
            out["error"] = self.error.to_dict()
        return out

    def to_json(self) -> str:
        return dumps(self.to_dict())


@dataclass(frozen=True)
class EngineResponse:
    job_id: str
    engine: str
    items: tuple[EngineItemResult, ...]
    elapsed_ms: int
    gpu_ms: int = 0
    v: int = CONTRACT_VERSION

    def to_dict(self) -> dict[str, Any]:
        return {"v": self.v, "job_id": self.job_id, "engine": self.engine,
                "items": [i.to_dict() for i in self.items], "elapsed_ms": self.elapsed_ms, "gpu_ms": self.gpu_ms}

    def to_json(self) -> str:
        return dumps(self.to_dict())


@dataclass(frozen=True)
class PrepareReferenceResponse:
    ref_key: str
    transcript: str
    duration_ms: int
    sha256: str
    v: int = CONTRACT_VERSION

    def to_dict(self) -> dict[str, Any]:
        return {"v": self.v, "ref_key": self.ref_key, "transcript": self.transcript,
                "duration_ms": self.duration_ms, "sha256": self.sha256}

    def to_json(self) -> str:
        return dumps(self.to_dict())


@dataclass(frozen=True)
class EngineHealth:
    engines: dict[str, str]
    storage_ok: bool
    model_revs: dict[str, str] = field(default_factory=dict)
    v: int = CONTRACT_VERSION

    def to_dict(self) -> dict[str, Any]:
        return {"v": self.v, "engines": dict(sorted(self.engines.items())), "storage_ok": self.storage_ok,
                "model_revs": dict(sorted(self.model_revs.items()))}

    def to_json(self) -> str:
        return dumps(self.to_dict())


# ------------------------------------------------------------------------------------------------------------- parsing


def _version(o: Obj) -> int:
    v = o.raw("v")
    if v != CONTRACT_VERSION or isinstance(v, bool):
        raise ContractError(f"contract version {v!r} not supported (expected {CONTRACT_VERSION})")
    return CONTRACT_VERSION


def _voice(o: Obj, engine: str) -> EngineVoice:
    kind = o.choice("kind", ("preset", "reference"))
    if kind == "preset":
        voice_id = o.text("id", nonblank=True, max_chars=64)
        o.done()
        known = voices.KOKORO_VOICES if engine == "kokoro" else voices.MING_PRESETS
        if voice_id not in known:
            raise ContractError(f"voice.id {voice_id!r} is not a {engine} preset")
        return PresetVoice(id=voice_id)
    if engine == "kokoro":
        raise ContractError("the kokoro engine only has preset voices")
    voice_id = o.text("id", pattern=ID_RE)
    ref_key = o.text("ref_key", pattern=REF_KEY_RE)
    transcript = o.text("transcript", nonblank=True, max_chars=MAX_VOICE_TRANSCRIPT_CHARS)
    o.done()
    if ref_key != f"voices/{voice_id}/ref.wav":
        raise ContractError(f"voice.ref_key must be voices/{voice_id}/ref.wav")
    return ReferenceVoice(id=voice_id, ref_key=ref_key, transcript=transcript)


def _check_mode(where: str, engine: str, voice: EngineVoice, mode: str, emotion: str) -> None:
    """Which (engine, voice, mode, emotion) combinations exist. Ming rejects controls that belong to another task
    (vendor/ming3/tts_tasks.py), so each mode carries exactly what its task accepts."""
    if engine == "kokoro":
        if mode != "kokoro":
            raise ContractError(f"{where}.mode must be kokoro for the kokoro engine")
        if emotion != "neutral":
            raise ContractError(f"{where}.emotion must be neutral for the kokoro engine")
        return
    if mode == "kokoro":
        raise ContractError(f"{where}.mode kokoro is not a ming mode")
    if mode == "ip" and not isinstance(voice, PresetVoice):
        raise ContractError(f"{where}.mode ip needs a preset voice")
    if mode == "tts" and not isinstance(voice, ReferenceVoice):
        raise ContractError(f"{where}.mode tts needs a reference voice")
    if mode in ("ip", "tts") and emotion != "neutral":
        raise ContractError(f"{where}.emotion must be neutral in mode {mode}; use mode emotion")
    if mode == "emotion" and emotion == "neutral":
        raise ContractError(f"{where}.mode emotion needs an emotion other than neutral")


def _item(o: Obj, engine: str, voice: EngineVoice) -> EngineItem:
    key = o.text("key", pattern=KEY_RE)
    text = o.text("text", nonblank=True, max_chars=MAX_ITEM_CHARS)
    mode = o.choice("mode", MODES)
    emotion = o.choice("emotion", EMOTIONS, default="neutral")
    boundary = o.choice("boundary", BOUNDARIES, default="sentence")
    object_key = o.text("object_key", pattern=OBJECT_KEY_RE)
    o.done()
    expected = audio_object_key(engine, key)
    if object_key != expected:
        raise ContractError(f"{o.at('object_key')} must be {expected}")
    _check_mode(o.path, engine, voice, mode, emotion)
    return EngineItem(key=key, text=text, mode=mode, emotion=emotion, boundary=boundary, object_key=object_key)


def _audio_profile(o: Obj) -> AudioProfile:
    profile_id = o.integer("id", lo=1)
    bitrate = o.integer("bitrate_kbps", lo=6, hi=510)
    rate = o.integer("sample_rate")
    if rate not in OPUS_SAMPLE_RATES:
        raise ContractError(f"{o.at('sample_rate')} must be one of {OPUS_SAMPLE_RATES}")
    target = o.number("target_lufs", lo=-40.0, hi=-5.0)
    correction = o.number("max_correction_db", lo=0.0, hi=12.0)
    ceiling = o.number("peak_ceiling_dbfs", lo=-20.0, hi=0.0)
    p = o.obj("pauses")
    pauses = PauseProfile(sentence_ms=p.integer("sentence_ms", hi=5000),
                          paragraph_ms=p.integer("paragraph_ms", hi=5000),
                          section_ms=p.integer("section_ms", hi=5000))
    p.done()
    o.done()
    return AudioProfile(id=profile_id, bitrate_kbps=bitrate, sample_rate=rate, target_lufs=target,
                        max_correction_db=correction, peak_ceiling_dbfs=ceiling, pauses=pauses)


def parse_engine_request(raw: str | bytes | dict) -> EngineRequest:
    o = Obj(load_json(raw), "")
    _version(o)
    job_id = o.text("job_id", pattern=ID_RE)
    engine = o.choice("engine", ENGINES)
    voice = _voice(o.obj("voice"), engine)
    raw_items = o.array("items", min_len=1, max_len=MAX_ITEMS)
    items = tuple(_item(Obj(item, f"items[{i}]"), engine, voice) for i, item in enumerate(raw_items))
    audio = _audio_profile(o.obj("audio"))
    marker_key = o.text("marker_key", pattern=MARKER_KEY_RE)
    o.done()
    if marker_key != f"jobs/{job_id}.json":
        raise ContractError(f"marker_key must be jobs/{job_id}.json")
    keys = [item.key for item in items]
    if len(set(keys)) != len(keys):
        raise ContractError("items contain the same key twice")
    req = EngineRequest(job_id=job_id, engine=engine, voice=voice, items=items, audio=audio, marker_key=marker_key)
    if req.total_chars() > MAX_BATCH_CHARS:
        raise ContractError(f"batch is {req.total_chars()} characters, limit {MAX_BATCH_CHARS}")
    return req


def parse_prepare_reference_request(raw: str | bytes | dict) -> PrepareReferenceRequest:
    o = Obj(load_json(raw), "")
    _version(o)
    raw_key = o.text("raw_key", pattern=RAW_KEY_RE)
    ref_key = o.text("ref_key", pattern=REF_KEY_RE)
    transcript = o.text("transcript", nonblank=True, max_chars=MAX_SCRIPT_CHARS)
    o.done()
    voice_id = raw_key.split("/")[1]
    if ref_key != f"voices/{voice_id}/ref.wav":
        raise ContractError(f"ref_key must be voices/{voice_id}/ref.wav, next to raw_key")
    return PrepareReferenceRequest(raw_key=raw_key, ref_key=ref_key, transcript=transcript)


def _words(o: Obj, duration_ms: int) -> tuple[tuple[int, int, int, int], ...]:
    out = []
    for i, word in enumerate(o.array("words", default=[])):
        where = f"{o.at('words')}[{i}]"
        if (not isinstance(word, list) or len(word) != 4
                or not all(isinstance(n, int) and not isinstance(n, bool) and 0 <= n <= 2**32 - 1 for n in word)):
            raise ContractError(f"{where} must be four non-negative integers")
        start, end, char_start, char_end = word
        if start > end or char_start > char_end or end > duration_ms:
            raise ContractError(f"{where} is not an ordered range inside the clip")
        out.append((start, end, char_start, char_end))
    return tuple(out)


def _item_result(o: Obj, engine: str | None) -> EngineItemResult:
    key = o.text("key", pattern=KEY_RE)
    ok = o.boolean("ok")
    object_key = o.text("object_key", default="")
    duration_ms = o.integer("duration_ms", default=0)
    size = o.integer("bytes", hi=U64_MAX, default=0)
    words = _words(o, duration_ms)
    error_raw = o.raw("error", None)
    error = None
    if error_raw is not None:
        e = Obj(error_raw, o.at("error"))
        error = EngineItemError(code=e.text("code", nonblank=True), message=e.text("message"))
        e.done()
    o.done()
    if ok:
        if error is not None:
            raise ContractError(f"{o.at('error')} must be absent when ok is true")
        m = OBJECT_KEY_RE.fullmatch(object_key)
        if m is None or m.group(2) != key or (engine is not None and m.group(1) != engine):
            raise ContractError(f"{o.at('object_key')} must be audio/{engine or '<engine>'}/{key}.webm")
        if duration_ms == 0 or size == 0:
            raise ContractError(f"{o.at('duration_ms')} and bytes must be positive when ok is true")
    elif error is None:
        raise ContractError(f"{o.at('error')} is required when ok is false")
    return EngineItemResult(key=key, ok=ok, object_key=object_key, duration_ms=duration_ms, bytes=size, words=words,
                            error=error)


def parse_item_result(raw: str | bytes | dict) -> EngineItemResult:
    return _item_result(Obj(load_json(raw), ""), None)


def parse_engine_response(raw: str | bytes | dict) -> EngineResponse:
    o = Obj(load_json(raw), "")
    _version(o)
    job_id = o.text("job_id", pattern=ID_RE)
    engine = o.choice("engine", ENGINES)
    items = tuple(_item_result(Obj(item, f"items[{i}]"), engine)
                  for i, item in enumerate(o.array("items", max_len=MAX_ITEMS)))
    elapsed_ms = o.integer("elapsed_ms", hi=U64_MAX)
    gpu_ms = o.integer("gpu_ms", hi=U64_MAX, default=0)
    o.done()
    keys = [item.key for item in items]
    if len(set(keys)) != len(keys):
        raise ContractError("items contain the same key twice")
    return EngineResponse(job_id=job_id, engine=engine, items=items, elapsed_ms=elapsed_ms, gpu_ms=gpu_ms)


def parse_prepare_reference_response(raw: str | bytes | dict) -> PrepareReferenceResponse:
    o = Obj(load_json(raw), "")
    _version(o)
    out = PrepareReferenceResponse(
        ref_key=o.text("ref_key", pattern=REF_KEY_RE),
        transcript=o.text("transcript", nonblank=True, max_chars=MAX_SCRIPT_CHARS),
        duration_ms=o.integer("duration_ms", lo=1),
        sha256=o.text("sha256", pattern=SHA256_RE),
    )
    o.done()
    return out


def parse_health(raw: str | bytes | dict) -> EngineHealth:
    o = Obj(load_json(raw), "")
    _version(o)
    out = EngineHealth(engines=o.string_map("engines"), storage_ok=o.boolean("storage_ok"),
                       model_revs=o.string_map("model_revs", default={}))
    o.done()
    return out
