# /// script
# requires-python = ">=3.12"
# dependencies = ["httpx>=0.28", "pillow>=11.3", "resvg-py>=0.2", "boto3>=1.35"]
# ///
"""Builds the Explore catalogue from Standard Ebooks' GitHub repos and (with --upload) puts it on R2.

    uv run tools/catalog/build.py --out <dir> [--only repo,repo | --upload [--allow-partial]] [--no-status-check]

Steps per curated title: pin the repo's head commit, fetch the package document, text and compiled cover at that
commit, check the licence and the title's standardebooks.org status (451 = withdrawn, skipped), pack a minimal
EPUB, convert it with the web app's importer (tools/catalog/epub2body), render cover thumbnails, then write
catalog.json, upload-plan.json and report.json under --out. Objects are staged at <out>/<R2 key>.

Upload rules: --upload replaces the live index, so it refuses --only (a partial index would empty the other shelves)
and --no-status-check (a withdrawn title would be served), and it does not run when any title failed unless
--allow-partial is given. A failure of one title (network, licence, import, cover) never stops the others."""

from __future__ import annotations

import argparse
import json
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from secat import catalog, convert, covers, curation, opf, pack, r2, sources

HERE = Path(__file__).resolve().parent
COVER_SVG = "src/epub/images/cover.svg"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--shelves", type=Path, default=HERE / "shelves.toml")
    p.add_argument("--out", type=Path, required=True, help="staging dir; R2 keys are written below it")
    p.add_argument("--cache", type=Path, help="source file cache (default <out>/_cache)")
    p.add_argument("--only", help="comma-separated repos to build locally (the index then lists only these)")
    p.add_argument("--no-status-check", action="store_true", help="skip the standardebooks.org 451 check")
    p.add_argument("--budget-mb", type=float, default=150.0, help="refuse to upload a plan larger than this")
    p.add_argument("--upload", action="store_true", help="upload the plan to R2 (nolabs-media, catalog/v1/)")
    p.add_argument(
        "--allow-partial", action="store_true", help="upload even if titles failed (they drop out of the live index)"
    )
    return p.parse_args(argv)


def check_args(args: argparse.Namespace) -> str | None:
    """Why this combination of flags is refused, or None."""
    if args.upload and args.only:
        return "--only cannot be combined with --upload (it would replace the live index with a partial one)"
    if args.upload and args.no_status_check:
        return "--upload requires the 451 status check (drop --no-status-check)"
    if args.allow_partial and not args.upload:
        return "--allow-partial only applies to --upload"
    return None


def failed_titles(report: dict[str, dict]) -> list[str]:
    """Titles that did not build. Withdrawn (451) titles are excluded: leaving them out is the correct outcome."""
    return sorted(r for r, v in report.items() if not v["ok"] and not v.get("withdrawn"))


def describe(e: BaseException) -> str:
    return str(e) if isinstance(e, (sources.SourceError, opf.OpfError)) else f"{type(e).__name__}: {e}"


def fetch_sources(gh: sources.GitHub, snap: sources.Snapshot) -> tuple[dict[str, bytes], bytes, bytes]:
    """(files under src/ for the EPUB, LICENSE.md, compiled cover SVG)."""
    wanted = [e.path for e in snap.files if e.path.startswith("src/") and pack.wanted(e.path[4:])]
    names = {e.path for e in snap.files}
    for required in ("LICENSE.md", COVER_SVG, "src/epub/content.opf"):
        if required not in names:
            raise sources.SourceError(f"{snap.repo}: {required} missing")
    with ThreadPoolExecutor(max_workers=8) as pool:
        texts = dict(zip(wanted, pool.map(lambda p: gh.file(snap, p), wanted)))
    src = {p[4:]: data for p, data in texts.items()}
    return src, gh.file(snap, "LICENSE.md"), gh.file(snap, COVER_SVG)


