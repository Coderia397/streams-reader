"""Headless check: every in-page anchor of the landing lands its target 80 ± 4 px from the top (below the sticky bar).
The skip link (#main) and the "try it" links (they place the whole demo card, played by site.js) are left out.

uv run --no-project --python 3.12 --with websockets==13.1 python crates/site/tools/check_anchors.py http://127.0.0.1:8793/ [widths]

Serve crates/site/dist first (python -m http.server 8793 --bind 127.0.0.1). Uses Google Chrome (Beta) headless on
CDP port 9545 with a throwaway profile. Exits 1 on any miss. Added 2026-10-31 after #faq overshot by 1,118 px at 1440.
"""
import asyncio, glob, json, shutil, subprocess, sys, tempfile, time, urllib.request

import websockets

CHROME = (glob.glob("/Applications/Google Chrome Beta.app/Contents/MacOS/*") + glob.glob("/Applications/Google Chrome.app/Contents/MacOS/*"))[0]
PORT = 9545
URL = sys.argv[1]
WIDTHS = [int(w) for w in (sys.argv[2] if len(sys.argv) > 2 else "390,768,1440").split(",")]
PROBE = """(async()=>{const r={},w=ms=>new Promise(x=>setTimeout(x,ms));
for(const a of [...document.querySelectorAll('a[href*="#"]')]){const id=a.getAttribute('href').split('#')[1],t=document.getElementById(id);
if(!t||!t.closest('main')||a.hasAttribute('data-try')||id==='main'||!a.getAttribute('href').replace(/#.*/,'').match(/^(\\/([a-z]{2}\\/)?)?$/))continue;
window.scrollTo({top:0,behavior:'instant'});await w(300);a.click();await w(2200);
r[(a.closest('footer')?'footer ':'')+'#'+id]=Math.round(t.getBoundingClientRect().top)}return r})()"""


async def main():
    prof = tempfile.mkdtemp(prefix="anchors-")
    p = subprocess.Popen([CHROME, "--headless=new", f"--remote-debugging-port={PORT}", f"--user-data-dir={prof}", "--no-first-run", "about:blank"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    bad = 0
    try:
        for _ in range(60):
            try:
                page = next(t for t in json.load(urllib.request.urlopen(f"http://127.0.0.1:{PORT}/json")) if t["type"] == "page")
                break
            except Exception:
                time.sleep(0.2)
        async with websockets.connect(page["webSocketDebuggerUrl"], max_size=2**26) as ws:
            n = [0]

            async def call(m, **pa):
                n[0] += 1
                my = n[0]
                await ws.send(json.dumps({"id": my, "method": m, "params": pa}))
                while True:
                    msg = json.loads(await ws.recv())
                    if msg.get("id") == my:
                        return msg.get("result", {})

            await call("Page.enable")
            for w in WIDTHS:
                await call("Emulation.setDeviceMetricsOverride", width=w, height=844 if w < 600 else 900, deviceScaleFactor=1, mobile=w < 600)
                await call("Page.navigate", url=URL)
                await asyncio.sleep(3)
                r = (await call("Runtime.evaluate", expression=PROBE, returnByValue=True, awaitPromise=True)).get("result", {}).get("value", {})
                for k, top in r.items():
                    ok = abs(top - 80) <= 4
                    bad += not ok
                    print(f"{w:5} {k:24} top={top:6} {'ok' if ok else 'MISS'}")
    finally:
        p.terminate()
        p.wait(timeout=10)
        shutil.rmtree(prof, ignore_errors=True)
    sys.exit(1 if bad else 0)


asyncio.run(main())
