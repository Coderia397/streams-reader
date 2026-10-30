import copy
import json

import pytest
from conftest import JOB, K1, K2, VOICE_ID, dumps, item, kokoro_request, ming_request, reference_voice

from nolabs_engine import contracts as c
from nolabs_engine.errors import ContractError


def parse(body: dict) -> c.EngineRequest:
    return c.parse_engine_request(dumps(body))


# ------------------------------------------------------------------------------------------------- engine request


def test_parses_kokoro_request_with_serde_defaults():
    req = parse(kokoro_request())
    assert req.engine == "kokoro" and req.voice == c.PresetVoice(id="af_heart")
    assert req.items[0].emotion == "neutral" and req.items[0].boundary == "sentence"
    assert req.audio.pauses.for_boundary("paragraph") == 600
    assert req.audio.target_lufs == -16.0 and req.marker_key == f"jobs/{JOB}.json"


def test_parses_ming_reference_request():
    body = ming_request(item(K1, "Well [laughter] fine.", "tts", "ming"),
                        item(K2, "Get out.", "emotion", "ming", emotion="angry", boundary="section"),
                        voice=reference_voice())
    req = parse(body)
    assert isinstance(req.voice, c.ReferenceVoice) and req.voice.ref_key == f"voices/{VOICE_ID}/ref.wav"
    assert [i.mode for i in req.items] == ["tts", "emotion"] and req.items[1].boundary == "section"


def test_accepts_dict_and_bytes():
    body = kokoro_request()
    assert c.parse_engine_request(body) == c.parse_engine_request(dumps(body).encode())


def test_text_is_kept_exactly_and_counted_in_code_points():
    text = "  Café — “naïve” 😀 ok  "
    req = parse(kokoro_request(item(K1, text)))
    assert req.items[0].text == text
    assert req.total_chars() == len(text) == 23  # the emoji is one code point, as in Rust chars()


def test_request_round_trips_through_to_dict():
    req = parse(ming_request(item(K1, "x", "emotion", "ming", emotion="sad"), voice=reference_voice()))
    assert c.parse_engine_request(req.to_dict()) == req


def _with(body: dict, path: str, value) -> dict:
    body = copy.deepcopy(body)
    *parents, last = path.split(".")
    target = body
    for p in parents:
        target = target[int(p)] if p.isdigit() else target[p]
    if value is _DELETE:
        del target[last]
    else:
        target[last] = value
    return body


_DELETE = object()
K_BAD_ENGINE = item(K1, "x", "kokoro", "ming")


@pytest.mark.parametrize("path, value, message", [
    ("v", 2, "contract version"),
    ("v", True, "contract version"),
    ("job_id", "JOB-1", "job_id"),
    ("job_id", "0123456789abcdei", "job_id"),
    ("marker_key", "jobs/zzzzzzzzzzzzzzzz.json", "marker_key must be"),
    ("marker_key", "../jobs/x.json", "marker_key"),
    ("engine", "fish", "engine must be one of"),
    ("voice", {"kind": "preset", "id": "nobody"}, "not a kokoro preset"),
    ("voice", {"kind": "preset", "id": "US_Emma"}, "not a kokoro preset"),
    ("voice", {"kind": "clone", "id": "x"}, "voice.kind"),
    ("voice", reference_voice(), "only has preset voices"),
    ("voice", {"kind": "preset", "id": "af_heart", "speed": 2}, "unknown fields: speed"),
    ("items", [], "items must have 1 to 64"),
    ("items", [item(f"{i:064x}", "x") for i in range(65)], "items must have 1 to 64"),
    ("items.0.key", "A" * 64, "items[0].key"),
    ("items.0.key", "1" * 63, "items[0].key"),
    ("items.0.text", " \n ", "must not be empty"),
    ("items.0.text", "x" * 1001, "limit 1000"),
    ("items.0.text", 5, "must be a string"),
    ("items.0.mode", "ip", "mode must be kokoro"),
    ("items.0.emotion", "happy", "emotion must be neutral for the kokoro engine"),
    ("items.0.emotion", "smug", "emotion must be one of"),
    ("items.0.boundary", "chapter", "boundary must be one of"),
    ("items.0.object_key", f"audio/ming/{K1}.webm", "object_key must be audio/kokoro/"),
    ("items.0.object_key", f"audio/kokoro/{K2}.webm", "object_key must be audio/kokoro/"),
    ("items.0.object_key", f"audio/kokoro/{K1}.ogg", "does not match"),
    ("items.0.object_key", f"voices/{K1}.webm", "does not match"),
    ("items.0.object_key", _DELETE, "object_key is missing"),
    ("items.0.speed", 1.5, "items[0] has unknown fields: speed"),
    ("audio.sample_rate", 44100, "sample_rate must be one of"),
    ("audio.bitrate_kbps", 0, "bitrate_kbps must be between"),
    ("audio.bitrate_kbps", 32.0, "must be an integer"),
    ("audio.target_lufs", -60, "target_lufs must be between"),
    ("audio.target_lufs", "loud", "must be a number"),
    ("audio.max_correction_db", -1, "max_correction_db"),
    ("audio.peak_ceiling_dbfs", 0.5, "peak_ceiling_dbfs"),
    ("audio.pauses.section_ms", 9000, "section_ms must be between"),
    ("audio.pauses.sentence_ms", -1, "sentence_ms must be between"),
    ("audio.id", 0, "audio.id"),
    ("audio.codec", "opus", "audio has unknown fields: codec"),
    ("speed", 1.0, "request has unknown fields: speed"),
])
def test_rejects_invalid_kokoro_request(path, value, message):
    with pytest.raises(ContractError, match=message.replace("[", r"\[").replace("]", r"\]")):
        parse(_with(kokoro_request(), path, value))


