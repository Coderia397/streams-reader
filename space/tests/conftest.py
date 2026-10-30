"""Shared test helpers. Tests need numpy, soundfile and pyloudnorm only; ffmpeg tests skip when no ffmpeg is found
(set NOLABS_FFMPEG or install imageio-ffmpeg to run them)."""
from __future__ import annotations

import copy
import json
import os
import shutil
import sys
from pathlib import Path

import numpy as np
import pytest

SPACE = Path(__file__).resolve().parents[1]
if str(SPACE) not in sys.path:
    sys.path.insert(0, str(SPACE))

LOCAL_FIXTURES = SPACE / "tests" / "fixtures"
SHARED_FIXTURES = SPACE.parent / "contracts" / "fixtures"

JOB = "7q9vbxz2k4m6n8p0"
VOICE_ID = "0123456789abcdef"
K1, K2, K3 = "1" * 64, "2" * 64, "3" * 64

KOKORO_AUDIO = {"id": 1, "bitrate_kbps": 32, "sample_rate": 24000, "target_lufs": -16.0, "max_correction_db": 4.0,
                "peak_ceiling_dbfs": -1.5, "pauses": {"sentence_ms": 250, "paragraph_ms": 600, "section_ms": 1200}}
MING_AUDIO = {**KOKORO_AUDIO, "id": 2, "bitrate_kbps": 56, "sample_rate": 48000, "max_correction_db": 2.0}


def item(key: str, text: str, mode: str = "kokoro", engine: str = "kokoro", **extra) -> dict:
    return {"key": key, "text": text, "mode": mode, "object_key": f"audio/{engine}/{key}.webm", **extra}


def kokoro_request(*items: dict, **over) -> dict:
    body = {"v": 1, "job_id": JOB, "engine": "kokoro", "voice": {"kind": "preset", "id": "af_heart"},
            "items": list(items) or [item(K1, "Hello there.")], "audio": copy.deepcopy(KOKORO_AUDIO),
            "marker_key": f"jobs/{JOB}.json"}
    body.update(over)
    return body


def reference_voice() -> dict:
    return {"kind": "reference", "id": VOICE_ID, "ref_key": f"voices/{VOICE_ID}/ref.wav",
            "transcript": "A quiet hour before dawn."}


def ming_request(*items: dict, voice: dict | None = None, **over) -> dict:
    body = {"v": 1, "job_id": JOB, "engine": "ming", "voice": voice or {"kind": "preset", "id": "US_Emma"},
            "items": list(items) or [item(K1, "Hello there.", "ip", "ming")], "audio": copy.deepcopy(MING_AUDIO),
            "marker_key": f"jobs/{JOB}.json"}
    body.update(over)
    return body


def dumps(body: dict) -> str:
    return json.dumps(body, ensure_ascii=False)


def tone(seconds: float, sample_rate: int, amplitude: float = 0.3, freq: float = 220.0) -> np.ndarray:
    t = np.arange(int(seconds * sample_rate)) / sample_rate
    return (amplitude * np.sin(2 * np.pi * freq * t)).astype(np.float32)


def find_ffmpeg() -> str | None:
    exe = os.environ.get("NOLABS_FFMPEG") or shutil.which("ffmpeg")
    if exe:
        return exe
    try:
        import imageio_ffmpeg
    except ImportError:
        return None
    return imageio_ffmpeg.get_ffmpeg_exe()


@pytest.fixture
def ffmpeg(monkeypatch) -> str:
    exe = find_ffmpeg()
    if exe is None:
        pytest.skip("ffmpeg not available")
    monkeypatch.setenv("NOLABS_FFMPEG", exe)
    return exe
