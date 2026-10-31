"""Pre-render the landing page's audio: the hero demo (4 texts x 4 standard voices, plus one expressive sample) and
the 6-8 s opening-line previews of the classics row. Run once; the outputs are committed.

Standard voices are rendered through the live API (POST /api/render with kokoro:* voices, the CPU tier). Expressive
(ming:*) voices spend the shared GPU budget, so this script never requests one: the expressive sample reuses the
film's existing Ming voice-over (media/assets/vo/en-ming, voice ming:US_Emma), whose word timings are estimated by
aligning the Kokoro timings of the same text to the pauses in the Ming audio.

Every clip is re-encoded to Opus 32 kbps mono WebM. Writes:
  crates/site/assets/demo/<text>-<voice>.webm + .json   ({"d": ms, "a": 48 loudness levels 1-10, "w": [[start, end, char_from, char_to]]})
  crates/site/assets/demo/demo.json                     (texts, voices and which clips exist; read by site-gen)
  crates/site/assets/previews/<slug>.webm               (one per book in assets/covers/covers.json)

    uv run --no-project --python 3.12 --with imageio-ffmpeg==0.6.0 --with numpy python -I crates/site/tools/demo_clips.py
"""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile

import imageio_ffmpeg
import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "media/tools"))
from streams_api import Api  # noqa: E402

SITE = ROOT / "crates/site"
DEMO = SITE / "assets/demo"
PREVIEWS = SITE / "assets/previews"
FFMPEG = imageio_ffmpeg.get_ffmpeg_exe()

# Sample texts. Story, poem: public domain. Article and night: written for Streams Reader.
TEXTS = {
    "story": {
        "text": "It is a truth universally acknowledged, that a single man in possession of a good fortune, "
                "must be in want of a wife.",
        "source": "Jane Austen, Pride and Prejudice (1813)",
    },
    "article": {
        "text": "Why is the sky blue? Sunlight holds every colour, but the air scatters short blue waves far more "
                "than long red ones. So blue light reaches your eyes from every direction you look.",
        "source": "Streams Reader",
    },
    "poem": {
        "text": "Hope is the thing with feathers,\nThat perches in the soul,\nAnd sings the tune without the words,\n"
                "And never stops at all.",
        "source": "Emily Dickinson (1891)",
    },
    "night": {
        "text": "It's late. Your eyes are tired, but the story isn't finished. So let it read to you.",
        "source": "Streams Reader",
    },
}
STANDARD = [
    ("kokoro:af_heart", "Heart", "us"),
    ("kokoro:am_michael", "Michael", "us"),
    ("kokoro:bf_isabella", "Isabella", "uk"),
    ("kokoro:bm_george", "George", "uk"),
]
EXPRESSIVE = ("ming:US_Emma", "Emma", "us")
MING_LINES = ["late", "readto"]  # en-ming voice-over lines that make up the "night" text
PREVIEW_VOICES = ["kokoro:bm_george", "kokoro:af_heart", "kokoro:bf_isabella", "kokoro:am_michael"]


def stem(voice: str) -> str:
    return voice.split(":")[1].lower()


def opus(src: bytes | pathlib.Path, dst: pathlib.Path, concat: list[pathlib.Path] | None = None) -> None:
    if concat:
        inputs = sum((["-i", str(p)] for p in concat), [])
        flt = "".join(f"[{i}:a]" for i in range(len(concat))) + f"concat=n={len(concat)}:v=0:a=1[a]"
        cmd = [FFMPEG, "-y", "-loglevel", "error", *inputs, "-filter_complex", flt, "-map", "[a]"]
        subprocess.run([*cmd, "-ac", "1", "-c:a", "libopus", "-b:a", "32k", "-application", "voip", str(dst)], check=True)
        return
    with tempfile.NamedTemporaryFile(suffix=".webm") as tmp:
        tmp.write(src if isinstance(src, bytes) else src.read_bytes())
        tmp.flush()
        subprocess.run([FFMPEG, "-y", "-loglevel", "error", "-i", tmp.name, "-ac", "1", "-c:a", "libopus", "-b:a", "32k",
                        "-application", "voip", str(dst)], check=True)


def pcm(path: pathlib.Path, rate: int = 16000) -> np.ndarray:
    out = subprocess.run([FFMPEG, "-loglevel", "error", "-i", str(path), "-f", "s16le", "-ac", "1", "-ar", str(rate), "-"],
                         check=True, capture_output=True).stdout
    return np.frombuffer(out, dtype=np.int16).astype(np.float32) / 32768.0


