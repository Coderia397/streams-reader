"""build.py upload rules, with GitHub, the status check, the converter, covers and R2 replaced by fakes."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from conftest import CATALOG, load_fixture, src_files

import build
from secat import convert, covers, sources

REPOS = ("kahlil-gibran_the-prophet", "t-s-eliot_poetry")
SHELVES = """version = 1
[[shelf]]
id = "poetry"
title = "Poems"
books = [
  { repo = "kahlil-gibran_the-prophet", year = 1923, died = 1931 },
  { repo = "t-s-eliot_poetry", year = 1922, died = 1965 },
]
"""


class Fakes:
    def __init__(self) -> None:
        self.fail: dict[str, Exception] = {}  # repo -> error raised while fetching it
        self.status: dict[str, int] = {}  # repo -> HTTP status of its SE page
        self.bad_cover: set[str] = set()
        self.uploads: list[list] = []


@pytest.fixture
def fakes(monkeypatch, tmp_path) -> Fakes:
    f = Fakes()

    class GitHub:
        def __init__(self, *a, **k) -> None:
            pass

        def snapshot(self, repo: str) -> sources.Snapshot:
            if repo in f.fail:
                raise f.fail[repo]
            return sources.Snapshot(repo, "c0ffee" * 6, ())

    def fetch_sources(gh, snap):
        return src_files(snap.repo), load_fixture(snap.repo)["LICENSE.md"], snap.repo.encode()

    class Status:
        def status(self, url: str) -> int:
            return next((c for r, c in f.status.items() if r.split("_")[0] in url), 200)

    def convert_all(pairs, binary):
        out = {}
        for _, body in pairs:
            body.write_bytes(b"{}")
            out[body] = convert.BodyMeta("T", "A", 900, 150, 10, 1, 2, "ab" * 32)
        return out

    def render(svg: bytes):
        if svg.decode() in f.bad_cover:
            raise RuntimeError("resvg could not render")
        return [covers.Thumb(200, 300, "avif", b"img")]

    monkeypatch.setattr(build.sources, "GitHub", GitHub)
    monkeypatch.setattr(build.sources, "StatusChecker", Status)
    monkeypatch.setattr(build.sources, "github_token", lambda: None)
    monkeypatch.setattr(build, "fetch_sources", fetch_sources)
    monkeypatch.setattr(build.convert, "build", lambda: Path("epub2body"))
    monkeypatch.setattr(build.convert, "convert", convert_all)
    monkeypatch.setattr(build.covers, "render", render)
    monkeypatch.setattr(build.r2, "read_env", dict)
    monkeypatch.setattr(
        build.r2,
        "upload",
        lambda plan, env: f.uploads.append(plan) or {"uploaded": len(plan), "skipped": 0, "bytes": 0},
    )
    (tmp_path / "shelves.toml").write_text(SHELVES)
    return f


def run(tmp_path: Path, *flags: str) -> tuple[int, dict, dict]:
    code = build.main(["--shelves", str(tmp_path / "shelves.toml"), "--out", str(tmp_path / "out"), *flags])
    out = tmp_path / "out"
    report = json.loads((out / "report.json").read_text()) if (out / "report.json").exists() else {}
    index_path = out / "catalog" / "v1" / "catalog.json"
    index = json.loads(index_path.read_text()) if index_path.exists() else {}
    return code, report, index


@pytest.mark.parametrize(
    "flags, message",
    [
        (["--upload", "--only", "t-s-eliot_poetry"], "--only cannot be combined with --upload"),
        (["--upload", "--no-status-check"], "--upload requires the 451 status check"),
        (["--allow-partial"], "--allow-partial only applies to --upload"),
    ],
)
def test_refused_flag_combinations(fakes, tmp_path, capsys, flags, message):
    code, report, _ = run(tmp_path, *flags)
    assert code == 2 and message in capsys.readouterr().err
    assert report == {} and fakes.uploads == []


def test_clean_build_uploads_with_regions(fakes, tmp_path):
    code, _, index = run(tmp_path, "--upload")
    assert code == 0 and len(fakes.uploads) == 1
    assert index["us_only"] == ["t-s-eliot_poetry"]
    regions = {b["id"]: b["region"] for b in index["shelves"][0]["books"]}
    assert regions == {"kahlil-gibran_the-prophet": "world", "t-s-eliot_poetry": "us"}


@pytest.mark.parametrize(
    "error",
    [
        sources.SourceError("GitHub HTTP 401: /repos/x"),
        KeyError("default_branch"),
        __import__("httpx").ReadTimeout("simulated"),
    ],
)
def test_failed_title_blocks_the_upload(fakes, tmp_path, capsys, error):
    fakes.fail["t-s-eliot_poetry"] = error
    code, report, _ = run(tmp_path, "--upload")
    assert code == 1 and fakes.uploads == []
    assert "not uploading: 1 titles failed: ['t-s-eliot_poetry']" in capsys.readouterr().err
    assert report["kahlil-gibran_the-prophet"]["ok"] and not report["t-s-eliot_poetry"]["ok"]


def test_cover_failure_fails_one_title_and_stages_nothing_of_it(fakes, tmp_path):
    fakes.bad_cover.add("t-s-eliot_poetry")
    code, report, index = run(tmp_path, "--upload")
    assert code == 1 and fakes.uploads == []
    assert report["t-s-eliot_poetry"]["reason"].startswith("cover/stage failed: RuntimeError")
    assert [b["id"] for b in index["shelves"][0]["books"]] == ["kahlil-gibran_the-prophet"]
    plan = json.loads((tmp_path / "out" / "upload-plan.json").read_text())
    assert not any("t-s-eliot" in o["key"] for o in plan["objects"])


def test_allow_partial_uploads_but_still_exits_1(fakes, tmp_path):
    fakes.fail["t-s-eliot_poetry"] = sources.SourceError("gone")
    code, _, index = run(tmp_path, "--upload", "--allow-partial")
    assert code == 1 and len(fakes.uploads) == 1
    assert [b["id"] for b in index["shelves"][0]["books"]] == ["kahlil-gibran_the-prophet"]


def test_withdrawn_title_is_not_a_failure(fakes, tmp_path):
    fakes.status["t-s-eliot_poetry"] = 451
    code, report, index = run(tmp_path, "--upload")
    assert code == 0 and len(fakes.uploads) == 1
    assert report["t-s-eliot_poetry"]["withdrawn"] is True
    assert [w["id"] for w in index["withdrawn"]] == ["t-s-eliot_poetry"]
    assert build.failed_titles(report) == []


def test_only_without_upload_builds_locally(fakes, tmp_path):
    code, report, _ = run(tmp_path, "--only", "kahlil-gibran_the-prophet")
    assert code == 0 and fakes.uploads == [] and list(report) == ["kahlil-gibran_the-prophet"]


def test_real_shelves_file_loads():
    assert build.curation.load(CATALOG / "shelves.toml")
