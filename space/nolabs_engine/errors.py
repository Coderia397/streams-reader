"""Error kinds the Space reports, and how each becomes the text of the Gradio error the Worker classifies.

The Worker's `EngineError::classify` (crates/core/src/engine_error.rs) reads these strings, so the prefixes are part of
the contract: `bad request: …` for anything the caller must change, `engine error: …` for failures on our side, and
ZeroGPU scheduler errors passed through with their original text (it parses "runs limit", "quota" and
"Try again in H:MM:SS" out of them).
"""
from __future__ import annotations

import enum

BAD_REQUEST_PREFIX = "bad request: "
ENGINE_ERROR_PREFIX = "engine error: "

# Lowercase substrings of the messages `spaces` 0.51.3 raises from its scheduler (spaces/zero/client.py). They arrive
# as gr.Error already; matching the text as well covers any path that re-raises them as a plain exception.
_ZEROGPU_MARKERS = (
    "zerogpu",
    "gpu quota",
    "requested gpu duration",
    "no gpu is currently available",
    "gpu task aborted",
    "reached its gpu limit",
)


class ContractError(ValueError):
    """The request does not match contract v1. Reported as `bad request: …`."""


class InputError(ContractError):
    """The request is well formed but its content cannot be used (for example a recording with too little speech).
    Also a `bad request`, because only a different input can fix it."""


class EngineUnavailable(RuntimeError):
    """An engine failed to load on this Space. Reported as `engine error: …`, which the Worker retries for CPU jobs."""


class FailureKind(enum.Enum):
    BAD_REQUEST = "bad_request"
    PASSTHROUGH = "passthrough"
    ENGINE = "engine"


def is_zerogpu_message(message: str) -> bool:
    lowered = message.lower()
    return any(marker in lowered for marker in _ZEROGPU_MARKERS)


def classify(exc: BaseException, passthrough: tuple[type[BaseException], ...] = ()) -> tuple[FailureKind, str]:
    """Decide how an endpoint failure is reported. `passthrough` holds exception types that already carry the final
    message (gr.Error on the Space); they and ZeroGPU texts are re-raised unchanged."""
    if isinstance(exc, ContractError):
        return FailureKind.BAD_REQUEST, f"{BAD_REQUEST_PREFIX}{exc}"
    if passthrough and isinstance(exc, passthrough):
        return FailureKind.PASSTHROUGH, str(exc)
    if is_zerogpu_message(str(exc)):
        return FailureKind.PASSTHROUGH, str(exc)
    return FailureKind.ENGINE, f"{ENGINE_ERROR_PREFIX}{type(exc).__name__}: {exc}"
