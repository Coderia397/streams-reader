import numpy as np
import pytest
import soundfile as sf
from conftest import K1, K2, K3, item, ming_request, reference_voice

from nolabs_engine import contracts as c
from nolabs_engine.engines import ming_plan
from nolabs_engine.engines.ming import GpuAnchor, GpuDone, GpuWave, MingDone, MingEngine
from nolabs_engine.engines.ming_plan import ANCHOR_TEXT, GpuPolicy
from nolabs_engine.errors import ContractError

SR = 44_100


def request(*items, voice=None) -> c.EngineRequest:
    return c.parse_engine_request(ming_request(*items, voice=voice))


# ------------------------------------------------------------------------------------------------------------ plan


def test_plan_maps_modes_to_ming_tasks():
    req = request(item(K1, "plain", "ip", "ming"), item(K2, "cheer", "emotion", "ming", emotion="happy"))
    ip, emotion = ming_plan.plan_batch(req.items, req.voice, "", {})
    assert (ip.task, ip.controls) == ("IP", {"ip": "US_Emma"})
    assert (emotion.task, emotion.controls, emotion.prompt) == ("EMOTION", {"emotion": "高兴"}, "anchor:US_Emma")
    assert emotion.anchor_voice == "US_Emma" and ip.anchor_voice is None
    cached = ming_plan.plan(req.items[1], req.voice, "", {"US_Emma": "/tmp/a.wav"})
    assert cached.prompt == "/tmp/a.wav" and cached.anchor_voice is None


def test_plan_reference_voice_uses_prompt_and_transcript():
    req = request(item(K1, "Hi [laughter].", "tts", "ming"), item(K2, "No!", "emotion", "ming", emotion="angry"),
                  voice=reference_voice())
    tts, emotion = ming_plan.plan_batch(req.items, req.voice, "/refs/abc.wav", {})
    assert (tts.task, tts.prompt, tts.prompt_text) == ("TTS", "/refs/abc.wav", "A quiet hour before dawn.")
    assert (emotion.task, emotion.prompt, emotion.controls) == ("EMOTION", "/refs/abc.wav", {"emotion": "愤怒"})


def test_gpu_seconds_follow_the_measured_policy():
    policy = GpuPolicy()
    assert (policy.base_s, policy.s_per_char, policy.max_s) == (8.0, 0.06, 180)
    assert policy.request(0) == 8 and policy.request(500) == 38 and policy.request(501) == 39  # 38.06 rounds up
    assert policy.request(10_000) == 180
    jobs = [ming_plan.Job("EMOTION", "x" * 100, prompt="anchor:US_Emma"),
            ming_plan.Job("EMOTION", "y" * 100, prompt="anchor:US_Emma")]
    # The anchor is spoken once per preset in a call.
    assert ming_plan.request_seconds(policy, jobs) == policy.request(200 + len(ANCHOR_TEXT))


def test_policy_reads_env():
    policy = GpuPolicy.from_env({"MING_BASE_SECONDS": "10", "MING_SECONDS_PER_CHAR": "0.1", "MING_MAX_SECONDS": "90"})
    assert policy == GpuPolicy(10.0, 0.1, 90)
    assert GpuPolicy.from_env({}) == GpuPolicy()


# ---------------------------------------------------------------------------------------------------------- engine


class FakeGpu:
    def __init__(self, error: Exception | None = None, drop: bool = False):
        self.calls: list[list[ming_plan.Job]] = []
        self.error, self.drop = error, drop

    def __call__(self, jobs):
        self.calls.append(jobs)
        if self.error is not None:
            raise self.error
        return self._events(jobs)

    def _events(self, jobs):
        anchored = set()
        for i, j in enumerate(jobs[:-1] if self.drop else jobs):
            if j.anchor_voice and j.anchor_voice not in anchored:
                anchored.add(j.anchor_voice)
                yield GpuAnchor(j.anchor_voice, np.full(SR, 0.1, np.float32))
            yield GpuWave(i, np.full(SR, 0.2, np.float32) if "bad" not in j.text else "ValueError: no")
        yield GpuDone(2.5)


def render(engine, req):
    """Drain the stream into (waves, gpu_s)."""
    waves, gpu_s = [], None
    for event in engine.render_stream(req.items, req.voice, ""):
        if isinstance(event, MingDone):
            gpu_s = event.gpu_s
        else:
            assert event.index == len(waves) and event.sample_rate == SR
            waves.append(event.wave)
    return waves, gpu_s


def test_engine_caches_anchors_on_the_cpu_side(tmp_path):
    gpu = FakeGpu()
    engine = MingEngine(gpu, SR, GpuPolicy(), anchor_dir=str(tmp_path))
    req = request(item(K1, "first", "emotion", "ming", emotion="sad"))
    _, gpu_s = render(engine, req)
    assert gpu_s == 2.5 and gpu.calls[0][0].prompt == "anchor:US_Emma"
    path = engine.anchors["US_Emma"]
    wave, rate = sf.read(path, dtype="float32")
    assert rate == SR and wave.size == SR
    render(engine, req)
    assert gpu.calls[1][0].prompt == path  # second call reuses the anchor, so it is not rendered (or paid for) again


def test_engine_keeps_per_item_errors_and_checks_counts(tmp_path):
    engine = MingEngine(FakeGpu(), SR, anchor_dir=str(tmp_path))
    req = request(item(K1, "ok", "ip", "ming"), item(K2, "bad one", "ip", "ming"))
    assert render(engine, req)[0][1] == "ValueError: no"
    broken = MingEngine(FakeGpu(drop=True), SR, anchor_dir=str(tmp_path))
    with pytest.raises(RuntimeError, match="returned 1 clips for 2 items"):
        render(broken, req)


def test_engine_refuses_batches_longer_than_one_call(tmp_path):
    engine = MingEngine(FakeGpu(), SR, GpuPolicy(max_s=20), anchor_dir=str(tmp_path))
    req = request(item(K1, "x" * 150, "ip", "ming"), item(K2, "y" * 50, "ip", "ming"), item(K3, "z", "ip", "ming"))
    with pytest.raises(ContractError, match="more than the 20 s"):
        render(engine, req)


def test_zerogpu_errors_propagate_unchanged(tmp_path):
    error = RuntimeError("You have exceeded your ZeroGPU runs limit. ")
    engine = MingEngine(FakeGpu(error=error), SR, anchor_dir=str(tmp_path))
    req = request(item(K1, "x", "ip", "ming"))
    with pytest.raises(RuntimeError) as caught:
        render(engine, req)
    assert caught.value is error
