import json

import pytest
from conftest import kokoro_request

from nolabs_engine import contracts as c
from nolabs_engine.errors import FailureKind, classify, is_zerogpu_message
from nolabs_engine.handlers import EndpointFailure, Endpoints
from nolabs_engine.revisions import MODEL_REVS
from nolabs_engine.service import Service


class GradioError(Exception):
    """Stands in for gr.Error."""


class Storage:
    def ping(self):
        return True


class ExplodingKokoro:
    def __init__(self, error):
        self.error = error

    def synthesize(self, text, voice):
        raise self.error


class FailingService(Service):
    def __init__(self, error):
        super().__init__(Storage(), None, None, statuses={"kokoro": "ready"})
        self.error = error

    def stream_cpu(self, req):
        raise self.error


def endpoints(error=None) -> Endpoints:
    svc = FailingService(error) if error else Service(Storage(), None, None, statuses={"kokoro": "ready"})
    return Endpoints(svc, passthrough=(GradioError,))


def failure(call) -> EndpointFailure:
    with pytest.raises(EndpointFailure) as caught:
        call()
    return caught.value


def test_bad_json_is_a_bad_request():
    f = failure(lambda: list(endpoints().render_cpu("{nope")))
    assert f.kind is FailureKind.BAD_REQUEST and f.message.startswith("bad request: request is not valid JSON")


def test_contract_violation_is_a_bad_request():
    f = failure(lambda: list(endpoints().render_gpu(json.dumps(kokoro_request(v=2)))))
    assert f.message == "bad request: contract version 2 not supported (expected 1)"


def test_unexpected_exception_is_an_engine_error():
    f = failure(lambda: list(endpoints(KeyError("x")).render_cpu(json.dumps(kokoro_request()))))
    assert f.kind is FailureKind.ENGINE and f.message == "engine error: KeyError: 'x'" and f.original is None


def test_gradio_errors_pass_through_as_the_same_object():
    original = GradioError("You have exceeded your ZeroGPU runs limit. ")
    f = failure(lambda: list(endpoints(original).render_cpu(json.dumps(kokoro_request()))))
    assert f.kind is FailureKind.PASSTHROUGH and f.original is original


@pytest.mark.parametrize("message", [
    "You have exceeded your ZeroGPU runs limit. ",
    "You have exceeded your free ZeroGPU quota (90s requested vs. 30s left). Try again in 1:02:03. ",
    "The requested GPU duration (300s) is larger than the maximum allowed",
    "No GPU is currently available for you after 60s",
])
def test_zerogpu_texts_pass_through_unwrapped(message):
    assert is_zerogpu_message(message)
    kind, text = classify(RuntimeError(message))
    assert kind is FailureKind.PASSTHROUGH and text == message


def test_engine_missing_maps_to_engine_error():
    f = failure(lambda: list(endpoints().render_cpu(json.dumps(kokoro_request()))))
    assert f.message == "engine error: EngineUnavailable: kokoro is not loaded (ready)"


def test_health_ignores_input_and_matches_contract():
    out = c.parse_health(endpoints().health("anything"))
    assert out.storage_ok and out.model_revs == MODEL_REVS and out.engines == {"kokoro": "ready"}


def test_prepare_reference_validates_before_touching_storage():
    f = failure(lambda: endpoints().prepare_reference(json.dumps({"v": 1, "raw_key": "x", "ref_key": "y",
                                                                  "transcript": "z"})))
    assert f.kind is FailureKind.BAD_REQUEST and "raw_key" in f.message


def test_render_streams_item_results_then_the_full_response():
    from conftest import K1, K2, item
    from test_service import FakeKokoro, fake_encode

    svc = Service(FakeStorageForStream(), FakeKokoro(), None, statuses={"kokoro": "ready"}, encode=fake_encode)
    body = json.dumps(kokoro_request(item(K1, "one"), item(K2, "two")))
    *items, last = list(Endpoints(svc).render_cpu(body))
    assert [c.parse_item_result(x).key for x in items] == [K1, K2]
    response = c.parse_engine_response(last)
    assert [r.to_json() for r in response.items] == items


class FakeStorageForStream(Storage):
    def put_audio(self, key, data):
        pass

    def put_json(self, key, text):
        pass
