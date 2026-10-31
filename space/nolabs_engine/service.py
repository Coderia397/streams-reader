"""Request handling: synthesise, post-process, upload, report, one segment at a time so results stream out.
Engines, storage, the clock and ffmpeg are injected so the whole flow runs in tests with fakes.

Write order per job (notes/09): each clip's audio, then its sidecar `<object_key>.json`, and the job marker (the full
response) last, so a marker always means every listed object is in place.
"""
from __future__ import annotations

import contextlib
import hashlib
import io
import time
from collections.abc import Callable, Generator, Iterator, Mapping, Sequence
from concurrent.futures import ThreadPoolExecutor
from typing import Protocol

import numpy as np
import soundfile as sf

from . import reference, streaming
from .audio import codec, pipeline
from .audio.pipeline import FinishedClip, RawClip, SilentOutput
from .contracts import (
    EngineHealth,
    EngineItem,
    EngineItemResult,
    EngineRequest,
    EngineResponse,
    EngineVoice,
    PrepareReferenceRequest,
    PrepareReferenceResponse,
    PresetVoice,
    ReferenceVoice,
    sidecar_key,
)
from .engines.ming import MingClip, MingDone
from .engines.timing import Word, shift_words
from .errors import ContractError, EngineUnavailable, InputError
from .revisions import MODEL_REVS

# A guided recording is 20–60 s (notes/06 v2 #12); anything past this is cut before analysis.
MAX_RECORDING_S = 180.0


class StorageLike(Protocol):
    def put_audio(self, key: str, data: bytes) -> None: ...
    def put_json(self, key: str, text: str) -> None: ...
    def put_wav(self, key: str, data: bytes) -> None: ...
    def get_bytes(self, key: str) -> bytes: ...
    def download(self, key: str) -> str: ...
    def ping(self) -> bool: ...


class SynthesisLike(Protocol):
    wave: np.ndarray
    sample_rate: int
    words: Sequence[Word]


class KokoroLike(Protocol):
    def synthesize(self, text: str, voice: str) -> SynthesisLike: ...


class MingLike(Protocol):
    def render_stream(self, items: Sequence[EngineItem], voice: EngineVoice,
                      ref_path: str) -> Generator[MingClip | MingDone, None, None]: ...


