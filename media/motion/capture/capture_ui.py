#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["websocket-client>=1.8,<2"]
# ///
"""Capture reference stills of the real Streams Reader app (re-run after every redesign).

    uv run --script media/motion/capture/capture_ui.py [media/motion/capture/screens.json]

Logs in to the live app exactly like media/capture/capture_app.py (passphrase from ~/.config/nolabs/app.env, never
printed), opens each screen in screens.json at its viewport and DPR in headless Chrome, runs the screen's optional
`actions` (click by text, wait, scroll) and writes a PNG to media/motion/capture/ref/<name>.png plus a JSON with the
geometry of the elements the reel rebuilds (`probe` selectors: bounding boxes, font sizes, colours).

The reel itself does not paste these screenshots: it rebuilds the screens as vector HTML with the brand tokens and
the real fonts (media/motion/lib/ui.js), so every frame is sharp at any size and the word highlight can move. The
refs and probe JSON are what the rebuild is checked against (media/motion/README.md, "Keeping the UI faithful").
"""
from __future__ import annotations

import base64, json, os, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "media/tools"))
os.environ.setdefault("MEDIA_TMP", str(Path(tempfile.gettempdir()) / "streams-media"))
from cdp import Chrome  # noqa: E402
from streams_api import Api, BASE  # noqa: E402

HERE = Path(__file__).resolve().parent
cfg_path = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / "screens.json"
cfg = json.loads(cfg_path.read_text())
out = HERE / "ref"
out.mkdir(exist_ok=True)

api = Api().login()
docs = api.req("GET", "/api/library").get("documents", [])
by_title = {d["meta"]["title"]: d["id"] for d in docs}

PROBE = """(sels)=>Object.fromEntries(Object.entries(sels).map(([k,s])=>{const e=document.querySelector(s);
 if(!e) return [k,null]; const r=e.getBoundingClientRect(), c=getComputedStyle(e);
 return [k,{x:r.x,y:r.y,w:r.width,h:r.height,font:c.fontFamily.split(',')[0],size:c.fontSize,weight:c.fontWeight,
   color:c.color,bg:c.backgroundColor,radius:c.borderRadius,text:(e.innerText||'').slice(0,80)}]}))"""

for scr in cfg["screens"]:
    W, H = scr["viewport"]
    dpr = scr.get("dpr", 2)
    c = Chrome(W, H)
    d = c.cdp
    try:
        d.call("Emulation.setDeviceMetricsOverride", width=W, height=H, deviceScaleFactor=dpr, mobile=scr.get("mobile", False))
        if scr.get("theme"):
            d.call("Emulation.setEmulatedMedia", features=[{"name": "prefers-color-scheme", "value": scr["theme"]}])
        d.call("Network.enable")
        for ck in api.cookies():
            d.call("Network.setCookie", name=ck["name"], value=ck["value"], url=BASE + "/", path="/", secure=True, httpOnly=True)
        d.call("Page.navigate", url=BASE + cfg.get("base", "") + "/settings")
        time.sleep(2.5)
        d.eval("localStorage.setItem('nolabs.welcomed','true')")
        path = scr["path"].replace("{base}", cfg.get("base", ""))
        if "{doc}" in path:
            path = path.replace("{doc}", by_title.get(scr.get("doc_title", ""), next(iter(by_title.values()), "")))
        d.call("Page.navigate", url=BASE + path)
        time.sleep(scr.get("wait", 5))
        for a in scr.get("actions", []):
            if a.get("click"):
                ok = d.eval(f"""(()=>{{const n={json.dumps(a['click'])}.toLowerCase();
                  const el=[...document.querySelectorAll('button,[role=button],[role=option],a,label')]
                    .find(e=>((e.getAttribute('aria-label')||'')+' '+e.innerText).toLowerCase().includes(n));
                  if(!el) return false; el.click(); return true}})()""")
                print(f"  {scr['name']}: click {a['click']!r} -> {ok}")
            if a.get("scroll"):
                d.eval(f"window.scrollTo(0,{int(a['scroll'])})")
            time.sleep(a.get("wait", 1.0))
        png = base64.b64decode(d.call("Page.captureScreenshot", format="png")["data"])
        (out / f"{scr['name']}.png").write_bytes(png)
        probe = d.eval(f"({PROBE})({json.dumps(scr.get('probe', {}))})")
        (out / f"{scr['name']}.json").write_text(json.dumps({"viewport": [W, H], "dpr": dpr, "probe": probe}, indent=1))
        print(f"{scr['name']}: {len(png) // 1024} KB")
    finally:
        c.close()
