import numpy as np
import pytest
from conftest import tone

from nolabs_engine.audio import codec

WEBM_MAGIC = b"\x1a\x45\xdf\xa3"


def test_encode_command_follows_the_profile():
    cmd = codec.encode_command("ffmpeg", 24000, 24000, 32, "/tmp/x.webm", soxr=True)
    joined = " ".join(cmd)
    assert "-f f32le -ar 24000 -ac 1 -i pipe:0" in joined
    assert "-c:a libopus -vbr on -application audio -frame_duration 20 -b:a 32k" in joined
    assert "-af" not in cmd  # no resampling at equal rates
    assert cmd[-3:] == ["-f", "webm", "/tmp/x.webm"] and "+bitexact" in cmd


def test_encode_command_resamples_with_soxr_when_available():
    cmd = codec.encode_command("ffmpeg", 44100, 48000, 56, "/tmp/x.webm", soxr=True)
    assert cmd[cmd.index("-af") + 1] == "aresample=48000:resampler=soxr:precision=28"
    fallback = codec.encode_command("ffmpeg", 44100, 48000, 56, "/tmp/x.webm", soxr=False)
    assert fallback[fallback.index("-af") + 1] == "aresample=48000"


def test_decode_command_is_mono_float_at_rate():
    cmd = codec.decode_command("ffmpeg", "/tmp/in", 24000, 180, soxr=False)
    assert cmd[cmd.index("-t") + 1] == "180" and "-ac" in cmd and cmd[-3:] == ["-f", "f32le", "pipe:1"]


def test_missing_ffmpeg_is_reported(monkeypatch):
    monkeypatch.setenv("NOLABS_FFMPEG", "")
    monkeypatch.setattr(codec.shutil, "which", lambda _: None)
    with pytest.raises(codec.CodecError, match="not installed"):
        codec.ffmpeg_exe()


@pytest.mark.parametrize("source, out, kbps", [(24000, 24000, 32), (44100, 48000, 56)])
def test_encode_produces_webm_opus_that_decodes_back(ffmpeg, source, out, kbps):
    wave = tone(1.5, source, amplitude=0.3)
    data = codec.encode_webm(wave, source, out, kbps)
    assert data[:4] == WEBM_MAGIC and b"OpusHead" in data
    # Same input, same bytes (cache objects are immutable and content-addressed).
    assert codec.encode_webm(wave, source, out, kbps) == data
    back = codec.decode_mono(data, 24000, 60)
    assert abs(back.size / 24000 - 1.5) < 0.05
    assert np.abs(back).max() == pytest.approx(0.3, abs=0.05)


def test_decode_cuts_at_max_seconds(ffmpeg):
    data = codec.encode_webm(tone(3.0, 24000), 24000, 24000, 32)
    assert codec.decode_mono(data, 24000, 1.0).size == pytest.approx(24000, abs=480)


def test_decode_rejects_garbage(ffmpeg):
    with pytest.raises(codec.CodecError, match="ffmpeg failed"):
        codec.decode_mono(b"definitely not audio", 24000, 10)
