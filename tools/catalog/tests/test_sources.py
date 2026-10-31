"""Network error handling with a mock transport: every httpx failure is a SourceError, transient ones are retried."""

from __future__ import annotations

from pathlib import Path

import httpx
import pytest

from secat import sources


@pytest.fixture(autouse=True)
def no_sleep(monkeypatch):
    monkeypatch.setattr(sources.time, "sleep", lambda s: None)


def github(tmp_path: Path, handler) -> sources.GitHub:
    gh = sources.GitHub(tmp_path)
    transport = httpx.MockTransport(handler)
    gh.api = httpx.Client(base_url="https://api.github.com", transport=transport)
    gh.raw = httpx.Client(transport=transport)
    return gh


def flaky(responses: list):
    """Handler that plays `responses` in order: an exception class is raised, an int is a status, a dict is JSON."""
    calls = []

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request.url)
        item = responses[min(len(calls), len(responses)) - 1]
        if isinstance(item, type) and issubclass(item, Exception):
            raise item("simulated", request=request)
        if isinstance(item, tuple):
            return httpx.Response(item[0], headers=item[1])
        if isinstance(item, int):
            return httpx.Response(item, content=b"x")
        return httpx.Response(200, json=item)

    return handler, calls


def test_transport_errors_are_retried(tmp_path):
    handler, calls = flaky([httpx.ConnectTimeout, httpx.ReadError, {"ok": 1}])
    assert github(tmp_path, handler)._get_json("/x") == {"ok": 1}
    assert len(calls) == 3


def test_persistent_transport_error_is_a_source_error(tmp_path):
    handler, calls = flaky([httpx.ConnectError])
    with pytest.raises(sources.SourceError, match="ConnectError"):
        github(tmp_path, handler)._get_json("/x")
    assert len(calls) == sources.ATTEMPTS


@pytest.mark.parametrize("status", [401, 403, 422])
def test_client_errors_are_source_errors(tmp_path, status):
    handler, calls = flaky([status])
    with pytest.raises(sources.SourceError, match=f"GitHub HTTP {status}"):
        github(tmp_path, handler)._get_json("/x")
    assert len(calls) == 1


def test_secondary_rate_limit_waits_and_retries(tmp_path):
    handler, calls = flaky([(403, {"retry-after": "5"}), {"ok": 1}])
    assert github(tmp_path, handler)._get_json("/x") == {"ok": 1}
    assert len(calls) == 2


def test_primary_rate_limit_is_final(tmp_path):
    handler, calls = flaky([(403, {"x-ratelimit-remaining": "0", "x-ratelimit-reset": "0"})])
    with pytest.raises(sources.SourceError, match="rate limit"):
        github(tmp_path, handler)._get_json("/x")
    assert len(calls) == 1


def test_server_errors_retry_then_fail(tmp_path):
    handler, calls = flaky([502])
    with pytest.raises(sources.SourceError, match="GitHub HTTP 502"):
        github(tmp_path, handler)._get_json("/x")
    assert len(calls) == sources.ATTEMPTS


def test_invalid_json_is_a_source_error(tmp_path):
    gh = github(tmp_path, lambda r: httpx.Response(200, content=b"<html>"))
    with pytest.raises(sources.SourceError, match="invalid JSON"):
        gh._get_json("/x")


def test_raw_file_transport_error_then_success(tmp_path):
    handler, calls = flaky([httpx.RemoteProtocolError, 200])
    assert github(tmp_path, handler)._get_bytes("https://raw.githubusercontent.com/a") == b"x"
    assert len(calls) == 2


def test_status_check_turns_httpx_errors_into_source_errors():
    checker = sources.StatusChecker(spacing_s=0)
    checker.client = httpx.Client(transport=httpx.MockTransport(flaky([httpx.ReadTimeout])[0]))
    with pytest.raises(sources.SourceError, match="ReadTimeout"):
        checker.status("https://standardebooks.org/ebooks/a/b")
    checker.client = httpx.Client(transport=httpx.MockTransport(flaky([httpx.ConnectError, 451])[0]))
    assert checker.status("https://standardebooks.org/ebooks/a/b") == 451
