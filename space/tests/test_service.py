import hashlib
import io
import json
import threading
import time
from dataclasses import dataclass

import numpy as np
import pytest
import soundfile as sf
from conftest import JOB, K1, K2, K3, VOICE_ID, item, kokoro_request, ming_request, reference_voice, tone

from nolabs_engine import contracts as c
from nolabs_engine.audio import codec, pipeline
from nolabs_engine.errors import ContractError, EngineUnavailable, InputError
from nolabs_engine.engines.ming import GpuDone, GpuWave, MingClip, MingDone, MingEngine
from nolabs_engine.engines.ming_plan import GpuPolicy
from nolabs_engine.service import Service

SR = 24_000
WAIT_S = 5.0


class FakeStorage:
    def __init__(self, fail_keys: tuple[str, ...] = ()):
        self.writes: list[tuple[str, str, bytes]] = []
        self.objects: dict[str, bytes] = {}
        self.downloads: list[str] = []
        self.fail_keys = fail_keys
        self.ok = True

    def _put(self, kind: str, key: str, data: bytes) -> None:
        if any(key.endswith(k) for k in self.fail_keys):
            raise OSError(f"R2 refused {key}")
        self.writes.append((kind, key, data))
        self.objects[key] = data

    def put_audio(self, key, data):
        self._put("audio", key, data)

    def put_json(self, key, text):
        self._put("json", key, text.encode())

    def put_wav(self, key, data):
        self._put("wav", key, data)

    def get_bytes(self, key):
        return self.objects[key]

    def download(self, key):
        self.downloads.append(key)
        return f"/refs/{key.replace('/', '_')}"

    def ping(self):
        return self.ok


@dataclass
class Synth:
    wave: np.ndarray
    sample_rate: int
    words: tuple


