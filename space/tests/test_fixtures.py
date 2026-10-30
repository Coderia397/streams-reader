"""Every JSON fixture shared with the Rust side (contracts/fixtures) and the Space's own samples (tests/fixtures) must
parse with the contract type its name announces, and serialise back without losing or changing a field.

Naming: the path (lowercase) names the type: `prepare_reference_request`, `prepare_reference_response`, `health`,
`item_result` or `sidecar`, `engine_response` or `marker`, `engine_request`. A path containing `invalid` must be
rejected. Fixtures whose names match none of these are tried against every type and skipped when none fits (they belong
to the web API, not the engine contract).
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest
from conftest import LOCAL_FIXTURES, SHARED_FIXTURES

from nolabs_engine import contracts as c
from nolabs_engine.errors import ContractError

PARSERS = (
    ("prepare_reference_request", c.parse_prepare_reference_request),
    ("prepare_reference_response", c.parse_prepare_reference_response),
    ("health", c.parse_health),
    ("item_result", c.parse_item_result),
    ("sidecar", c.parse_item_result),
    ("engine_response", c.parse_engine_response),
    ("marker", c.parse_engine_response),
    ("engine_request", c.parse_engine_request),
)


def fixtures() -> list[Path]:
    found = []
    for root in (LOCAL_FIXTURES, SHARED_FIXTURES):
        if root.is_dir():
            found.extend(sorted(root.rglob("*.json")))
    return found


def parser_for(path: Path):
    name = f"{path.parent.name}/{path.name}".lower()
    for keyword, parser in PARSERS:
        if keyword in name:
            return parser
    return None


def _subset(expected, actual) -> bool:
    """Every field of the fixture is present in the serialised value with an equal value (serde defaults may add
    fields the fixture left out)."""
    if isinstance(expected, dict):
        return isinstance(actual, dict) and all(k in actual and _subset(v, actual[k]) for k, v in expected.items())
    if isinstance(expected, list):
        return isinstance(actual, list) and len(expected) == len(actual) and all(map(_subset, expected, actual))
    return expected == actual


def test_fixture_folders_are_not_empty():
    assert any(LOCAL_FIXTURES.glob("*.json"))


@pytest.mark.parametrize("path", fixtures(), ids=lambda p: f"{p.parent.parent.name}/{p.name}")
def test_fixture_matches_contract(path: Path):
    raw = path.read_text(encoding="utf-8")
    parser = parser_for(path)
    invalid = "invalid" in path.name.lower()
    if parser is None:
        candidates = [p for _, p in PARSERS]
        accepted = []
        for candidate in candidates:
            try:
                accepted.append(candidate(raw))
            except ContractError:
                continue
        if not accepted:
            pytest.skip("not an engine contract fixture")
        return
    if invalid:
        with pytest.raises(ContractError):
            parser(raw)
        return
    parsed = parser(raw)
    out = parsed.to_dict()
    assert parser(json.dumps(out)) == parsed
    assert _subset(json.loads(raw), out), f"{path.name} changes or drops a field when serialised"