class ItemFailure(Exception):
    """One item failed; the batch carries on and reports `code` in its result."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


Decoder = Callable[[bytes, int, float], np.ndarray]


class Service:
    def __init__(self, storage: StorageLike, kokoro: KokoroLike | None = None, ming: MingLike | None = None, *,
                 statuses: Mapping[str, str] | None = None, model_revs: Mapping[str, str] = MODEL_REVS,
                 clock: Callable[[], float] = time.monotonic, encode: pipeline.Encoder = codec.encode_webm,
                 decode: Decoder = codec.decode_mono, upload_workers: int = 4, max_ahead: int = 2):
        self.storage, self.kokoro, self.ming = storage, kokoro, ming
        self.statuses = statuses if statuses is not None else {}
        self.model_revs = dict(model_revs)
        self.clock, self._encode, self._decode = clock, encode, decode
        # Items synthesised but not yet handed out: segment N+1 renders while N encodes and uploads.
        self.max_ahead = max_ahead
        self._uploads = ThreadPoolExecutor(max_workers=upload_workers, thread_name_prefix="r2")

    # -- render ---------------------------------------------------------------------------------------------------
    #
    # `stream_*` yield one EngineItemResult per item, in request order, as soon as its audio and sidecar are in R2,
    # then the EngineResponse (the same results, after the marker is written). `render_*` return the response only.

    def stream_cpu(self, req: EngineRequest) -> Iterator[EngineItemResult | EngineResponse]:
        if req.engine != "kokoro":
            raise ContractError("render_cpu only runs the kokoro engine")
        kokoro = self.kokoro
        if kokoro is None:
            raise EngineUnavailable(f"kokoro is not loaded ({self.statuses.get('kokoro', 'unknown')})")
        if not isinstance(req.voice, PresetVoice):
            raise ContractError("the kokoro engine only has preset voices")
        voice = req.voice.id

        def synthesise() -> Iterator[tuple[RawClip | BaseException, Sequence[Word]]]:
            for item in req.items:
                try:
                    synth = kokoro.synthesize(item.text, voice)
                except Exception as e:  # noqa: BLE001 - reported per item, the batch carries on
                    yield ItemFailure("synthesis_failed", f"{type(e).__name__}: {e}"), ()
                    continue
                yield RawClip(wave=synth.wave, sample_rate=synth.sample_rate, boundary=item.boundary), synth.words

        yield from self._stream(req, self.clock(), synthesise(), lambda: 0)

    def stream_gpu(self, req: EngineRequest) -> Iterator[EngineItemResult | EngineResponse]:
        if req.engine != "ming":
            raise ContractError("render_gpu only runs the ming engine")
        ming = self.ming
        if ming is None:
            raise EngineUnavailable(f"ming is not loaded ({self.statuses.get('ming', 'unknown')})")
        started = self.clock()
        ref_path = self.storage.download(req.voice.ref_key) if isinstance(req.voice, ReferenceVoice) else ""
        gpu_ms = [0]

        def synthesise() -> Iterator[tuple[RawClip | BaseException, Sequence[Word]]]:
            # Not wrapped: ZeroGPU quota and runs-limit errors must reach the Worker with their original text.
            # Closed explicitly so an early close reaches the GPU generator now, not whenever the frames are freed.
            with contextlib.closing(ming.render_stream(req.items, req.voice, ref_path)) as events:
                for event in events:
                    if isinstance(event, MingDone):
                        gpu_ms[0] = round(event.gpu_s * 1000)
                        continue
                    item = req.items[event.index]
                    if isinstance(event.wave, str):
                        yield ItemFailure("synthesis_failed", event.wave), ()
                    else:
                        yield RawClip(wave=event.wave, sample_rate=event.sample_rate, boundary=item.boundary), ()

        yield from self._stream(req, started, synthesise(), lambda: gpu_ms[0])

    def render_cpu(self, req: EngineRequest) -> EngineResponse:
        return _last(self.stream_cpu(req))

    def render_gpu(self, req: EngineRequest) -> EngineResponse:
        return _last(self.stream_gpu(req))

    def _stream(self, req: EngineRequest, started: float,
                clips: Iterator[tuple[RawClip | BaseException, Sequence[Word]]],
                gpu_ms: Callable[[], int]) -> Iterator[EngineItemResult | EngineResponse]:
        """Level each clip in order on the synthesis thread, encode + upload it on the pool, and yield results in
        request order. Write order per item stays audio then sidecar; the marker is written after the last item."""
        stream = pipeline.Stream(req.audio, self._encode)
        items = iter(req.items)

        def levelled() -> Iterator[tuple[EngineItem, pipeline.Levelled | BaseException, Sequence[Word]]]:
            try:
                for clip, words in clips:
                    item = next(items, None)
                    if item is None:
                        raise RuntimeError(f"the engine returned more clips than the {len(req.items)} items asked for")
                    yield item, stream.level(clip), words
            finally:
                # Close the engine generator explicitly, on this (the synthesis) thread: for Ming this chain reaches
                # the ZeroGPU generator, whose GeneratorExit is what stops the GPU worker after a client disconnect.
                close = getattr(clips, "close", None)
                if close is not None:
                    close()

        def finish(work: tuple[EngineItem, pipeline.Levelled | BaseException, Sequence[Word]]) -> EngineItemResult:
            item, clip, words = work
            done = stream.finish(clip)
            if isinstance(done, BaseException):
                return _failure(item.key, done)
            return self._upload(item, done, _success(item, done, words))

        results: list[EngineItemResult] = []
        first_ms: int | None = None
        for result in streaming.ordered_pipeline(levelled(), finish, self._uploads, self.max_ahead):
            if first_ms is None:
                first_ms = round((self.clock() - started) * 1000)
            results.append(result)
            yield result
        if len(results) != len(req.items):
            raise RuntimeError(f"the engine returned {len(results)} clips for {len(req.items)} items")
        response = EngineResponse(job_id=req.job_id, engine=req.engine, items=tuple(results),
                                  elapsed_ms=round((self.clock() - started) * 1000), gpu_ms=gpu_ms())
        self._write_marker(req.marker_key, response)
        ok = sum(1 for r in results if r.ok)
        print(f"render job={req.job_id} engine={req.engine} items={len(results)} ok={ok} first_ms={first_ms} "
              f"elapsed_ms={response.elapsed_ms} gpu_ms={response.gpu_ms}", flush=True)
        yield response

    def _upload(self, item: EngineItem, clip: FinishedClip, result: EngineItemResult) -> EngineItemResult:
        try:
            self.storage.put_audio(item.object_key, clip.data)
            self.storage.put_json(sidecar_key(item.object_key), result.to_json())
        except Exception as e:  # noqa: BLE001 - reported per item
            return EngineItemResult.failed(item.key, "upload_failed", f"{type(e).__name__}: {e}")
        return result

    def _write_marker(self, marker_key: str, response: EngineResponse) -> None:
        """The marker is the Worker's recovery path after a lost connection; the response itself is the primary
        channel, so a failed marker write is logged, not raised (raising would discard a finished GPU run)."""
        try:
            self.storage.put_json(marker_key, response.to_json())
        except Exception as e:  # noqa: BLE001
            print(f"marker write failed for {marker_key}: {type(e).__name__}: {e}", flush=True)

    # -- reference ------------------------------------------------------------------------------------------------

    def prepare_reference(self, req: PrepareReferenceRequest) -> PrepareReferenceResponse:
        data = self.storage.get_bytes(req.raw_key)
        try:
            wave = self._decode(data, reference.SAMPLE_RATE, MAX_RECORDING_S)
        except codec.CodecError as e:
            raise InputError(f"the recording could not be decoded: {e}") from None
        if wave.size == 0:
            raise InputError("the recording holds no audio")
        cut = reference.prepare(wave, reference.SAMPLE_RATE, req.transcript)
        wav = wav_bytes(cut.wave, reference.SAMPLE_RATE)
        self.storage.put_wav(req.ref_key, wav)
        return PrepareReferenceResponse(
            ref_key=req.ref_key, transcript=cut.transcript,
            duration_ms=round(cut.wave.size * 1000 / reference.SAMPLE_RATE),
            sha256=hashlib.sha256(wav).hexdigest(),
        )

    # -- health ---------------------------------------------------------------------------------------------------

    def health(self) -> EngineHealth:
        return EngineHealth(engines=dict(self.statuses), storage_ok=self.storage.ping(), model_revs=self.model_revs)


def _last(events: Iterator[EngineItemResult | EngineResponse]) -> EngineResponse:
    response = None
    for event in events:
        response = event
    if not isinstance(response, EngineResponse):
        raise RuntimeError("the render stream ended without a response")
    return response


def wav_bytes(wave: np.ndarray, sample_rate: int) -> bytes:
    """16-bit PCM WAV. The same samples always give the same bytes, so the sha256 is a stable voice fingerprint."""
    buf = io.BytesIO()
    sf.write(buf, np.asarray(wave, dtype=np.float32), sample_rate, format="WAV", subtype="PCM_16")
    return buf.getvalue()


def _success(item: EngineItem, clip: FinishedClip, words: Sequence[Word]) -> EngineItemResult:
    return EngineItemResult(key=item.key, ok=True, object_key=item.object_key, duration_ms=clip.duration_ms,
                            bytes=len(clip.data), words=shift_words(words, clip.lead_ms, clip.speech_ms))


def _failure(key: str, exc: BaseException) -> EngineItemResult:
    if isinstance(exc, ItemFailure):
        return EngineItemResult.failed(key, exc.code, str(exc))
    if isinstance(exc, SilentOutput):
        return EngineItemResult.failed(key, "silent_output", str(exc))
    return EngineItemResult.failed(key, "encode_failed", f"{type(exc).__name__}: {exc}")