class FakeKokoro:
    """0.5 s of leading silence, 1 s of 'speech', 0.25 s tail; one word timing at 0.6–0.9 s."""

    def __init__(self):
        self.calls = []

    def synthesize(self, text, voice):
        self.calls.append((text, voice))
        if "boom" in text:
            raise RuntimeError("kaboom")
        if "hush" in text:
            return Synth(np.zeros(SR, np.float32), SR, ())
        wave = np.concatenate([np.zeros(SR // 2, np.float32), tone(1.0, SR), np.zeros(SR // 4, np.float32)])
        return Synth(wave, SR, ((600, 900, 0, 4),))


class FakeMing:
    def __init__(self, error: Exception | None = None):
        self.calls = []
        self.error = error

    def render_stream(self, items, voice, ref_path):
        self.calls.append((items, voice, ref_path))
        if self.error is not None:
            raise self.error
        for i, it in enumerate(items):
            yield MingClip(i, tone(1.0, 44_100) if "bad" not in it.text else "ValueError: no", 44_100)
        yield MingDone(1.5)


def fake_encode(wave, source_rate, out_rate, kbps):
    return b"WEBM" + np.asarray(wave, np.float32).tobytes()[:64]


class Clock:
    def __init__(self):
        self.t = 100.0

    def __call__(self):
        self.t += 0.25
        return self.t


def service(storage=None, kokoro=None, ming=None, **kw) -> Service:
    return Service(storage or FakeStorage(), kokoro, ming, encode=fake_encode, clock=Clock(),
                   statuses={"kokoro": "ready", "ming": "failed: no CUDA"}, **kw)


def render_cpu(svc, body) -> c.EngineResponse:
    return svc.render_cpu(c.parse_engine_request(body))


# ------------------------------------------------------------------------------------------------------------ kokoro


def test_kokoro_writes_audio_then_sidecar_per_item_and_marker_last():
    store = FakeStorage()
    body = kokoro_request(item(K1, "Hello world.", boundary="paragraph"), item(K2, "Second one."))
    out = render_cpu(service(store, FakeKokoro()), body)
    assert [r.ok for r in out.items] == [True, True]
    keys = [k for _, k, _ in store.writes]
    for key in (K1, K2):
        audio = keys.index(f"audio/kokoro/{key}.webm")
        assert keys.index(f"audio/kokoro/{key}.webm.json") == audio + 1
    assert keys[-1] == f"jobs/{JOB}.json" and len(keys) == 5
    marker = c.parse_engine_response(store.objects[f"jobs/{JOB}.json"])
    assert marker == out and json.loads(out.to_json())["items"][0]["object_key"] == f"audio/kokoro/{K1}.webm"
    sidecar = c.parse_item_result(store.objects[f"audio/kokoro/{K1}.webm.json"])
    assert sidecar == out.items[0]


def test_kokoro_result_fields_and_shifted_word_timings():
    out = render_cpu(service(kokoro=FakeKokoro()), kokoro_request(item(K1, "Word", boundary="paragraph")))
    result = out.items[0]
    lead_ms = 500 - 30  # 30 ms guard kept before the first loud sample
    speech_ms = 1000 + 2 * 30
    assert result.duration_ms == pytest.approx(speech_ms + 600, abs=1)
    start, end, cs, ce = result.words[0]
    assert start == pytest.approx(600 - lead_ms, abs=1) and end == pytest.approx(900 - lead_ms, abs=1)
    assert (cs, ce) == (0, 4)
    assert result.bytes == 4 + 64  # fake_encode: magic + the first 64 bytes of PCM
    assert out.elapsed_ms > 0 and out.gpu_ms == 0


def test_kokoro_item_failures_are_reported_in_order():
    store = FakeStorage(fail_keys=(f"{K3}.webm",))
    body = kokoro_request(item(K1, "boom"), item(K2, "hush"), item(K3, "fine"))
    out = render_cpu(service(store, FakeKokoro()), body)
    assert [(r.key, r.ok, r.error.code if r.error else None) for r in out.items] == [
        (K1, False, "synthesis_failed"), (K2, False, "silent_output"), (K3, False, "upload_failed")]
    assert "kaboom" in out.items[0].error.message
    assert [k for _, k, _ in store.writes] == [f"jobs/{JOB}.json"]  # nothing but the marker
    c.parse_engine_response(out.to_json())


def test_encoder_failure_is_per_item():
    def broken(wave, *args):
        raise codec.CodecError("ffmpeg failed (1): bad")

    svc = Service(FakeStorage(), FakeKokoro(), None, encode=broken)
    out = svc.render_cpu(c.parse_engine_request(kokoro_request()))
    assert out.items[0].error.code == "encode_failed" and "ffmpeg failed" in out.items[0].error.message


def test_marker_failure_does_not_lose_the_response():
    store = FakeStorage(fail_keys=(f"jobs/{JOB}.json",))
    out = render_cpu(service(store, FakeKokoro()), kokoro_request())
    assert out.items[0].ok and f"jobs/{JOB}.json" not in store.objects


def test_wrong_endpoint_or_missing_engine():
    with pytest.raises(ContractError, match="render_cpu only runs the kokoro engine"):
        service(kokoro=FakeKokoro()).render_cpu(c.parse_engine_request(ming_request()))
    with pytest.raises(ContractError, match="render_gpu only runs the ming engine"):
        service(ming=FakeMing()).render_gpu(c.parse_engine_request(kokoro_request()))
    with pytest.raises(EngineUnavailable, match="ming is not loaded \\(failed: no CUDA\\)"):
        service().render_gpu(c.parse_engine_request(ming_request()))
    with pytest.raises(EngineUnavailable, match="kokoro is not loaded"):
        service().render_cpu(c.parse_engine_request(kokoro_request()))


# --------------------------------------------------------------------------------------------------------- streaming


class GatedKokoro(FakeKokoro):
    """Item i+1 is synthesised only once the consumer holds item i's result, so a test hangs (and fails) if results
    wait for the whole batch."""

    def __init__(self, handed_out: list):
        super().__init__()
        self.handed_out = handed_out

    def synthesize(self, text, voice):
        index = len(self.calls)
        deadline = time.monotonic() + 5
        while index > 0 and len(self.handed_out) < index and time.monotonic() < deadline:
            time.sleep(0.001)
        assert len(self.handed_out) >= index, "item results were held back until later items were synthesised"
        return super().synthesize(text, voice)


def test_stream_cpu_yields_each_item_before_the_next_is_synthesised_then_the_response():
    store, handed_out = FakeStorage(), []
    keys = (K1, K2, K3)
    body = kokoro_request(*(item(k, f"Segment {i}.") for i, k in enumerate(keys)))
    svc = service(store, GatedKokoro(handed_out))
    events = []
    for event in svc.stream_cpu(c.parse_engine_request(body)):
        if isinstance(event, c.EngineItemResult):
            # The audio and its sidecar are in R2 before the result is announced; the marker is not.
            assert f"audio/kokoro/{event.key}.webm.json" in store.objects
            assert f"jobs/{JOB}.json" not in store.objects
            handed_out.append(event)
        events.append(event)
    *items, response = events
    assert [r.key for r in items] == list(keys) and isinstance(response, c.EngineResponse)
    assert response.items == tuple(items) and all(r.ok for r in items)
    assert c.parse_engine_response(store.objects[f"jobs/{JOB}.json"]) == response
    assert [k for _, k, _ in store.writes][-1] == f"jobs/{JOB}.json"


def test_stream_keeps_request_order_when_uploads_finish_out_of_order():
    class SlowFirstStorage(FakeStorage):
        def put_audio(self, key, data):
            if K1 in key:
                time.sleep(0.05)
            super().put_audio(key, data)

    body = kokoro_request(item(K1, "one"), item(K2, "two"), item(K3, "boom"))
    events = list(service(SlowFirstStorage(), FakeKokoro(), max_ahead=3).stream_cpu(c.parse_engine_request(body)))
    assert [e.key for e in events[:-1]] == [K1, K2, K3]
    assert [e.ok for e in events[:-1]] == [True, True, False]


def test_streamed_and_drained_renders_give_the_same_response():
    body = kokoro_request(item(K1, "one", boundary="paragraph"), item(K2, "hush"), item(K3, "three"))
    streamed = list(service(kokoro=FakeKokoro()).stream_cpu(c.parse_engine_request(body)))[-1]
    drained = render_cpu(service(kokoro=FakeKokoro()), body)
    assert streamed.items == drained.items


def test_stream_gpu_yields_items_then_response_with_gpu_time():
    store = FakeStorage()
    body = ming_request(item(K1, "one", "ip", "ming"), item(K2, "bad two", "ip", "ming"))
    events = list(service(store, ming=FakeMing()).stream_gpu(c.parse_engine_request(body)))
    assert [type(e) for e in events] == [c.EngineItemResult, c.EngineItemResult, c.EngineResponse]
    assert events[0].ok and events[1].error.code == "synthesis_failed" and events[2].gpu_ms == 1500


# -------------------------------------------------------------------------------------------------------------- ming


def test_ming_reference_downloads_prompt_and_reports_gpu_time():
    store, ming = FakeStorage(), FakeMing()
    body = ming_request(item(K1, "one", "tts", "ming"), item(K2, "bad two", "emotion", "ming", emotion="sad"),
                        voice=reference_voice())
    out = service(store, ming=ming).render_gpu(c.parse_engine_request(body))
    assert store.downloads == [f"voices/{VOICE_ID}/ref.wav"]
    assert ming.calls[0][2] == f"/refs/voices_{VOICE_ID}_ref.wav"
    assert out.gpu_ms == 1500 and out.items[0].ok and out.items[0].words == ()
    assert out.items[1].error == c.EngineItemError("synthesis_failed", "ValueError: no")
    assert out.items[0].object_key == f"audio/ming/{K1}.webm" and f"audio/ming/{K1}.webm.json" in store.objects


def test_ming_preset_needs_no_download():
    store = FakeStorage()
    service(store, ming=FakeMing()).render_gpu(c.parse_engine_request(ming_request()))
    assert store.downloads == []


def test_zerogpu_error_propagates_and_writes_nothing():
    store = FakeStorage()
    error = RuntimeError("You have exceeded your free ZeroGPU quota (60s requested vs. 12s left). "
                         "Try again in 0:42:00.")
    with pytest.raises(RuntimeError) as caught:
        service(store, ming=FakeMing(error)).render_gpu(c.parse_engine_request(ming_request()))
    assert caught.value is error and store.writes == []


def test_zerogpu_abort_mid_batch_keeps_finished_items_and_writes_no_marker(tmp_path):
    # Item 0 is in R2 with its sidecar before the abort; no marker, and the error keeps its identity (and text).
    store = FakeStorage()
    error = RuntimeError("You have exceeded your free ZeroGPU quota (60s requested vs. 12s left). "
                         "Try again in 0:42:00.")

    def gpu_run(jobs):
        yield GpuWave(0, tone(1.0, 44_100))
        raise error

    ming = MingEngine(gpu_run, 44_100, GpuPolicy(), anchor_dir=str(tmp_path))
    req = c.parse_engine_request(ming_request(item(K1, "one", "ip", "ming"), item(K2, "two", "ip", "ming")))
    events = []
    with pytest.raises(RuntimeError) as caught:
        for event in service(store, ming=ming).stream_gpu(req):
            events.append(event)
    assert caught.value is error
    assert [type(e) for e in events] == [c.EngineItemResult] and events[0].ok
    audio_key = f"audio/ming/{K1}.webm"
    assert [key for _, key, _ in store.writes] == [audio_key, c.sidecar_key(audio_key)]
    assert req.marker_key not in store.objects


class HeldMing(MingEngine):
    """Keeps every generator it hands out alive, as a traceback or a lingering frame would, so only an explicit
    `close()` (never a refcount drop) can end them."""

    def __init__(self, gpu_run, tmp_path):
        self.held: list = []

        def held_gpu_run(jobs):
            run = gpu_run(jobs)
            self.held.append(run)
            return run

        super().__init__(held_gpu_run, 44_100, GpuPolicy(), anchor_dir=str(tmp_path))

    def render_stream(self, items, voice, ref_path):
        events = super().render_stream(items, voice, ref_path)
        self.held.append(events)
        return events


def start_synth(events) -> threading.Thread:
    """Pull the first result (which starts the 'synth' thread) and return that thread."""
    before = set(threading.enumerate())
    assert isinstance(next(events), c.EngineItemResult)
    synth = [t for t in threading.enumerate() if t.name == "synth" and t not in before]
    assert len(synth) == 1
    return synth[0]


def test_closing_stream_gpu_early_closes_the_gpu_generator_on_the_synthesis_thread(tmp_path):
    # spaces stops the ZeroGPU worker only when GeneratorExit reaches its generator; that must happen when the client
    # goes away, not whenever the last reference happens to drop. HeldMing holds every link, so each close is needed.
    closed_on: list[str] = []

    def gpu_run(jobs):
        try:
            for i in range(len(jobs)):
                yield GpuWave(i, tone(1.0, 44_100))
            yield GpuDone(1.0)
        finally:
            closed_on.append(threading.current_thread().name)

    ming = HeldMing(gpu_run, tmp_path)
    req = c.parse_engine_request(ming_request(*(item(k, f"item {k[:4]}", "ip", "ming") for k in (K1, K2, K3))))
    events = service(ming=ming, max_ahead=1).stream_gpu(req)
    synth = start_synth(events)
    events.close()
    synth.join(WAIT_S)
    assert not synth.is_alive()
    assert closed_on == ["synth"]
    assert len(ming.held) == 2 and all(g.gi_frame is None for g in ming.held)


def test_closing_a_stream_early_closes_the_engine_generator_it_was_given():
    # The first link of the chain: `_stream` closes the clip source itself, on the synthesis thread.
    closed_on: list[str] = []

    def clips():
        try:
            while True:
                yield pipeline.RawClip(wave=tone(1.0, SR), sample_rate=SR, boundary="sentence"), ()
        finally:
            closed_on.append(threading.current_thread().name)

    source = clips()  # held here for the whole test, so only an explicit close can run its `finally`
    req = c.parse_engine_request(kokoro_request(item(K1, "one"), item(K2, "two"), item(K3, "three")))
    events = service(max_ahead=1)._stream(req, 0.0, source, lambda: 0)
    synth = start_synth(events)
    events.close()
    synth.join(WAIT_S)
    assert not synth.is_alive()
    assert closed_on == ["synth"] and source.gi_frame is None


# --------------------------------------------------------------------------------------------- prepare reference


SCRIPT = ["The morning light crept slowly over the old stone wall.",
          "She laughed, surprised that anyone had remembered her name.",
          "Nobody answered when I knocked on the heavy wooden door.",
          "Please, just tell me what happened last night."]


def guided_recording() -> np.ndarray:
    parts = [np.zeros(SR // 2, np.float32)]
    for line in SCRIPT:
        parts += [tone(len(line) / 14.0, SR, amplitude=0.2), np.zeros(int(0.6 * SR), np.float32)]
    return np.concatenate(parts)


def prepare_request() -> c.PrepareReferenceRequest:
    return c.parse_prepare_reference_request({"v": 1, "raw_key": f"voices/{VOICE_ID}/raw.wav",
                                              "ref_key": f"voices/{VOICE_ID}/ref.wav",
                                              "transcript": "\n".join(SCRIPT)})


def test_prepare_reference_writes_wav_and_fingerprint():
    store = FakeStorage()
    store.objects[f"voices/{VOICE_ID}/raw.wav"] = b"raw upload"
    decoded = []

    def decode(data, rate, max_s):
        decoded.append((data, rate, max_s))
        return guided_recording()

    out = service(store, decode=decode).prepare_reference(prepare_request())
    assert decoded == [(b"raw upload", 24000, 180.0)]
    wav = store.objects[f"voices/{VOICE_ID}/ref.wav"]
    assert out.sha256 == hashlib.sha256(wav).hexdigest()
    wave, rate = sf.read(io.BytesIO(wav), dtype="float32")
    assert rate == 24000 and out.duration_ms == round(wave.size * 1000 / 24000)
    assert 10_000 <= out.duration_ms <= 18_200
    assert out.transcript in {" ".join(SCRIPT[i:j]) for i in range(4) for j in range(i + 1, 5)}
    c.parse_prepare_reference_response(out.to_json())
    # Deterministic: the same upload gives the same fingerprint.
    again = service(FakeStorage(), decode=decode)
    again.storage.objects[f"voices/{VOICE_ID}/raw.wav"] = b"raw upload"
    assert again.prepare_reference(prepare_request()).sha256 == out.sha256


def test_prepare_reference_input_errors():
    store = FakeStorage()
    store.objects[f"voices/{VOICE_ID}/raw.wav"] = b"x"

    def undecodable(*_):
        raise codec.CodecError("ffmpeg failed (1): Invalid data")

    with pytest.raises(InputError, match="could not be decoded"):
        service(store, decode=undecodable).prepare_reference(prepare_request())
    with pytest.raises(InputError, match="no audio"):
        service(store, decode=lambda *_: np.zeros(0, np.float32)).prepare_reference(prepare_request())


# ------------------------------------------------------------------------------------------------------------ health


def test_health_reports_engines_storage_and_revisions():
    store = FakeStorage()
    store.ok = False
    health = service(store).health()
    assert health.engines == {"kokoro": "ready", "ming": "failed: no CUDA"} and health.storage_ok is False
    assert health.model_revs["kokoro"].startswith("hexgrad/Kokoro-82M@")
    assert c.parse_health(health.to_json()) == health


# ------------------------------------------------------------------------------------------- with a real ffmpeg


def test_render_cpu_end_to_end_with_ffmpeg(ffmpeg):
    store = FakeStorage()
    svc = Service(store, FakeKokoro(), None, statuses={"kokoro": "ready"})
    out = svc.render_cpu(c.parse_engine_request(kokoro_request(item(K1, "Word", boundary="section"))))
    data = store.objects[f"audio/kokoro/{K1}.webm"]
    assert out.items[0].ok and data[:4] == b"\x1a\x45\xdf\xa3" and out.items[0].bytes == len(data)
    back = codec.decode_mono(data, 24000, 60)
    assert abs(back.size / 24 - out.items[0].duration_ms) < 30  # within one Opus frame plus pre-skip


def test_prepare_reference_end_to_end_with_ffmpeg(ffmpeg):
    store = FakeStorage()
    buf = io.BytesIO()
    # Uploaded at 48 kHz, so decoding also resamples to the 24 kHz analysis rate.
    sf.write(buf, np.repeat(guided_recording(), 2), 48000, format="WAV", subtype="PCM_16")
    store.objects[f"voices/{VOICE_ID}/raw.wav"] = buf.getvalue()
    out = Service(store).prepare_reference(prepare_request())
    assert 10_000 <= out.duration_ms <= 18_200
    assert sf.info(io.BytesIO(store.objects[f"voices/{VOICE_ID}/ref.wav"])).samplerate == 24000
