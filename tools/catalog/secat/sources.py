"""Network access: the Standard Ebooks GitHub repos (API + raw files pinned to a commit) and the one-URL status check
on standardebooks.org that detects withdrawn titles (HTTP 451). Never follows links on standardebooks.org pages."""

from __future__ import annotations

import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

import httpx

ORG = "standardebooks"
USER_AGENT = "StreamsReader-catalog/1 (+https://listen.streams.watch)"
SE_SPACING_S = 2.0
STATUS_ATTEMPTS = 3
MAX_FILE_BYTES = 8 * 1024 * 1024
ATTEMPTS = 4
MAX_RETRY_AFTER_S = 60


class SourceError(RuntimeError):
    pass


@dataclass(frozen=True)
class Entry:
    path: str
    size: int


@dataclass(frozen=True)
class Snapshot:
    repo: str
    commit: str
    files: tuple[Entry, ...]


def github_token() -> str | None:
    """GITHUB_TOKEN, else the gh CLI's token. The value is only sent to api.github.com."""
    if tok := os.environ.get("GITHUB_TOKEN"):
        return tok
    if shutil.which("gh"):
        out = subprocess.run(["gh", "auth", "token"], capture_output=True, text=True, check=False)
        if out.returncode == 0 and out.stdout.strip():
            return out.stdout.strip()
    return None


class GitHub:
    def __init__(self, cache: Path, token: str | None = None) -> None:
        self.cache = cache
        headers = {"User-Agent": USER_AGENT, "Accept": "application/vnd.github+json"}
        if token:
            headers["Authorization"] = f"Bearer {token}"
        self.api = httpx.Client(base_url="https://api.github.com", headers=headers, timeout=30)
        self.raw = httpx.Client(headers={"User-Agent": USER_AGENT}, timeout=60, follow_redirects=True)

    def snapshot(self, repo: str) -> Snapshot:
        """The repo's default-branch commit and its file list. Raises SourceError if the repo is gone or archived."""
        meta = self._get_json(f"/repos/{ORG}/{repo}")
        if meta.get("archived"):
            raise SourceError(f"{repo}: repository is archived")
        branch = meta["default_branch"]
        commit = self._get_json(f"/repos/{ORG}/{repo}/commits/{branch}")["sha"]
        tree = self._get_json(f"/repos/{ORG}/{repo}/git/trees/{commit}", params={"recursive": "1"})
        if tree.get("truncated"):
            raise SourceError(f"{repo}: file list truncated by the API")
        files = tuple(Entry(t["path"], t.get("size", 0)) for t in tree["tree"] if t["type"] == "blob")
        return Snapshot(repo, commit, files)

    def file(self, snap: Snapshot, path: str) -> bytes:
        """A file at the pinned commit, cached on disk (content at a commit never changes)."""
        local = self.cache / snap.repo / snap.commit / path
        if local.is_file():
            return local.read_bytes()
        url = f"https://raw.githubusercontent.com/{ORG}/{snap.repo}/{snap.commit}/{path}"
        data = self._get_bytes(url)
        local.parent.mkdir(parents=True, exist_ok=True)
        tmp = local.with_suffix(local.suffix + ".part")
        tmp.write_bytes(data)
        tmp.replace(local)
        return data

    def _get_json(self, path: str, params: dict | None = None) -> dict:
        r = _request(self.api, path, params=params)
        if r.status_code == 404:
            raise SourceError(f"GitHub 404: {path}")
        if r.status_code in (403, 429) and r.headers.get("x-ratelimit-remaining") == "0":
            wait = max(1, int(r.headers.get("x-ratelimit-reset", "0")) - int(time.time()))
            raise SourceError(f"GitHub rate limit reached; resets in {wait} s")
        if not 200 <= r.status_code < 300:
            raise SourceError(f"GitHub HTTP {r.status_code}: {path}")
        try:
            return r.json()
        except ValueError as e:
            raise SourceError(f"GitHub sent invalid JSON for {path}: {e}") from e

    def _get_bytes(self, url: str) -> bytes:
        r = _request(self.raw, url)
        if r.status_code != 200:
            raise SourceError(f"HTTP {r.status_code}: {url}")
        if len(r.content) > MAX_FILE_BYTES:
            raise SourceError(f"file larger than {MAX_FILE_BYTES} bytes: {url}")
        return r.content


def _retry_wait(r: httpx.Response, attempt: int) -> float | None:
    """Seconds to wait before retrying this response, or None when it is final. Retried: 5xx, 429, and a 403 that
    carries Retry-After (GitHub's secondary rate limit). A primary rate limit (x-ratelimit-remaining: 0) is final."""
    if r.status_code >= 500:
        return float(2**attempt)
    if r.status_code in (403, 429) and r.headers.get("x-ratelimit-remaining") != "0":
        after = r.headers.get("retry-after", "")
        if after.isdigit():
            return float(min(int(after), MAX_RETRY_AFTER_S))
        if r.status_code == 429:
            return float(2**attempt)
    return None


def _request(client: httpx.Client, url: str, **kw) -> httpx.Response:
    """GET with ATTEMPTS tries. Transport errors (timeouts, resets, DNS) and retryable statuses back off and retry;
    the last response is returned for the caller to judge. Every httpx failure surfaces as SourceError."""
    last = "no attempt made"
    for attempt in range(ATTEMPTS):
        try:
            r = client.get(url, **kw)
        except httpx.TransportError as e:
            last = f"{type(e).__name__}: {e}"
        except httpx.HTTPError as e:  # e.g. an invalid URL; retrying will not help
            raise SourceError(f"{type(e).__name__}: {e}: {url}") from e
        else:
            wait = _retry_wait(r, attempt)
            if wait is None or attempt == ATTEMPTS - 1:
                return r
            last = f"HTTP {r.status_code}"
            time.sleep(wait)
            continue
        if attempt < ATTEMPTS - 1:
            time.sleep(2**attempt)
    raise SourceError(f"kept failing after {ATTEMPTS} attempts ({last}): {url}")


class StatusChecker:
    """One request per title to its canonical standardebooks.org URL, spaced by SE_SPACING_S. 451 = withdrawn."""

    def __init__(self, spacing_s: float = SE_SPACING_S) -> None:
        self.spacing_s = spacing_s
        self._last = 0.0
        self.client = httpx.Client(headers={"User-Agent": USER_AGENT}, timeout=30, follow_redirects=True)

    def status(self, url: str) -> int:
        """The HTTP status of the title's page. Transport errors are retried; any httpx failure is a SourceError."""
        if not url.startswith("https://standardebooks.org/ebooks/"):
            raise SourceError(f"not a Standard Ebooks book URL: {url}")
        last = ""
        for attempt in range(STATUS_ATTEMPTS):
            wait = self._last + self.spacing_s * (2**attempt) - time.monotonic()
            if wait > 0:
                time.sleep(wait)
            try:
                return self.client.head(url).status_code
            except httpx.TransportError as e:
                last = f"{type(e).__name__}: {e}"
            except httpx.HTTPError as e:
                raise SourceError(f"status check failed ({type(e).__name__}: {e}): {url}") from e
            finally:
                self._last = time.monotonic()
        raise SourceError(f"status check kept failing ({last}): {url}")