@pytest.mark.parametrize("items, voice, message", [
    ([item(K1, "x", "kokoro", "ming")], None, "mode kokoro is not a ming mode"),
    ([item(K1, "x", "tts", "ming")], None, "mode tts needs a reference voice"),
    ([item(K1, "x", "ip", "ming")], reference_voice(), "mode ip needs a preset voice"),
    ([item(K1, "x", "ip", "ming", emotion="happy")], None, "must be neutral in mode ip"),
    ([item(K1, "x", "tts", "ming", emotion="sad")], reference_voice(), "must be neutral in mode tts"),
    ([item(K1, "x", "emotion", "ming")], None, "needs an emotion other than neutral"),
    ([item(K1, "x", "ip", "kokoro")], None, "object_key must be audio/ming/"),
    ([item(K1, "x", "ip", "ming")], {"kind": "preset", "id": "af_heart"}, "not a ming preset"),
    ([item(K1, "x", "tts", "ming")], {**reference_voice(), "ref_key": "voices/zzzzzzzzzzzzzzzz/ref.wav"},
     "ref_key must be voices/0123456789abcdef/ref.wav"),
    ([item(K1, "x", "tts", "ming")], {**reference_voice(), "ref_key": "voices/../ref.wav"}, "does not match"),
    ([item(K1, "x", "tts", "ming")], {**reference_voice(), "id": "user:0123456789abcdef"}, "voice.id"),
    ([item(K1, "x", "tts", "ming")], {**reference_voice(), "transcript": "  "}, "transcript must not be empty"),
    ([item(K1, "x", "ip", "ming"), item(K1, "y", "ip", "ming")], None, "same key twice"),
])
def test_rejects_inconsistent_ming_request(items, voice, message):
    with pytest.raises(ContractError, match=message):
        parse(ming_request(*items, voice=voice))


def test_every_ming_mode_combination_that_exists_is_accepted():
    preset = parse(ming_request(item(K1, "a", "ip", "ming"), item(K2, "b", "emotion", "ming", emotion="fearful")))
    assert [i.mode for i in preset.items] == ["ip", "emotion"]
    ref = parse(ming_request(item(K1, "a", "tts", "ming"), item(K2, "b", "emotion", "ming", emotion="surprised"),
                             voice=reference_voice()))
    assert [i.emotion for i in ref.items] == ["neutral", "surprised"]


def test_batch_character_limit():
    items = [item(f"{i:064x}", "x" * 1000) for i in range(25)]
    with pytest.raises(ContractError, match="limit 24000"):
        parse(kokoro_request(*items))


@pytest.mark.parametrize("raw", ["not json", "[1, 2]", '{"v": NaN}', "", '{"v": 1, "job_id": Infinity}'])
def test_rejects_non_json_and_non_objects(raw):
    with pytest.raises(ContractError):
        c.parse_engine_request(raw)


# ----------------------------------------------------------------------------------------------- prepare reference


def prepare_body(**over) -> dict:
    body = {"v": 1, "raw_key": f"voices/{VOICE_ID}/raw.wav", "ref_key": f"voices/{VOICE_ID}/ref.wav",
            "transcript": "Line one.\nLine two."}
    body.update(over)
    return body


