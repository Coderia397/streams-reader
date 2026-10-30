import json
from types import SimpleNamespace

import numpy as np
import pytest

from nolabs_engine import audio, contracts
from nolabs_engine.contracts import WordTiming
from nolabs_engine.service import Service

K1, K2 = "1" * 64, "2" * 64


class FakeStorage:
    def __init__(self):
        self.objects, self.downloads = {}, []

    def put_audio(self, key, data):
        self.objects[key] = data

    def put_wav(self, key, data):
        self.objects[key] = data

    def download(self, key):
        self.downloads.append(key)
        return f"/tmp/{key.replace('/', '_')}"


class FakeKokoro:
    def synthesize(self, text, voice, speed):
        if "boom" in text:
            raise RuntimeError("kaboom")
        return SimpleNamespace(wave=np.zeros(2400, np.float32), words=[WordTiming(0, 50, 0, 5)])


class FakeMing:
    SAMPLE_RATE = 44_100
    PRESETS = {"US_Emma": ("Emma", "female", "American")}
    ANCHOR_TEXT = "anchor"

    def __init__(self):
        self.jobs = None

    def plan(self, text, emotion, voice_kind, voice_id, ref_path, transcript):
        return SimpleNamespace(text=text, emotion=emotion, kind=voice_kind, ref=ref_path, transcript=transcript)

    def design_job(self, description, sample_text):
        return SimpleNamespace(text=sample_text)

    def run(self, jobs):
        self.jobs = jobs
        return [np.zeros(4410, np.float32) if "bad" not in j.text else "ValueError: no" for j in jobs], {}, 1.5


@pytest.fixture(autouse=True)
def no_ffmpeg(monkeypatch):
    monkeypatch.setattr(audio, "finish", lambda wave, sr, kbps: audio.EncodedAudio(b"ogg", round(wave.size * 1000 / sr)))


def render(engine, voice, texts):
    return contracts.parse_render(json.dumps({"v": 1, "engine": engine, "voice": voice,
                                              "segments": [{"key": k, "text": t} for k, t in texts]}))


def test_kokoro_uploads_each_segment_and_keeps_order_with_errors():
    store = FakeStorage()
    out = Service(store, FakeKokoro(), None).render_kokoro(
        render("kokoro", {"kind": "preset", "id": "af_heart"}, [(K1, "boom here"), (K2, "fine")]))
    assert [r.key for r in out.results] == [K1, K2]
    assert out.results[0].error.startswith("RuntimeError") and not out.results[0].object_key
    assert out.results[1].object_key == f"audio/{K2}.ogg" and out.results[1].duration_ms == 100
    assert out.results[1].words == [[0, 50, 0, 5]]
    assert store.objects == {f"audio/{K2}.ogg": b"ogg"}


def test_ming_reference_voice_downloads_prompt_once_and_reports_gpu_time():
    store, ming = FakeStorage(), FakeMing()
    voice = {"kind": "reference", "ref_key": "voices/me/ref.wav", "transcript": "hello"}
    out = Service(store, None, ming).render_ming(render("ming", voice, [(K1, "one"), (K2, "bad two")]))
    assert store.downloads == ["voices/me/ref.wav"]
    assert all(j.ref.endswith("ref.wav") and j.transcript == "hello" for j in ming.jobs)
    assert out.gpu_ms == 1500 and out.results[0].object_key and out.results[1].error == "ValueError: no"


def test_ming_rejects_unknown_preset():
    with pytest.raises(contracts.ContractError, match="unknown Ming voice"):
        Service(FakeStorage(), None, FakeMing()).render_ming(render("ming", {"kind": "preset", "id": "nobody"},
                                                                    [(K1, "x")]))


def test_engine_missing_is_a_contract_error():
    with pytest.raises(contracts.ContractError, match="not loaded"):
        Service(FakeStorage(), None, None).render_kokoro(render("kokoro", {"kind": "preset", "id": "af_heart"},
                                                                [(K1, "x")]))


def test_normalise_matches_target_loudness_and_ceiling():
    import pyloudnorm

    sr = 24_000
    t = np.arange(sr * 2) / sr
    quiet = (0.01 * np.sin(2 * np.pi * 220 * t)).astype(np.float32)
    out = audio.normalise(quiet, sr)
    assert abs(pyloudnorm.Meter(sr).integrated_loudness(out.astype(np.float64)) - audio.TARGET_LUFS) < 0.6
    assert np.abs(out).max() <= audio.PEAK_CEILING + 1e-6 and out[0] == 0.0