def stage_book(out: Path, d: dict, res: convert.BodyMeta, body_data: bytes) -> tuple[list[catalog.Upload], dict, dict]:
    """Stages one converted title (body, cover thumbnails, about file). Returns (uploads, index entry, report row);
    nothing of a title that raises here reaches the plan."""
    repo = d["pick"].repo
    body = catalog.stage(out, catalog.body_key(repo, res.sha256), body_data, "application/json", catalog.IMMUTABLE)
    uploads: list[catalog.Upload] = []
    thumbs = []
    for th in covers.render(d["svg"]):
        key = catalog.cover_key(repo, th)
        uploads.append(catalog.stage(out, key, th.data, th.content_type, catalog.IMMUTABLE))
        thumbs.append((th, key))
    about_data = catalog.about_bytes(d["meta"])
    about = catalog.stage(out, catalog.about_key(repo, about_data), about_data, "application/json", catalog.IMMUTABLE)
    uploads += [body, about]
    entry = catalog.book_entry(d["pick"], d["meta"], res, body, about, thumbs, d["snap"].commit)
    row = {
        "ok": True,
        "region": entry["region"],
        "chars": res.char_count,
        "words": res.word_count,
        "se_words": d["meta"].se_word_count,
        "blocks": res.block_count,
        "headings": res.heading_count,
        "body_bytes": res.body_bytes,
        "minutes": catalog.listening_minutes(res.char_count),
    }
    return uploads, entry, row


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if refusal := check_args(args):
        print(refusal, file=sys.stderr)
        return 2
    sys.stdout.reconfigure(line_buffering=True)
    shelves = curation.load(args.shelves)
    picks = [p for s in shelves for p in s.picks]
    if args.only:
        only = {r.strip() for r in args.only.split(",") if r.strip()}
        unknown = only - {p.repo for p in picks}
        if unknown:
            print(f"not in the shelves file: {sorted(unknown)}", file=sys.stderr)
            return 2
        picks = [p for p in picks if p.repo in only]

    out: Path = args.out.resolve()
    work = out / "_work"
    work.mkdir(parents=True, exist_ok=True)
    gh = sources.GitHub(args.cache or out / "_cache", sources.github_token())
    status = None if args.no_status_check else sources.StatusChecker()

    report: dict[str, dict] = {}
    withdrawn: list[dict] = []
    prepared: dict[str, dict] = {}
    t0 = time.monotonic()
    for i, pick in enumerate(picks, 1):
        tag = f"[{i}/{len(picks)}] {pick.repo}"
        try:
            snap = gh.snapshot(pick.repo)
            src, license_md, cover_svg = fetch_sources(gh, snap)
            meta = opf.parse(src[pack.OPF])
            if not catalog.licence_ok(license_md.decode("utf-8", "replace")):
                raise sources.SourceError("LICENSE.md is not the SE public-domain + CC0 text")
            if status is not None:
                code = status.status(meta.url)
                if code == 451:
                    withdrawn.append({"id": pick.repo, "url": meta.url})
                    report[pick.repo] = {"ok": False, "withdrawn": True, "reason": "withdrawn (HTTP 451)"}
                    print(f"{tag}: withdrawn (451), skipped")
                    continue
                if code != 200:
                    raise sources.SourceError(f"standardebooks.org answered {code} for {meta.url}")
            epub_path = work / f"{pick.repo}.epub"
            epub_path.write_bytes(pack.pack(src))
            prepared[pick.repo] = {"pick": pick, "snap": snap, "meta": meta, "epub": epub_path, "svg": cover_svg}
            print(f"{tag}: fetched {len(src)} files at {snap.commit[:10]}")
        except Exception as e:  # noqa: BLE001 - one title's failure (network, licence, metadata, packing) must not stop the rest
            report[pick.repo] = {"ok": False, "reason": describe(e)}
            print(f"{tag}: FAILED {describe(e)}", file=sys.stderr)

    binary = convert.build()
    pairs = [(d["epub"], work / f"{repo}.body.json") for repo, d in prepared.items()]
    results = convert.convert(pairs, binary)

    plan: list[catalog.Upload] = []
    entries: dict[str, dict] = {}
    for (epub_path, body_path), repo in zip(pairs, prepared):
        res = results[body_path]
        if isinstance(res, str):
            report[repo] = {"ok": False, "reason": f"import failed: {res}"}
            print(f"{repo}: import FAILED {res}", file=sys.stderr)
            continue
        try:
            uploads, entries[repo], report[repo] = stage_book(out, prepared[repo], res, body_path.read_bytes())
        except Exception as e:  # noqa: BLE001 - cover rendering or staging of one title must not stop the rest
            report[repo] = {"ok": False, "reason": f"cover/stage failed: {describe(e)}"}
            print(f"{repo}: cover/stage FAILED {describe(e)}", file=sys.stderr)
            continue
        plan += uploads
        epub_path.unlink(missing_ok=True)
        body_path.unlink(missing_ok=True)

    index = catalog.index(shelves, entries, withdrawn)
    plan.append(
        catalog.stage(out, catalog.INDEX_KEY, catalog.index_bytes(index), "application/json", catalog.INDEX_CACHE)
    )
    total = sum(u.size for u in plan)
    (out / "upload-plan.json").write_text(
        json.dumps({"bucket_prefix": catalog.PREFIX, "bytes": total, "objects": [u.as_json() for u in plan]}, indent=1)
    )
    (out / "report.json").write_text(json.dumps(report, indent=1))
    ok = sum(1 for r in report.values() if r["ok"])
    failed = failed_titles(report)
    print(f"built {ok}/{len(picks)} books, {len(plan)} objects, {total / 1e6:.1f} MB in {time.monotonic() - t0:.0f} s")
    if withdrawn:
        print(f"withdrawn (451, left out): {[w['id'] for w in withdrawn]}")

    if total > args.budget_mb * 1e6:
        print(f"plan exceeds the {args.budget_mb} MB budget; not uploading", file=sys.stderr)
        return 1
    if args.upload and failed and not args.allow_partial:
        print(f"not uploading: {len(failed)} titles failed: {failed} (see report.json)", file=sys.stderr)
        return 1
    if args.upload:
        stats = r2.upload(plan, r2.read_env())
        print(f"uploaded {stats['uploaded']} ({stats['bytes'] / 1e6:.1f} MB), skipped {stats['skipped']} existing")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
