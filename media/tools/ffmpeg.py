"""Locate a static ffmpeg: $FFMPEG, then ffmpeg on PATH, then the imageio-ffmpeg wheel via uvx (cached)."""
import functools, os, shutil, subprocess


@functools.cache
def ffmpeg_exe() -> str:
    if os.environ.get("FFMPEG"):
        return os.environ["FFMPEG"]
    if shutil.which("ffmpeg"):
        return shutil.which("ffmpeg")
    out = subprocess.run(["uvx", "--from", "imageio-ffmpeg", "python", "-c",
                          "import imageio_ffmpeg;print(imageio_ffmpeg.get_ffmpeg_exe())"],
                         capture_output=True, text=True, check=True).stdout.strip().splitlines()[-1]
    return out
