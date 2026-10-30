"""The four Space endpoints as plain functions from one JSON string to one JSON string. app.py binds them to Gradio and
turns `EndpointFailure` into `gr.Error`; everything else is testable without Gradio."""
from __future__ import annotations

import traceback
from collections.abc import Callable

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

    def render_cpu(self, raw: str) -> str:
        return self._call(lambda: self.service.render_cpu(contracts.parse_engine_request(raw)).to_json())

    def render_gpu(self, raw: str) -> str:
        return self._call(lambda: self.service.render_gpu(contracts.parse_engine_request(raw)).to_json())

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
            kind, message = classify(e, self.passthrough)
            if kind is FailureKind.ENGINE:
                traceback.print_exc()
            raise EndpointFailure(kind, message, e if kind is FailureKind.PASSTHROUGH else None) from e
