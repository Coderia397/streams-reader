import json

import pytest

from nolabs_engine import contracts as c

KEY = "a" * 64


def req(**over):
    base = {"v": 1, "engine": "kokoro", "voice": {"kind": "preset", "id": "af_heart"},
            "segments": [{"key": KEY, "text": "Hello there."}]}
    base.update(over)
    return json.dumps(base)


def test_parses_minimal_render():
    r = c.parse_render(req())
    assert r.engine == "kokoro" and r.voice.id == "af_heart"
    assert r.segments[0].emotion == "neutral" and r.speed == 1.0 and r.bitrate_kbps == 48


@pytest.mark.parametrize("over, msg", [
    ({"v": 2}, "contract version"),
    ({"engine": "fish"}, "engine"),
    ({"segments": []}, "segments"),
    ({"segments": [{"key": "xyz", "text": "a"}]}, "64 lowercase hex"),
    ({"segments": [{"key": KEY, "text": "  "}]}, "non-empty"),
    ({"segments": [{"key": KEY, "text": "a", "emotion": "smug"}]}, "emotion"),
    ({"speed": 3}, "speed"),
    ({"bitrate_kbps": 40}, "bitrate"),
    ({"voice": {"kind": "reference", "ref_key": "voices/me.wav", "transcript": "hi"}}, "kokoro only has preset"),
])
def test_rejects_bad_render(over, msg):
    with pytest.raises(c.ContractError, match=msg):
        c.parse_render(req(**over))


def test_reference_voice_key_is_confined_to_voices_prefix():
    bad = {"kind": "reference", "ref_key": "../audio/x.wav", "transcript": "hi"}
    with pytest.raises(c.ContractError, match="under voices/"):
        c.parse_render(req(engine="ming", voice=bad))
    ok = {"kind": "reference", "ref_key": "voices/u1/ref.wav", "transcript": "hi"}
    assert c.parse_render(req(engine="ming", voice=ok)).voice.ref_key == "voices/u1/ref.wav"


def test_duplicate_keys_are_rendered_once():
    segs = [{"key": KEY, "text": "One."}, {"key": KEY, "text": "One."}]
    assert len(c.parse_render(req(segments=segs)).segments) == 1


def test_batch_character_limit():
    segs = [{"key": f"{i:064x}", "text": "x" * 1000} for i in range(30)]
    with pytest.raises(c.ContractError, match="limit"):
        c.parse_render(req(segments=segs))


def test_response_json_shape():
    out = c.RenderResponse(results=[c.SegmentResult(key=KEY, object_key=c.audio_object_key(KEY), duration_ms=10)],
                           engine="kokoro", elapsed_ms=5)
    data = json.loads(out.to_json())
    assert data["v"] == 1 and data["results"][0]["object_key"] == f"audio/{KEY}.ogg"
