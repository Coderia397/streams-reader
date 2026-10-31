"""Raster exports for every platform, from the generated SVGs."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw

from . import art, contact, icons, wordmark
from .art import SPOTS
from .raster import render, save_png
from .tokens import Tokens

ANDROID_DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}
ICON_SIZES = [16, 24, 32, 48, 64, 96, 128, 180, 192, 256, 512, 1024]
# The bare blue mark is only for web favicons (as Streams' favicon). Every app and desktop
# icon is the tile at every size, as in Streams' desktop set.

ADAPTIVE_XML = """<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@mipmap/ic_launcher_background"/>
    <foreground android:drawable="@mipmap/ic_launcher_foreground"/>
    <monochrome android:drawable="@mipmap/ic_launcher_monochrome"/>
</adaptive-icon>
"""


class Exporter:
    """Every raster is drawn at its own size (whole-pixel cells), never scaled from a master."""

    def __init__(self, t: Tokens, brand: Path) -> None:
        self.brand = brand
        self.icons = brand / "app-icon"
        self.dist = brand / "dist"
        self.lk = art.look(t, brand)
        self._cache: dict[tuple[str, int, int], Image.Image] = {}

    def r(self, svg: str, w: int, h: int | None = None) -> Image.Image:
        key = (svg, w, h or w)
        if key not in self._cache:
            self._cache[key] = render(svg, w, h)
        return self._cache[key].copy()

    def bare(self, n: int) -> Image.Image:
        return self.r(icons.bare(self.lk, n), n)

    def tile(self, n: int, rounded: bool = True) -> Image.Image:
        return self.r(icons.tile(self.lk, n, rounded), n)

    def png(self, rel: str, img: Image.Image) -> None:
        save_png(img, self.dist / rel)

    # -------------------------------------------------------------- platforms
    def web(self) -> None:
        for n in ICON_SIZES:
            self.png(f"png/icon-{n}.png", self.tile(n))
        for n in (16, 32, 48, 96):
            self.png(f"web/favicon-{n}.png", self.bare(n))
        self.png("web/apple-touch-icon.png", self.tile(180, rounded=False))
        for n in (192, 512):
            self.png(f"web/icon-{n}.png", self.tile(n))
            self.png(f"web/icon-maskable-{n}.png", self.r(icons.maskable(self.lk, n), n))
        ico = [self.bare(n) for n in (16, 32, 48)]
        (self.dist / "web").mkdir(parents=True, exist_ok=True)
        ico[2].save(self.dist / "web" / "favicon.ico", sizes=[(16, 16), (32, 32), (48, 48)], append_images=ico[:2])
        shutil.copyfile(self.icons / "favicon.svg", self.dist / "web" / "favicon.svg")
        shutil.copyfile(self.brand / "mark" / "safari-pinned-tab.svg", self.dist / "web" / "safari-pinned-tab.svg")
        self.png("web/og-image.png", render(self.icons / "og-image.svg", 1200, 630))
        self.png("web/splash-2732.png", self.r(icons.splash(self.lk, 2732), 2732))

    def android(self) -> None:
        for name, k in ANDROID_DENSITIES.items():
            base = f"android/mipmap-{name}"
            launcher = round(48 * k)
            layer = round(108 * k)
            self.png(f"{base}/ic_launcher.png", self.tile(launcher))
            self.png(f"{base}/ic_launcher_round.png", _circle(self.r(icons.round_tile(self.lk, launcher), launcher)))
            self.png(f"{base}/ic_launcher_foreground.png", self.r(icons.adaptive_foreground(self.lk, layer), layer))
            self.png(f"{base}/ic_launcher_background.png", self.r(icons.adaptive_background(self.lk, layer), layer))
            self.png(f"{base}/ic_launcher_monochrome.png", self.r(icons.adaptive_monochrome(self.lk, layer), layer))
            splash = round(288 * k)
            self.png(f"android/drawable-{name}/splash_icon.png", self.r(icons.splash_icon(self.lk, splash), splash))
        anydpi = self.dist / "android" / "mipmap-anydpi-v26"
        anydpi.mkdir(parents=True, exist_ok=True)
        (anydpi / "ic_launcher.xml").write_text(ADAPTIVE_XML, encoding="utf-8")
        (anydpi / "ic_launcher_round.xml").write_text(ADAPTIVE_XML, encoding="utf-8")
        self.png("android/playstore-icon.png", self.tile(512, rounded=False))

    def macos(self) -> None:
        iconset = self.dist / "macos" / "icon.iconset"
        if iconset.exists():
            shutil.rmtree(iconset)
        for n in (16, 32, 128, 256, 512):
            self.png(f"macos/icon.iconset/icon_{n}x{n}.png", self._mac(n, n))
            self.png(f"macos/icon.iconset/icon_{n}x{n}@2x.png", self._mac(n, n * 2))
        icns = self.dist / "macos" / "icon.icns"
        iconutil = shutil.which("iconutil")  # macOS: the reference encoder
        if iconutil:
            subprocess.run([iconutil, "-c", "icns", str(iconset), "-o", str(icns)], check=True)
        else:
            frames = [self._mac(n, n) for n in (16, 32, 64, 128, 256, 512)]
            self._mac(512, 1024).save(icns, append_images=frames)

    def _mac(self, points: int, px: int) -> Image.Image:
        """16 pt is the full-bleed tile (1x and 2x, the 10x10 reduction), as Streams'
        icon_16x16; 32 pt and up the macOS template."""
        if points <= 16:
            return self.tile(px)
        return self.r(icons.macos(self.lk, px), px)

    def windows(self) -> None:
        sizes = [16, 24, 32, 48, 64, 128, 256]
        frames = [self.tile(n) for n in sizes]
        out = self.dist / "windows"
        out.mkdir(parents=True, exist_ok=True)
        frames[-1].save(out / "icon.ico", sizes=[(n, n) for n in sizes], append_images=frames[:-1])
        for n in (44, 71, 150, 310):
            self.png(f"windows/Square{n}x{n}Logo.png", self.tile(n, rounded=False))
        self.png("windows/StoreLogo.png", self.tile(50, rounded=False))

    def linux(self) -> None:
        for n in (16, 24, 32, 48, 64, 128, 256, 512):
            self.png(f"linux/hicolor/{n}x{n}/apps/streams-reader.png", self.tile(n))
        scalable = self.dist / "linux" / "hicolor" / "scalable" / "apps"
        scalable.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(self.icons / "icon.svg", scalable / "streams-reader.svg")

    def tauri(self) -> None:
        """The file set `tauri icon` would produce, under the names tauri.conf.json expects."""
        for name, n in {"32x32.png": 32, "64x64.png": 64, "128x128.png": 128, "128x128@2x.png": 256, "icon.png": 512}.items():
            self.png(f"tauri/{name}", self.tile(n))
        for n in (30, 44, 71, 89, 107, 142, 150, 284, 310):
            self.png(f"tauri/Square{n}x{n}Logo.png", self.tile(n, rounded=False))
        self.png("tauri/StoreLogo.png", self.tile(50, rounded=False))
        shutil.copyfile(self.dist / "macos" / "icon.icns", self.dist / "tauri" / "icon.icns")
        shutil.copyfile(self.dist / "windows" / "icon.ico", self.dist / "tauri" / "icon.ico")

    def lockups(self) -> None:
        for old in list((self.dist / "wordmark").glob("*.png")) + list((self.dist / "mark").glob("*.png")):
            old.unlink()
        for f in sorted((self.brand / "wordmark").glob("*.svg")):
            w, h = _viewbox(f)
            for scale_h in (64, 128):
                width = round(w * scale_h / h)
                self.png(f"wordmark/{f.stem}-{scale_h}h.png", render(f, width, scale_h))
        for f in sorted((self.brand / "mark").glob("streams-reader-mark*.svg")):
            if "currentcolor" in f.stem:
                continue
            self.png(f"mark/{f.stem}-512.png", render(f, 512))

    def email(self) -> None:
        """crates/mail: the horizontal lockup at 1x/2x with the mark on 3/6 px cells, the mark
        alone and the tile as avatar. PNG only (mail clients do not render SVG)."""
        box, height = wordmark.horizontal_mark_box()
        for variant in ("on-light", "on-dark"):
            src = self.brand / "wordmark" / f"streams-reader-horizontal-{variant}.svg"
            w, h = _viewbox(src)
            for scale, mark_px in (("", 48), ("@2x", 96)):
                out_h = round(height * mark_px / box)
                self.png(f"email/streams-reader-logo-{variant}{scale}.png", render(src, round(w * out_h / h), out_h))
        for scale, n in (("", 48), ("@2x", 96)):
            self.png(f"email/streams-reader-mark{scale}.png", self.bare(n))
            self.png(f"email/streams-reader-avatar{scale}.png", self.tile(n * 2, rounded=False))

    def spots(self) -> None:
        for name in SPOTS:
            src = self.brand / "spot" / f"{name}.svg"
            if src.exists():
                for n in (192, 288):
                    self.png(f"spot/{name}@{n // 96}x.png", render(src, n))


def _viewbox(path: Path) -> tuple[float, float]:
    vb = path.read_text(encoding="utf-8").split('viewBox="', 1)[1].split('"', 1)[0].split()
    return float(vb[2]), float(vb[3])


def _circle(img: Image.Image) -> Image.Image:
    scale = 4
    mask = Image.new("L", (img.width * scale, img.height * scale), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, mask.width - 1, mask.height - 1), fill=255)
    mask = mask.resize(img.size, Image.LANCZOS)
    out = Image.new("RGBA", img.size, (0, 0, 0, 0))
    out.paste(img, (0, 0), mask)
    return out


def build_all(t: Tokens, brand: Path) -> None:
    ex = Exporter(t, brand)
    for step in (ex.web, ex.android, ex.macos, ex.windows, ex.linux, ex.tauri, ex.lockups, ex.email, ex.spots):
        step()
    contact.build(t, brand, ex, SPOTS, brand / "dist" / "contact-sheet.png")
    print("exports -> brand/dist (web, android, macos, windows, linux, tauri, wordmark, mark, email, spot, contact-sheet.png)")
