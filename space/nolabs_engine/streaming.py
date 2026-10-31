"""Ordered two-stage pipelining: pull work from a source on one thread, finish each piece on a pool, hand results out in
source order.

The source is the synthesis loop (Kokoro, or the ZeroGPU generator for Ming) and must stay sequential: the models are
not re-entrant and segment N's loudness depends on segments 0..N. The stage (encode + upload) is what overlaps: while
segment N is encoded and uploaded, segment N+1 is already being synthesised.
"""
from __future__ import annotations

import contextvars
import queue
import threading
from collections.abc import Callable, Iterable, Iterator
from concurrent.futures import Executor, Future
from typing import TypeVar

T = TypeVar("T")
R = TypeVar("R")

_END = object()


class _Fatal:
    """The source itself raised: the stream ends with that exception, unchanged (ZeroGPU errors keep their text)."""

    def __init__(self, error: BaseException):
        self.error = error


def ordered_pipeline(source: Iterable[T], stage: Callable[[T], R], pool: Executor, max_ahead: int = 2,
                     name: str = "synth") -> Iterator[R]:
    """Yield `stage(x)` for every `x` of `source`, in source order.

    `source` is iterated on its own thread (started on the first `next`), inside a copy of the caller's context so
    context variables such as the Gradio request reach ZeroGPU. At most `max_ahead` items are pulled from the source
    but not yet handed out, which bounds memory and keeps the model from racing far ahead of slow uploads. If the
    source raises, items pulled before the error are still handed out, then the error is raised. Closing the iterator
    early stops the source before it pulls another item."""
    if max_ahead < 1:
        raise ValueError("max_ahead must be at least 1")
    slots = threading.Semaphore(max_ahead)
    handoff: queue.SimpleQueue[Future[R] | _Fatal | object] = queue.SimpleQueue()
    stop = threading.Event()

    def produce() -> None:
        items: Iterator[T] | None = None
        try:
            items = iter(source)  # inside the `try`: if iter() raises, the consumer still gets the error, not a hang
            while True:
                slots.acquire()  # before pulling, so at most `max_ahead` items exist ahead of the consumer
                if stop.is_set():
                    return
                try:
                    x = next(items)
                except StopIteration:
                    handoff.put(_END)
                    return
                handoff.put(pool.submit(stage, x))
        except BaseException as e:  # noqa: BLE001 - handed to the consumer, which re-raises it
            handoff.put(_Fatal(e))
        finally:
            close = getattr(items, "close", None) if items is not None else None
            if close is not None:
                close()  # a generator source runs its cleanup (for ZeroGPU: releasing the GPU) on this thread

    context = contextvars.copy_context()
    producer = threading.Thread(target=context.run, args=(produce,), name=name, daemon=True)
    producer.start()
    try:
        while True:
            entry = handoff.get()
            if entry is _END:
                return
            if isinstance(entry, _Fatal):
                raise entry.error
            assert isinstance(entry, Future)
            result = entry.result()
            slots.release()
            yield result
    finally:
        stop.set()
        slots.release()  # wake a producer blocked on a slot so it sees `stop`
