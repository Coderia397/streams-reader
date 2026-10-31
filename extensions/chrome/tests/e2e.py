# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""End-to-end test: loads the extension unpacked in headless Chrome and drives the real read flow.

    uv run extensions/chrome/tests/e2e.py [--chrome PATH] [--keep]

A local server serves a fixture article on http://127.0.0.1:PORT and a mock app (the reference receiver) on
http://localhost:PORT, which is a different origin. The extension is built with package.build() for the mock app's
origin, then a test entry point exposes read() to the test, 127.0.0.1 is added as a host permission because
headless Chrome has no toolbar to click (activeTab needs a user gesture), and the error badge lasts 2 s instead of 6 s.
Everything else is the shipped code, including the 30 s lease and 5 min expiry.

Chrome is driven over --remote-debugging-pipe with Extensions.loadUnpacked, which branded Chrome still allows
(the --load-extension switch is ignored there since Chrome 137). No third-party packages.
"""

from __future__ import annotations

import argparse
import fcntl
import functools
import http.server
import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXT = HERE.parent
sys.path.insert(0, str(EXT))
import package  # noqa: E402

CHROME_CANDIDATES = [
    os.environ.get("CHROME", ""),
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta",
    "/Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
]

TEST_SW = """import "./background.js";
import { read, store } from "./actions.js";
const claims = [];
const claim = store.claim;
store.claim = async (tabId) => {
  const t0 = Date.now();
  const p = await claim(tabId);
  claims.push({ tabId, got: !!p, ms: Date.now() - t0, at: Date.now() });
  return p;
};
globalThis.__srTest = { read, claims };
"""


TEST_BADGE_MS = 2000
SWEEP_ALARM = "handoff-sweep"
LEASE_MS = 30_000


class Fail(Exception):
    pass


# ---------------------------------------------------------------- server


class Handler(http.server.SimpleHTTPRequestHandler):
    """/ext/* → extension source, /fixtures/* → fixtures, /app/* → mock app (any query)."""

    def translate_path(self, path: str) -> str:
        path = path.split("?", 1)[0].split("#", 1)[0]
        if path.startswith("/ext/"):
            return str(EXT / path[len("/ext/") :])
        if path.startswith("/fixtures/"):
            return str(HERE / "fixtures" / path[len("/fixtures/") :])
        if path.startswith("/app/"):
            rel = path[len("/app/") :]
            target = HERE / "fixtures" / "app" / rel
            return str(target / "index.html" if target.is_dir() else target)
        return str(HERE / "fixtures" / "__nothing__")

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, *args: object) -> None:
        pass


def start_server() -> tuple[http.server.ThreadingHTTPServer, int]:
    Handler.extensions_map = {**Handler.extensions_map, ".js": "text/javascript"}
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


# ---------------------------------------------------------------- CDP over a pipe


class Cdp:
    def __init__(self, chrome: str, profile: Path) -> None:
        to_chrome_r, to_chrome_w = os.pipe()
        from_chrome_r, from_chrome_w = os.pipe()
        # Move the child's ends above 100 so dup2 onto 3 and 4 cannot clobber them.
        hi_r = fcntl.fcntl(to_chrome_r, fcntl.F_DUPFD, 100)
        hi_w = fcntl.fcntl(from_chrome_w, fcntl.F_DUPFD, 100)
        os.close(to_chrome_r)
        os.close(from_chrome_w)

        def child_fds() -> None:
            os.dup2(hi_r, 3)
            os.dup2(hi_w, 4)

        args = [
            chrome,
            "--headless=new",
            "--remote-debugging-pipe",
            "--enable-unsafe-extension-debugging",
            f"--user-data-dir={profile}",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-sync",
            "--disable-component-update",
            "--disable-background-networking",
            "--password-store=basic",
            "--use-mock-keychain",
            "about:blank",
        ]
        self.proc = subprocess.Popen(
            args, pass_fds=(3, 4), preexec_fn=child_fds, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        os.close(hi_r)
        os.close(hi_w)
        self.w = os.fdopen(to_chrome_w, "wb", buffering=0)
        self.r = os.fdopen(from_chrome_r, "rb", buffering=0)
        self.next_id = 0
        self.pending: dict[int, queue.Queue] = {}
        self.lock = threading.Lock()
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self) -> None:
        buf = b""
        while True:
            chunk = self.r.read(65536)
            if not chunk:
                return
            buf += chunk
            while b"\0" in buf:
                raw, buf = buf.split(b"\0", 1)
                msg = json.loads(raw)
                if "id" in msg and msg["id"] in self.pending:
                    self.pending.pop(msg["id"]).put(msg)

    def send(self, method: str, params: dict | None = None, session: str | None = None, timeout: float = 20) -> dict:
        with self.lock:
            self.next_id += 1
            mid = self.next_id
            q: queue.Queue = queue.Queue()
            self.pending[mid] = q
            msg: dict = {"id": mid, "method": method, "params": params or {}}
            if session:
                msg["sessionId"] = session
            self.w.write(json.dumps(msg).encode() + b"\0")
        try:
            res = q.get(timeout=timeout)
        except queue.Empty:
            raise Fail(f"CDP {method} timed out") from None
        if "error" in res:
            raise Fail(f"CDP {method}: {res['error']}")
        return res["result"]

    def evaluate(self, session: str, expr: str, timeout: float = 20):
        res = self.send(
            "Runtime.evaluate",
            {"expression": expr, "awaitPromise": True, "returnByValue": True},
            session,
            timeout,
        )
        if "exceptionDetails" in res:
            d = res["exceptionDetails"]
            raise Fail(f"evaluate failed: {d.get('exception', {}).get('description') or d.get('text')}")
        return res["result"].get("value")

    def attach(self, target_id: str) -> str:
        return self.send("Target.attachToTarget", {"targetId": target_id, "flatten": True})["sessionId"]

    def targets(self) -> list[dict]:
        return self.send("Target.getTargets")["targetInfos"]

    def wait_target(self, pred, what: str, timeout: float = 15) -> dict:
        end = time.time() + timeout
        while time.time() < end:
            for t in self.targets():
                if pred(t):
                    return t
            time.sleep(0.1)
        raise Fail(f"no target: {what}")

    def close(self) -> None:
        try:
            self.send("Browser.close", timeout=5)
        except Exception:
            pass
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def find_chrome(explicit: str | None) -> str:
    for c in [explicit or "", *CHROME_CANDIDATES]:
        if c and Path(c).exists():
            return c
    raise Fail("no Chrome found; pass --chrome or set CHROME")


# ---------------------------------------------------------------- the test


def build_test_extension(out: Path, app_origin: str) -> None:
    package.build(out, app_origin)
    (out / "src" / "test-sw.js").write_text(TEST_SW, "utf-8")
    cfg = out / "src" / "config.js"
    text = cfg.read_text("utf-8")
    badge = "export const BADGE_MS = 6000;"
    if badge not in text:
        raise Fail("config.js badge line changed; update e2e.py")
    cfg.write_text(text.replace(badge, f"export const BADGE_MS = {TEST_BADGE_MS};"), "utf-8")
    mpath = out / "manifest.json"
    m = json.loads(mpath.read_text("utf-8"))
    m["background"]["service_worker"] = "src/test-sw.js"
    m["host_permissions"] = ["http://127.0.0.1/*"]
    mpath.write_text(json.dumps(m, indent=2), "utf-8")


def check(cond: bool, what: str, results: list[str]) -> None:
    if not cond:
        raise Fail(what)
    results.append(f"ok  {what}")


def run(chrome: str, keep: bool, results: list[str]) -> None:
    srv, port = start_server()
    article_origin = f"http://127.0.0.1:{port}"
    app_origin = f"http://localhost:{port}"
    import_url = f"{app_origin}/app/import?from=extension"
    work = Path(tempfile.mkdtemp(prefix="sr-ext-e2e-"))
    cdp = None
    try:
        ext_dir = work / "ext"
        build_test_extension(ext_dir, app_origin)
        cdp = Cdp(chrome, work / "profile")
        version = cdp.send("Browser.getVersion")["product"]
        results.append(f"--  {version}")
        prod_dir = work / "prod"
        package.build(prod_dir)
        prod_id = cdp.send("Extensions.loadUnpacked", {"path": str(prod_dir)})["id"]
        cdp.wait_target(
            lambda t: t["type"] == "service_worker" and t["url"].startswith(f"chrome-extension://{prod_id}/"),
            "production build's service worker",
        )
        results.append("ok  production build loads and its worker starts")
        cdp.send("Extensions.uninstall", {"id": prod_id})
        ext_id = cdp.send("Extensions.loadUnpacked", {"path": str(ext_dir)})["id"]
        check(len(ext_id) == 32, "extension loads unpacked (manifest, CSP, locales accepted)", results)

        sw = cdp.wait_target(
            lambda t: t["type"] == "service_worker" and t["url"].startswith(f"chrome-extension://{ext_id}/"),
            "extension service worker",
        )
        sws = cdp.attach(sw["targetId"])
        cdp.evaluate(sws, "new Promise(r => { const t = () => globalThis.__srTest ? r(1) : setTimeout(t, 50); t(); })")
        name = cdp.evaluate(sws, "chrome.i18n.getMessage('ext_name')")
        check(name == "Streams Reader — read this page", f"i18n resolves the name ({name})", results)
        perms = cdp.evaluate(sws, "chrome.permissions.getAll()")
        check(
            sorted(perms["permissions"]) == ["activeTab", "alarms", "contextMenus", "scripting", "storage"],
            f"granted API permissions are exactly the five ({sorted(perms['permissions'])})",
            results,
        )

        # Unit tests in a plain page.
        unit = cdp.send("Target.createTarget", {"url": f"{article_origin}/ext/tests/unit.html"})["targetId"]
        us = cdp.attach(unit)
        u = cdp.evaluate(
            us,
            "new Promise(r => { const t = () => window.__unit ? r(window.__unit) : setTimeout(t, 50); t(); })",
        )
        check(not u["failures"], f"unit tests: {u['passed']} passed, failures {u['failures']}", results)
        cdp.send("Target.closeTarget", {"targetId": unit})

        def open_page(url: str) -> tuple[str, str, int]:
            tid = cdp.send("Target.createTarget", {"url": url})["targetId"]
            s = cdp.attach(tid)
            cdp.evaluate(s, "new Promise(r => document.readyState === 'complete' ? r(1) : addEventListener('load', () => r(1)))")
            tab_id = cdp.evaluate(
                sws,
                f"chrome.tabs.query({{}}).then(ts => ts.find(t => t.url === {json.dumps(url)} && t.status === 'complete')?.id ?? null)",
            )
            if tab_id is None:
                # The tab can still report "loading" for a moment after the page's load event.
                time.sleep(0.3)
                tab_id = cdp.evaluate(sws, f"chrome.tabs.query({{}}).then(ts => ts.find(t => t.url === {json.dumps(url)})?.id)")
            return tid, s, tab_id

        def read(tab_id: int, req: dict) -> dict:
            return cdp.evaluate(sws, f"chrome.tabs.get({tab_id}).then(t => __srTest.read(t, {json.dumps(req)}))")

        def app_result(app_tab_id: int) -> dict:
            # The tab's URL is read from DevTools: the extension itself may not see it (no "tabs" permission).
            t = cdp.wait_target(
                lambda t: t["type"] == "page" and t["url"].startswith(app_origin) and t["attached"] is False, "app tab"
            )
            check(t["url"] == import_url, f"app tab URL carries no page content ({t['url']})", results)
            s = cdp.attach(t["targetId"])
            try:
                res = cdp.evaluate(
                    s,
                    "new Promise(r => { const t = () => window.__mockApp ? r(window.__mockApp) : setTimeout(t, 50); t(); })",
                    timeout=15,
                )
            except Fail:
                state = cdp.evaluate(
                    s,
                    "JSON.stringify({ready: document.readyState, out: document.getElementById('out')?.textContent,"
                    " ext: document.documentElement.dataset.streamsReaderExtension ?? null, log: window.__mockLog,"
                    " nav: performance.getEntriesByType('navigation')[0]?.type, flag: localStorage.getItem('sr-lose-first')})",
                    timeout=5,
                )
                raise Fail(f"mock app did not finish: {state}") from None
            cdp.send("Target.closeTarget", {"targetId": t["targetId"]})
            return res

        # 1. Whole page.
        art_url = f"{article_origin}/fixtures/article.html"
        _, art_s, art_tab = open_page(art_url)
        t0 = time.time()
        r = read(art_tab, {"kind": "page"})
        check(r.get("ok") is True, f"read page returns ok ({r})", results)
        alarm = cdp.evaluate(sws, f"chrome.alarms.get({json.dumps(SWEEP_ALARM)}).then(a => a ? a.scheduledTime - Date.now() : null)")
        check(
            alarm is not None and 290_000 < alarm <= 301_000,
            f"storing a handoff arms the sweep alarm for its expiry (+1 s) ({alarm} ms ahead)",
            results,
        )
        app = app_result(r["appTabId"])
        elapsed = time.time() - t0
        p = app["first"]
        if p is None:
            dump = cdp.evaluate(sws, "chrome.storage.session.get(null).then(o => JSON.stringify(Object.values(o).map(e => ({tab: e.tabId, exp: e.expiresAt - Date.now()}))))")
            claims = cdp.evaluate(sws, "JSON.stringify(__srTest.claims)")
            raise Fail(f"app got nothing; app tab {r['appTabId']}; pending {dump}; claims {claims}; app {app}")
        check(p["kind"] == "page", "app receives the page payload", results)
        check(p["title"] == "The Lighthouse Keeper's Year" and p["lang"] == "en" and p["url"] == art_url, "title, lang, url", results)
        check("ARTICLE_MARKER" in p["html"], "article text is in the HTML", results)
        check("SCRIPT_MARKER" not in p["html"] and "SVG_MARKER" not in p["html"], "scripts, styles, svg stripped", results)
        check(
            "<q>SHADOW_MARKER" in p["html"] and "SLOT_MARKER" in p["html"] and "<slot" not in p["html"],
            "text inside a web component's open shadow root is in the HTML, slot filled",
            results,
        )
        check(app["second"] is None, "a second claim gets nothing (one-shot)", results)
        check(app["foreignReplies"] == [], "a cross-origin frame inside the app gets no reply", results)
        check(app["extension"] == "0.1.0", "app sees data-streams-reader-extension", results)
        left = cdp.evaluate(
            sws,
            "new Promise(r => { const end = Date.now() + 10000; const t = () => chrome.storage.session.get(null)"
            ".then(o => { const n = Object.keys(o).length; n === 0 || Date.now() > end ? r(n) : setTimeout(t, 100); }); t(); })",
        )
        check(left == 0, "the app's ack empties storage.session", results)
        results.append(f"--  page handoff: {len(p['html'])} chars of HTML, click to payload in app {elapsed:.2f} s (headless, local)")

        # 2. Selection.
        cdp.evaluate(
            art_s,
            "(() => { const r = document.createRange(); r.selectNodeContents(document.getElementById('first'));"
            " getSelection().removeAllRanges(); getSelection().addRange(r); return 1; })()",
        )
        r = read(art_tab, {"kind": "selection", "frameId": 0, "selectionText": "fallback"})
        p = app_result(r["appTabId"])["first"]
        check(p["kind"] == "selection" and p["text"].startswith("ARTICLE_MARKER The light"), "selection handoff", results)
        check("html" not in p, "selection payload carries no HTML", results)

        # 2b. The page clears its selection on right-click: Chrome's own copy of the selected text is sent.
        sel_url = f"{article_origin}/fixtures/clears-selection.html"
        _, sel_s, sel_tab = open_page(sel_url)
        sel_text = cdp.evaluate(
            sel_s,
            "(() => { const r = document.createRange(); r.selectNodeContents(document.getElementById('text'));"
            " getSelection().removeAllRanges(); getSelection().addRange(r); const t = getSelection().toString();"
            " document.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true }));"
            " return getSelection().toString() === '' ? t : null; })()",
        )
        check(bool(sel_text), "fixture page cleared its selection on contextmenu", results)
        r = read(sel_tab, {"kind": "selection", "frameId": 0, "selectionText": sel_text})
        p = app_result(r["appTabId"])["first"]
        check(
            p is not None and p["kind"] == "selection" and p["text"] == sel_text and p["lang"] == "fr",
            "a selection the page cleared falls back to Chrome's selection text",
            results,
        )

        # 3. Link.
        link = f"{article_origin}/fixtures/article.html#more"
        r = read(art_tab, {"kind": "link", "linkUrl": link})
        p = app_result(r["appTabId"])["first"]
        check(p["kind"] == "link" and p["url"] == link and "html" not in p, "link handoff", results)

        # 4. App tab closed before it claimed: the next app tab (reload, sign-in in a new tab) receives it.
        r = read(art_tab, {"kind": "page"})
        cdp.evaluate(sws, f"chrome.tabs.remove({r['appTabId']})")
        t = cdp.send("Target.createTarget", {"url": import_url})["targetId"]
        s = cdp.attach(t)
        res = cdp.evaluate(s, "new Promise(r => { const t = () => window.__mockApp ? r(window.__mockApp) : setTimeout(t, 50); t(); })", timeout=30)
        check(res["first"] is not None and res["first"]["kind"] == "page", "orphaned handoff reaches the next app tab", results)
        cdp.send("Target.closeTarget", {"targetId": t})

        # 5. The app's first reply is lost (it timed out): after the lease the reloaded app gets the same handoff.
        t = cdp.send("Target.createTarget", {"url": f"{app_origin}/fixtures/empty.html"})["targetId"]
        s = cdp.attach(t)
        cdp.evaluate(s, "new Promise(r => document.readyState === 'complete' ? r(1) : addEventListener('load', () => r(1)))")
        cdp.evaluate(s, "localStorage.setItem('sr-lose-first', '1')")
        cdp.send("Target.closeTarget", {"targetId": t})
        r = read(art_tab, {"kind": "page"})
        time.sleep(3)  # Let the page give up and reload before DevTools attaches (see the note on attach + reload).
        app = app_result(r["appTabId"])
        claims = cdp.evaluate(sws, "__srTest.claims.slice(-3)")
        got = [c["got"] for c in claims]
        gap = claims[1]["at"] - claims[0]["at"] if len(claims) == 3 else None
        check(
            app["first"] is not None
            and app["first"]["kind"] == "page"
            and app["second"] is None
            and got == [True, True, False]
            and claims[0]["tabId"] == claims[1]["tabId"] == r["appTabId"]
            and gap is not None
            and gap < LEASE_MS,
            f"a reply lost before a reload is re-offered to the same tab inside the lease (claims got {got}, "
            f"reload asked {gap} ms after the first claim)",
            results,
        )

        # 5b. A PDF open in Chrome's viewer goes to the app as a link to the PDF.
        pdf_url = f"{article_origin}/fixtures/doc.pdf"
        before = set(cdp.evaluate(sws, "chrome.tabs.query({}).then(ts => ts.map(t => t.id))"))
        pdf_t = cdp.send("Target.createTarget", {"url": pdf_url})["targetId"]
        pdf_tab = None
        end = time.time() + 10
        while pdf_tab is None and time.time() < end:
            time.sleep(0.3)
            pdf_tab = cdp.evaluate(
                sws,
                f"chrome.tabs.query({{}}).then(ts => ts.find(t => !{json.dumps(sorted(before))}.includes(t.id)"
                f" && t.url === {json.dumps(pdf_url)} && t.status === 'complete')?.id ?? null)",
            )
        check(pdf_tab is not None, "PDF tab opened", results)
        how = cdp.evaluate(
            sws,
            f"chrome.scripting.executeScript({{ target: {{ tabId: {pdf_tab} }}, func: () => document.contentType }})"
            ".then(r => 'script runs, contentType ' + r[0].result, e => 'script cannot run: ' + e.message)",
        )
        results.append(f"--  PDF viewer: {how}")
        r = read(pdf_tab, {"kind": "page"})
        check(r.get("ok") is True, f"read on the PDF viewer returns ok ({r})", results)
        p = app_result(r["appTabId"])["first"]
        check(
            p is not None and p["kind"] == "link" and p["url"] == pdf_url and "html" not in p,
            "PDF handoff is a link to the PDF",
            results,
        )
        cdp.send("Target.closeTarget", {"targetId": pdf_t})

        # 6. Errors: empty page, browser page, javascript: link.
        _, _, empty_tab = open_page(f"{article_origin}/fixtures/empty.html")
        r = read(empty_tab, {"kind": "page"})
        check(r == {"ok": False, "error": "empty"}, f"empty page is refused ({r})", results)
        badge = cdp.evaluate(sws, f"chrome.action.getBadgeText({{tabId: {empty_tab}}})")
        title = cdp.evaluate(sws, f"chrome.action.getTitle({{tabId: {empty_tab}}})")
        check(badge == "!" and title == "There's no readable text here.", f"error badge and tooltip ({badge!r}, {title!r})", results)
        # A second error 1 s later keeps the badge until its own timer; then the button's tooltip comes back.
        half = TEST_BADGE_MS // 2
        timeline = cdp.evaluate(
            sws,
            "(async () => { const sleep = (ms) => new Promise(r => setTimeout(r, ms));"
            f" const id = {empty_tab}; const state = async () => [await chrome.action.getBadgeText({{tabId: id}}),"
            " await chrome.action.getTitle({tabId: id})];"
            f" await sleep({TEST_BADGE_MS + 300}); const cleared = await state();"
            " const t = await chrome.tabs.get(id); await __srTest.read(t, {kind: 'page'});"
            f" await sleep({half}); await __srTest.read(t, {{kind: 'page'}});"
            f" await sleep({half + 500}); const overlap = await state();"
            f" await sleep({half + 300}); return {{ cleared, overlap, end: await state() }}; }})()",
            timeout=30,
        )
        action_title = "Read this page in Streams Reader"
        check(timeline["cleared"] == ["", action_title], f"badge clears and the button's tooltip is restored ({timeline['cleared']})", results)
        check(timeline["overlap"][0] == "!", f"a newer error's badge is not cleared by the older timer ({timeline['overlap']})", results)
        check(timeline["end"] == ["", action_title], f"the newer error clears on its own timer ({timeline['end']})", results)

        # Without the "tabs" permission the extension cannot see a chrome:// URL, so find the new tab by id diff.
        before = set(cdp.evaluate(sws, "chrome.tabs.query({}).then(ts => ts.map(t => t.id))"))
        tid = cdp.send("Target.createTarget", {"url": "chrome://version/"})["targetId"]
        time.sleep(0.5)
        after = set(cdp.evaluate(sws, "chrome.tabs.query({}).then(ts => ts.map(t => t.id))"))
        check(len(after - before) == 1, "chrome://version tab found", results)
        chrome_tab = (after - before).pop()
        r = read(chrome_tab, {"kind": "page"})
        check(r == {"ok": False, "error": "restricted"}, f"chrome:// page is refused ({r})", results)
        cdp.send("Target.closeTarget", {"targetId": tid})

        r = read(art_tab, {"kind": "link", "linkUrl": "javascript:alert(1)"})
        check(r == {"ok": False, "error": "restricted"}, "javascript: link is refused", results)

        tabs = [t for t in cdp.targets() if t["type"] == "page" and t["url"].startswith(app_origin)]
        check(not tabs, "refusals open no app tab", results)

        # 7. An entry nobody collects is removed by the sweep alarm, without any further use of the extension.
        t0 = time.time()
        swept = cdp.evaluate(
            sws,
            "(async () => { await chrome.storage.session.set({ 'handoff:stale': { payload: { createdAt: 0 }, tabId: 1,"
            " expiresAt: Date.now() - 1 } });"
            f" await chrome.alarms.create({json.dumps(SWEEP_ALARM)}, {{ when: Date.now() + 100 }});"
            " const end = Date.now() + 45000;"
            " for (;;) { const o = await chrome.storage.session.get('handoff:stale');"
            " if (!o['handoff:stale']) break; if (Date.now() > end) return null;"
            " await new Promise(r => setTimeout(r, 100)); }"
            " await new Promise(r => setTimeout(r, 300));"
            f" return {{ alarm: (await chrome.alarms.get({json.dumps(SWEEP_ALARM)})) ?? null,"
            " left: Object.keys(await chrome.storage.session.get(null)).length }; })()",
            timeout=60,
        )
        check(
            swept is not None and swept["left"] == 0 and swept["alarm"] is None,
            f"the sweep alarm removes an expired handoff and is not re-armed when none are left ({swept}, "
            f"{time.time() - t0:.1f} s)",
            results,
        )
    finally:
        if cdp:
            cdp.close()
        srv.shutdown()
        if keep:
            print(f"kept {work}")
        else:
            shutil.rmtree(work, ignore_errors=True)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--chrome")
    ap.add_argument("--keep", action="store_true", help="keep the temp profile and built extension")
    args = ap.parse_args()
    results: list[str] = []
    try:
        run(find_chrome(args.chrome), args.keep, results)
    except Fail as e:
        print("\n".join(results))
        print(f"FAIL {e}", file=sys.stderr)
        return 1
    print("\n".join(results))
    print(f"PASS {sum(1 for r in results if r.startswith('ok'))} checks")
    return 0


if __name__ == "__main__":
    sys.exit(main())
