import contextvars
import random
import threading
import time
from concurrent.futures import ThreadPoolExecutor

import pytest

from nolabs_engine.streaming import ordered_pipeline

WAIT_S = 5.0


@pytest.fixture
def pool():
    with ThreadPoolExecutor(max_workers=4) as executor:
        yield executor


def test_results_keep_source_order_when_stages_finish_out_of_order(pool):
    rng = random.Random(7)
    delays = [rng.uniform(0, 0.02) for _ in range(20)]

    def stage(i):
        time.sleep(delays[i])
        return i * 10

    assert list(ordered_pipeline(range(20), stage, pool, max_ahead=4)) == [i * 10 for i in range(20)]


def test_next_item_is_produced_while_the_previous_stage_runs(pool):
    second_pulled = threading.Event()

    def source():
        yield 0
        second_pulled.set()  # the source was resumed for item 1
        yield 1

    def stage(i):
        # Item 0's stage (encode + upload) only finishes once the source has moved on to item 1: pipelining.
        if i == 0:
            assert second_pulled.wait(WAIT_S), "the source did not run ahead while item 0 was in its stage"
        return i

    assert list(ordered_pipeline(source(), stage, pool, max_ahead=2)) == [0, 1]


def test_first_result_is_handed_out_before_the_source_finishes(pool):
    release = threading.Event()

    def source():
        yield 0
        assert release.wait(WAIT_S)
        yield 1

    out = ordered_pipeline(source(), lambda i: i, pool, max_ahead=2)
    assert next(out) == 0  # would hang if results waited for the whole source
    release.set()
    assert list(out) == [1]


def test_source_runs_at_most_max_ahead_items_in_front_of_the_consumer(pool):
    pulled = []

    def source():
        for i in range(10):
            pulled.append(i)
            yield i

    out = ordered_pipeline(source(), lambda i: i, pool, max_ahead=2)
    assert next(out) == 0
    time.sleep(0.05)
    assert len(pulled) <= 3  # item 0 handed out, at most 2 more pulled
    assert list(out) == list(range(1, 10))


def test_source_error_comes_after_the_items_before_it_and_keeps_its_identity(pool):
    error = RuntimeError("You have exceeded your ZeroGPU runs limit. ")

    def source():
        yield 1
        yield 2
        raise error

    out = ordered_pipeline(source(), lambda i: i, pool, max_ahead=4)
    assert next(out) == 1 and next(out) == 2
    with pytest.raises(RuntimeError) as caught:
        next(out)
    assert caught.value is error


def test_closing_early_stops_the_source_and_runs_its_cleanup(pool):
    pulled, cleaned = [], threading.Event()

    def source():
        try:
            for i in range(100):
                pulled.append(i)
                yield i
        finally:
            cleaned.set()

    out = ordered_pipeline(source(), lambda i: i, pool, max_ahead=1)
    assert next(out) == 0
    out.close()
    assert cleaned.wait(WAIT_S)
    assert len(pulled) <= 2


def test_source_whose_iter_raises_reaches_the_consumer_instead_of_hanging(pool):
    error = TypeError("this source cannot be iterated")

    class Broken:
        def __iter__(self):
            raise error

    outcome: list[BaseException] = []

    def consume():
        try:
            next(ordered_pipeline(Broken(), lambda i: i, pool))
        except BaseException as e:  # noqa: BLE001 - recorded for the assertion
            outcome.append(e)

    consumer = threading.Thread(target=consume, daemon=True)
    consumer.start()
    consumer.join(WAIT_S)
    assert not consumer.is_alive(), "the consumer hung waiting for a producer that had died"
    assert outcome and outcome[0] is error


def test_source_sees_the_callers_context_variables(pool):
    request = contextvars.ContextVar("request", default=None)
    seen = []

    def source():
        seen.append((request.get(), threading.current_thread().name))
        yield 1

    request.set("gradio-request")
    assert list(ordered_pipeline(source(), lambda i: i, pool, name="synth-test")) == [1]
    assert seen == [("gradio-request", "synth-test")]


def test_max_ahead_must_be_positive(pool):
    with pytest.raises(ValueError):
        list(ordered_pipeline([1], lambda i: i, pool, max_ahead=0))