def test_parses_prepare_reference_request():
    req = c.parse_prepare_reference_request(dumps(prepare_body()))
    assert req.transcript == "Line one.\nLine two." and req.to_dict() == prepare_body()


@pytest.mark.parametrize("over, message", [
    ({"raw_key": f"voices/{VOICE_ID}/raw.mp3"}, "raw_key"),
    ({"raw_key": "voices/../../audio/raw.wav"}, "raw_key"),
    ({"ref_key": "voices/zzzzzzzzzzzzzzzz/ref.wav"}, "next to raw_key"),
    ({"ref_key": f"voices/{VOICE_ID}/raw.wav"}, "ref_key"),
    ({"transcript": ""}, "transcript must not be empty"),
    ({"transcript": "x" * 4001}, "limit 4000"),
    ({"v": 0}, "contract version"),
    ({"name": "me"}, "unknown fields: name"),
])
def test_rejects_invalid_prepare_reference_request(over, message):
    with pytest.raises(ContractError, match=message):
        c.parse_prepare_reference_request(dumps(prepare_body(**over)))


# ------------------------------------------------------------------------------------------------------ responses


def ok_result(**over) -> dict:
    body = {"key": K1, "ok": True, "object_key": f"audio/kokoro/{K1}.webm", "duration_ms": 1200, "bytes": 4000,
            "words": [[0, 300, 0, 5]]}
    body.update(over)
    return body


def test_item_result_serialises_like_serde():
    ok = c.EngineItemResult(key=K1, ok=True, object_key=f"audio/kokoro/{K1}.webm", duration_ms=1200, bytes=4000,
                            words=((0, 300, 0, 5),))
    assert json.loads(ok.to_json()) == ok_result()
    failed = c.EngineItemResult.failed(K1, "upload_failed", "boom")
    assert json.loads(failed.to_json()) == {"key": K1, "ok": False, "object_key": "", "duration_ms": 0, "bytes": 0,
                                            "words": [], "error": {"code": "upload_failed", "message": "boom"}}
    assert c.parse_item_result(failed.to_json()) == failed


def test_item_result_defaults_follow_serde():
    parsed = c.parse_item_result({"key": K1, "ok": False, "error": {"code": "x", "message": "y"}})
    assert parsed.object_key == "" and parsed.words == () and parsed.bytes == 0


@pytest.mark.parametrize("over, message", [
    ({"ok": False}, "error is required"),
    ({"error": {"code": "x", "message": "y"}}, "must be absent"),
    ({"object_key": f"audio/kokoro/{K2}.webm"}, "object_key must be"),
    ({"duration_ms": 0, "words": []}, "must be positive"),
    ({"words": [[0, 300, 0]]}, "four non-negative integers"),
    ({"words": [[300, 0, 0, 5]]}, "ordered range"),
    ({"words": [[0, 1300, 0, 5]]}, "ordered range"),
    ({"words": [[0, 10, 5, 4]]}, "ordered range"),
    ({"words": [[0, 10, -1, 4]]}, "four non-negative integers"),
])
def test_rejects_inconsistent_item_result(over, message):
    with pytest.raises(ContractError, match=message):
        c.parse_item_result(ok_result(**over))


def test_engine_response_checks_engine_prefix_of_object_keys():
    body = {"v": 1, "job_id": JOB, "engine": "ming", "items": [ok_result()], "elapsed_ms": 10}
    with pytest.raises(ContractError, match="audio/ming/"):
        c.parse_engine_response(body)
    body["engine"] = "kokoro"
    parsed = c.parse_engine_response(body)
    assert parsed.gpu_ms == 0 and json.loads(parsed.to_json()) == {**body, "gpu_ms": 0}


def test_health_and_prepare_response_round_trip():
    health = c.EngineHealth(engines={"ming": "ready", "kokoro": "failed: x"}, storage_ok=False,
                            model_revs={"kokoro": "hexgrad/Kokoro-82M@abc"})
    assert c.parse_health(health.to_json()) == health
    assert c.parse_health({"v": 1, "engines": {}, "storage_ok": True}).model_revs == {}
    with pytest.raises(ContractError, match="engines"):
        c.parse_health({"v": 1, "engines": {"kokoro": 1}, "storage_ok": True})
    prepared = c.PrepareReferenceResponse(ref_key=f"voices/{VOICE_ID}/ref.wav", transcript="Hi there.",
                                          duration_ms=12000, sha256="ab" * 32)
    assert c.parse_prepare_reference_response(prepared.to_json()) == prepared
