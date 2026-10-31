"""The four Space endpoints as plain functions of one JSON string. app.py binds them to Gradio and turns
`EndpointFailure` into `gr.Error`; everything else is testable without Gradio.

The render endpoints are generators (Gradio streams each yield as a `generating` SSE event): one EngineItemResult JSON
per finished item, in request order, then the full EngineResponse JSON, which Gradio also sends as the `complete`
event. A client that only reads `complete` sees exactly the contract v1 response."""
from __future__ import annotations

import traceback
from collections.abc import Callable, Iterator

from . import contracts
from .errors import FailureKind, classify
from .service import Service

Handler = Callable[[str], str]


class EndpointFailure(Exception):
    """An endpoint failed. `message` is the exact text for the Gradio error; `original` is set when the original
    exception must be re-raised unchanged (ZeroGPU scheduler errors)."""

    def __init__(self, kind: FailureKind, message: str, original: BaseException | None = None):
        super().__init__(message)
        self.kind, self.message, self.original = kind, message, original


class Endpoints:
    def __init__(self, service: Service, passthrough: tuple[type[BaseException], ...] = ()):
        self.service = service
        self.passthrough = passthrough

    def render_cpu(self, raw: str) -> Iterator[str]:
        return self._stream(lambda: self.service.stream_cpu(contracts.parse_engine_request(raw)))

    def render_gpu(self, raw: str) -> Iterator[str]:
        return self._stream(lambda: self.service.stream_gpu(contracts.parse_engine_request(raw)))

    def prepare_reference(self, raw: str) -> str:
        return self._call(
            lambda: self.service.prepare_reference(contracts.parse_prepare_reference_request(raw)).to_json())

    def health(self, _raw: str = "") -> str:
        """Takes no input (any string is ignored) and never touches the GPU."""
        return self._call(lambda: self.service.health().to_json())

    def _call(self, fn: Callable[[], str]) -> str:
        try:
            return fn()
        except Exception as e:
            raise self._failure(e) from e

    def _stream(self, fn: Callable[[], Iterator[contracts.EngineItemResult | contracts.EngineResponse]]
                ) -> Iterator[str]:
        try:
            for event in fn():
                yield event.to_json()
        except Exception as e:
            raise self._failure(e) from e

    def _failure(self, e: Exception) -> EndpointFailure:
        kind, message = classify(e, self.passthrough)
        if kind is FailureKind.ENGINE:
            traceback.print_exc()
        return EndpointFailure(kind, message, e if kind is FailureKind.PASSTHROUGH else None)
