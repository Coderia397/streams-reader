"""Copy the hero film loop and the voxel spot renders from media/dist into crates/site/assets (committed copies, so the
site builds without the film studio's outputs, which are gitignored).

Hero: the landscape and portrait silent loops (WebM AV1 + MP4 H.264) and their AVIF/JPEG posters, each file checked
against the 1.5 MB budget. If media/dist/hero is missing, the promo poster stands in and the <video> keeps the expected
file names. Spots: media/dist/spots/<name>-512.webp resized to 280 px WebP.

    uv run --no-project --python 3.12 --with pillow==11.3.0 python -I crates/site/tools/media.py
"""
from __future__ import annotations

import pathlib
import shutil

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[3]
SITE = ROOT / "crates/site/assets"
HERO = ROOT / "media/dist/hero"
SPOTS = ROOT / "media/dist/spots"
VIDEO_BUDGET = 1_500_000
SPOT_NAMES = ["open-source", "voice-clone", "offline", "all-platforms", "import-link", "import-file", "import-paste",
              "background-play", "privacy", "invite", "voice-design", "languages", "sync-devices"]


def main() -> None:
    out = SITE / "media"
    out.mkdir(parents=True, exist_ok=True)
    names = [f"{n}{ext}" for n in ("hero-loop", "hero-loop-portrait")
             for ext in (".webm", ".mp4", ".poster.avif", ".poster.jpg")]
    if HERO.is_dir():
        for name in names:
            src = HERO / name
            if not src.is_file():
                print(f"missing {src}")
                continue
            size = src.stat().st_size
            if size > VIDEO_BUDGET:
                raise SystemExit(f"{name} is {size} B, over the 1.5 MB budget")
            shutil.copy2(src, out / name)
            print(f"hero  {name:32s} {size:8d} B")
    else:
        promo = ROOT / "media/dist/promo/promo.poster.jpg"
        shutil.copy2(promo, out / "hero-loop.poster.jpg")
        print("no hero loop: promo poster copied; the video keeps its expected file names")
    spots = SITE / "spots"
    spots.mkdir(parents=True, exist_ok=True)
    for n in SPOT_NAMES:
        src = SPOTS / f"{n}-512.webp"
        if not src.is_file():
            print(f"missing spot {n}")
            continue
        im = Image.open(src).convert("RGBA").resize((280, 280), Image.LANCZOS)
        dst = spots / f"{n}.webp"
        im.save(dst, "WEBP", quality=78, method=6)
        print(f"spot  {n:32s} {dst.stat().st_size:8d} B")


if __name__ == "__main__":
    main()
