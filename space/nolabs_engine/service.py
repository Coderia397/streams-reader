"""Request handling: validate, synthesise, encode, upload, report. Engines and storage are injected for testing."""
from __future__ import annotations

import io
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Callable, Protocol

import soundfile as sf

from . import audio
from .contracts import (
    ContractError, DesignRequest, RenderRequest, RenderResponse, SegmentResult, audio_object_key,
)


class StorageLike(Protocol):
    def put_audio(self, key: str, data: bytes) -> None: ...
    def put_wav(self, key: str, data: bytes) -> None: ...
    def download(self, key: str) -> str: ...


class KokoroLike(Protocol):
    def synthesize(self, text: str, voice: str, speed: float): ...


class MingLike(Protocol):
    SAMPLE_RATE: int
    PRESETS: dict
    ANCHOR_TEXT: str
    def plan(self, text, emotion, voice_kind, voice_id, ref_path, transcript): ...
    def design_job(self, description, sample_text): ...
    def run(self, jobs): ...


class Service:
    def __init__(self, storage: StorageLike, kokoro: KokoroLike | None, ming: MingLike | None,
                 clock: Callable[[], float] = time.monotonic):
        self.storage, self.kokoro, self.ming, self.clock = storage, kokoro, ming, clock
        self._uploads = ThreadPoolExecutor(max_workers=4, thread_name_prefix="r2")

    # -- standard tier --------------------------------------------------------------------------------------------
    def render_kokoro(self, req: RenderRequest) -> RenderResponse:
        if self.kokoro is None:
            raise ContractError("the Kokoro engine is not loaded on this Space")
        started = self.clock()
        results, pending = [], []
        for seg in req.segments:
            try:
                synth = self.kokoro.synthesize(seg.text, req.voice.id, req.speed)
                encoded = audio.finish(synth.wave, 24_000, req.bitrate_kbps)
                words = [[w.start_ms, w.end_ms, w.char_start, w.char_end] for w in synth.words]
                pending.append(self._upload(seg.key, encoded, words))
            except Exception as e:  # noqa: BLE001 - report per segment, keep going
                results.append(SegmentResult(key=seg.key, error=f"{type(e).__name__}: {e}"))
        results.extend(f.result() for f in pending)
        return RenderResponse(results=_ordered(req, results), engine="kokoro",
                              elapsed_ms=round((self.clock() - started) * 1000))

    # -- expressive tier ------------------------------------------------------------------------------------------
    def render_ming(self, req: RenderRequest) -> RenderResponse:
        if self.ming is None:
            raise ContractError("the Ming engine is not loaded on this Space")
        voice = req.voice
        if voice.kind == "preset" and voice.id not in self.ming.PRESETS:
            raise ContractError(f"unknown Ming voice {voice.id!r}")
        started = self.clock()
        ref_path = self.storage.download(voice.ref_key) if voice.kind == "reference" else ""
        jobs = [self.ming.plan(seg.text, seg.emotion, voice.kind, voice.id, ref_path, voice.transcript)
                for seg in req.segments]
        waves, _anchors, gpu_s = self.ming.run(jobs)
        results, pending = [], []
        for seg, wave in zip(req.segments, waves):
            if isinstance(wave, str):
                results.append(SegmentResult(key=seg.key, error=wave))
                continue
            try:
                encoded = audio.finish(wave, self.ming.SAMPLE_RATE, req.bitrate_kbps)
                pending.append(self._upload(seg.key, encoded, []))
            except Exception as e:  # noqa: BLE001
                results.append(SegmentResult(key=seg.key, error=f"{type(e).__name__}: {e}"))
        results.extend(f.result() for f in pending)
        return RenderResponse(results=_ordered(req, results), engine="ming",
                              elapsed_ms=round((self.clock() - started) * 1000), gpu_ms=round(gpu_s * 1000))

    def design_voice(self, req: DesignRequest) -> dict:
        if self.ming is None:
            raise ContractError("the Ming engine is not loaded on this Space")
        started = self.clock()
        waves, _, gpu_s = self.ming.run([self.ming.design_job(req.description, req.sample_text)])
        if isinstance(waves[0], str):
            raise RuntimeError(waves[0])
        wave = audio.normalise(waves[0], self.ming.SAMPLE_RATE)
        buf = io.BytesIO()
        sf.write(buf, wave, self.ming.SAMPLE_RATE, format="WAV", subtype="PCM_16")
        self.storage.put_wav(req.key, buf.getvalue())
        return {"v": 1, "ref_key": req.key, "transcript": req.sample_text,
                "duration_ms": round(wave.size * 1000 / self.ming.SAMPLE_RATE),
                "elapsed_ms": round((self.clock() - started) * 1000), "gpu_ms": round(gpu_s * 1000)}

    # -- helpers --------------------------------------------------------------------------------------------------
    def _upload(self, key: str, encoded: audio.EncodedAudio, words: list[list[int]]):
        object_key = audio_object_key(key)

        def put() -> SegmentResult:
            try:
                self.storage.put_audio(object_key, encoded.data)
            except Exception as e:  # noqa: BLE001
                return SegmentResult(key=key, error=f"upload failed: {e}")
            return SegmentResult(key=key, object_key=object_key, duration_ms=encoded.duration_ms,
                                 bytes=len(encoded.data), words=words)

        return self._uploads.submit(put)


def _ordered(req: RenderRequest, results: list[SegmentResult]) -> list[SegmentResult]:
    order = {seg.key: i for i, seg in enumerate(req.segments)}
    return sorted(results, key=lambda r: order.get(r.key, len(order)))
