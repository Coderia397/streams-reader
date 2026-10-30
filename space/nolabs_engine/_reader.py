"""Strict reading of JSON objects for contracts.py: typed field access with paths in error messages, serde-style
defaults, and rejection of unknown fields (a misspelt field is a bug on the other side, not something to ignore)."""
from __future__ import annotations

import json
import math
import re
from typing import Any

from .errors import ContractError

U32_MAX = 2**32 - 1
U64_MAX = 2**64 - 1

_MISSING: Any = object()


def load_json(raw: str | bytes | dict) -> Any:
    """Parse a JSON document. NaN and Infinity are rejected because serde_json never produces or accepts them."""
    if isinstance(raw, dict):
        return raw
    try:
        return json.loads(raw, parse_constant=_reject_constant)
    except (TypeError, ValueError) as e:
        raise ContractError(f"request is not valid JSON: {e}") from None


def _reject_constant(name: str) -> Any:
    raise ValueError(f"{name} is not valid JSON")


def dumps(value: Any) -> str:
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False, allow_nan=False)


class Obj:
    """One JSON object being read. Call `done()` after reading every field to reject the ones nobody read."""

    def __init__(self, data: Any, path: str):
        if not isinstance(data, dict):
            raise ContractError(f"{path} must be a JSON object")
        self._data, self._path, self._seen = data, path, set()

    @property
    def path(self) -> str:
        return self._path or "request"

    def at(self, name: str) -> str:
        return f"{self._path}.{name}" if self._path else name

    def raw(self, name: str, default: Any = _MISSING) -> Any:
        self._seen.add(name)
        if name in self._data:
            return self._data[name]
        if default is _MISSING:
            raise ContractError(f"{self.at(name)} is missing")
        return default

    def text(self, name: str, *, max_chars: int | None = None, nonblank: bool = False,
             pattern: re.Pattern[str] | None = None, default: Any = _MISSING) -> str:
        value = self.raw(name, default)
        if not isinstance(value, str):
            raise ContractError(f"{self.at(name)} must be a string")
        if nonblank and not value.strip():
            raise ContractError(f"{self.at(name)} must not be empty")
        if max_chars is not None and len(value) > max_chars:
            raise ContractError(f"{self.at(name)} is {len(value)} characters, limit {max_chars}")
        if pattern is not None and not pattern.fullmatch(value):
            raise ContractError(f"{self.at(name)} {value[:80]!r} does not match {pattern.pattern}")
        return value

    def integer(self, name: str, *, lo: int = 0, hi: int = U32_MAX, default: Any = _MISSING) -> int:
        value = self.raw(name, default)
        if isinstance(value, bool) or not isinstance(value, int):
            raise ContractError(f"{self.at(name)} must be an integer")
        if not lo <= value <= hi:
            raise ContractError(f"{self.at(name)} must be between {lo} and {hi}")
        return value

    def number(self, name: str, *, lo: float, hi: float) -> float:
        value = self.raw(name)
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
            raise ContractError(f"{self.at(name)} must be a number")
        if not lo <= value <= hi:
            raise ContractError(f"{self.at(name)} must be between {lo} and {hi}")
        return float(value)

    def boolean(self, name: str) -> bool:
        value = self.raw(name)
        if not isinstance(value, bool):
            raise ContractError(f"{self.at(name)} must be true or false")
        return value

    def choice(self, name: str, choices: tuple[str, ...], default: Any = _MISSING) -> str:
        value = self.raw(name, default)
        if value not in choices:
            raise ContractError(f"{self.at(name)} must be one of {', '.join(choices)}")
        return value

    def obj(self, name: str) -> Obj:
        return Obj(self.raw(name), self.at(name))

    def array(self, name: str, *, min_len: int = 0, max_len: int | None = None, default: Any = _MISSING) -> list:
        value = self.raw(name, default)
        if not isinstance(value, list):
            raise ContractError(f"{self.at(name)} must be a list")
        if len(value) < min_len or (max_len is not None and len(value) > max_len):
            upper = "" if max_len is None else f" to {max_len}"
            raise ContractError(f"{self.at(name)} must have {min_len}{upper} entries, got {len(value)}")
        return value

    def string_map(self, name: str, default: Any = _MISSING) -> dict[str, str]:
        value = self.raw(name, default)
        if not isinstance(value, dict) or not all(isinstance(v, str) for v in value.values()):
            raise ContractError(f"{self.at(name)} must be an object of strings")
        return dict(value)

    def done(self) -> None:
        unknown = sorted(set(self._data) - self._seen)
        if unknown:
            raise ContractError(f"{self.path} has unknown fields: {', '.join(unknown)}")