def duration_ms(path: pathlib.Path) -> int:
    return round(len(pcm(path)) / 16)


def envelope(path: pathlib.Path, bars: int = 48) -> list[int]:
    """Loudness of the clip in `bars` equal slices, as levels 1-10 (drawn as the demo's waveform)."""
    x = pcm(path)
    rms = np.array([np.sqrt((c ** 2).mean()) if len(c) else 0.0 for c in np.array_split(x, bars)])
    db = 20 * np.log10(rms + 1e-6)
    lo, hi = db.max() - 36.0, db.max()
    return [int(v) for v in np.clip(np.round(1 + 9 * (db - lo) / (hi - lo)), 1, 10)]


def clip_json(path: pathlib.Path, words: list, **extra) -> str:
    return json.dumps({"d": duration_ms(path), "a": envelope(path), "w": words, **extra}, separators=(",", ":"))


def speech_runs(x: np.ndarray, rate: int = 16000, hop_ms: int = 10, min_gap_ms: int = 140) -> list[tuple[int, int]]:
    """Speech regions in ms: frames above a level threshold, gaps shorter than min_gap_ms merged."""
    hop = rate * hop_ms // 1000
    frames = x[: len(x) // hop * hop].reshape(-1, hop)
    db = 20 * np.log10(np.sqrt((frames ** 2).mean(axis=1)) + 1e-9)
    on = db > max(-48.0, db.max() - 38.0)
    runs, start = [], None
    for i, v in enumerate(on):
        if v and start is None:
            start = i
        elif not v and start is not None:
            runs.append([start * hop_ms, i * hop_ms])
            start = None
    if start is not None:
        runs.append([start * hop_ms, len(on) * hop_ms])
    merged: list[list[int]] = []
    for r in runs:
        if merged and r[0] - merged[-1][1] < min_gap_ms:
            merged[-1][1] = r[1]
        else:
            merged.append(r)
    return [(a, b) for a, b in merged if b - a >= 60]


def align(kokoro_words: list[list[int]], ming: pathlib.Path, text: str) -> list[list[int]]:
    """Map Kokoro word timings onto the Ming audio. Pauses in the Ming audio are matched, in order, to the nearest
    word boundaries that end in punctuation; the words between two matched pauses are spread proportionally to their
    Kokoro durations over the Ming speech between them."""
    runs = speech_runs(pcm(ming))
    k_start, k_end = kokoro_words[0][0], kokoro_words[-1][1]
    m_start, m_end = runs[0][0], runs[-1][1]
    scale = (m_end - m_start) / max(1, k_end - k_start)
    punct = [i for i, w in enumerate(kokoro_words[:-1]) if w[3] < len(text) and text[w[3]] in ",.;:?!"]
    anchors = [(0, m_start, None)]  # (first word index of the segment, segment start ms, previous segment end ms)
    used = -1
    for (_, gap_start), (gap_end, _) in zip(runs[:-1], runs[1:]):
        mid = (gap_start + gap_end) / 2
        cands = [i for i in punct if i > used]
        if not cands:
            break
        guess = lambda i: m_start + ((kokoro_words[i][1] + kokoro_words[i + 1][0]) / 2 - k_start) * scale
        best = min(cands, key=lambda i: abs(guess(i) - mid))
        if abs(guess(best) - mid) > 600:
            continue
        used = best
        anchors.append((best + 1, gap_end, gap_start))
    out: list[list[int]] = []
    for n, (first, seg_start, _) in enumerate(anchors):
        last = anchors[n + 1][0] if n + 1 < len(anchors) else len(kokoro_words)
        seg_end = anchors[n + 1][2] if n + 1 < len(anchors) else m_end
        k0, k1 = kokoro_words[first][0], kokoro_words[last - 1][1]
        f = (seg_end - seg_start) / max(1, k1 - k0)
        for w in kokoro_words[first:last]:
            out.append([round(seg_start + (w[0] - k0) * f), round(seg_start + (w[1] - k0) * f), w[2], w[3]])
    return out


def ming_only() -> None:
    """Redo only the expressive sample from the committed Kokoro timings (no API calls)."""
    vo = ROOT / "media/assets/vo/en-ming"
    index = json.loads((vo / "index.json").read_text())
    dst = DEMO / f"night-{stem(EXPRESSIVE[0])}.webm"
    opus(b"", dst, concat=[vo / index["lines"][l]["file"] for l in MING_LINES])
    kokoro = json.loads((DEMO / f"night-{stem(STANDARD[0][0])}.json").read_text())["w"]
    words = align(kokoro, dst, TEXTS["night"]["text"])
    (DEMO / f"night-{stem(EXPRESSIVE[0])}.json").write_text(clip_json(dst, words, estimated=True))
    print(json.dumps(words))


def main() -> None:
    if "--ming-only" in sys.argv:
        ming_only()
        return
    DEMO.mkdir(parents=True, exist_ok=True)
    PREVIEWS.mkdir(parents=True, exist_ok=True)
    for old in list(DEMO.glob("*.mp3")) + list(DEMO.glob("*.json")) + list(DEMO.glob("*.webm")):
        old.unlink()
    api = Api().login()
    ids = list(TEXTS)
    clips: dict[str, list[str]] = {t: [] for t in ids}
    timings: dict[tuple[str, str], list] = {}
    for voice, name, _ in STANDARD:
        st = api.render(voice, [TEXTS[t]["text"] for t in ids], timeout_s=900)
        for t, s in zip(ids, st):
            dst = DEMO / f"{t}-{stem(voice)}.webm"
            opus(api.audio(s["key"]), dst)
            words = s["audio"]["words"]
            timings[(t, voice)] = words
            (DEMO / f"{t}-{stem(voice)}.json").write_text(clip_json(dst, words))
            clips[t].append(stem(voice))
            print(f"{t:8s} {name:9s} {s['audio']['duration_ms']:6d} ms {dst.stat().st_size:6d} B")

    # Expressive sample: the film's Ming voice-over, concatenated, with aligned timings.
    vo = ROOT / "media/assets/vo/en-ming"
    index = json.loads((vo / "index.json").read_text()) if (vo / "index.json").exists() else None
    expressive = None
    if index and index.get("voice") == EXPRESSIVE[0] and " ".join(index["lines"][l]["text"] for l in MING_LINES) == TEXTS["night"]["text"]:
        dst = DEMO / f"night-{stem(EXPRESSIVE[0])}.webm"
        opus(b"", dst, concat=[vo / index["lines"][l]["file"] for l in MING_LINES])
        words = align(timings[("night", STANDARD[0][0])], dst, TEXTS["night"]["text"])
        (DEMO / f"night-{stem(EXPRESSIVE[0])}.json").write_text(clip_json(dst, words, estimated=True))
        clips["night"].append(stem(EXPRESSIVE[0]))
        expressive = EXPRESSIVE
        print(f"night    {EXPRESSIVE[1]:9s} (ming, reused) {dst.stat().st_size:6d} B")
    else:
        print("no reusable Ming sample: the expressive voice shows 'with your account'")

    voices = [{"id": stem(v), "voice": v, "name": n, "accent": a, "tier": "standard"} for v, n, a in STANDARD]
    voices.append({"id": stem(EXPRESSIVE[0]), "voice": EXPRESSIVE[0], "name": EXPRESSIVE[1], "accent": EXPRESSIVE[2],
                   "tier": "expressive", "available": expressive is not None})
    manifest = {"texts": [{"id": t, **TEXTS[t], "clips": clips[t]} for t in ids], "voices": voices}
    (DEMO / "demo.json").write_text(json.dumps(manifest, indent=1) + "\n")

    # Opening-line previews for the classics row.
    covers = json.loads((SITE / "assets/covers/covers.json").read_text())["books"]
    for old in PREVIEWS.glob("*.webm"):
        old.unlink()
    for vi, voice in enumerate(PREVIEW_VOICES):
        books = [b for i, b in enumerate(covers) if i % len(PREVIEW_VOICES) == vi and b["line"]]
        if not books:
            continue
        st = api.render(voice, [b["line"] for b in books], timeout_s=900)
        for b, s in zip(books, st):
            dst = PREVIEWS / f"{b['slug']}.webm"
            opus(api.audio(s["key"]), dst)
            b["preview_voice"] = voice
            print(f"preview {b['slug']:40s} {voice:20s} {s['audio']['duration_ms']:6d} ms {dst.stat().st_size:6d} B")
    (SITE / "assets/covers/covers.json").write_text(json.dumps({"source": "Standard Ebooks (CC0 1.0)", "books": covers},
                                                               indent=1, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
