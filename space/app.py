"""nolabs engine: the speech models behind nolabs-reader, served as Gradio API endpoints on a ZeroGPU Space.

Only the Cloudflare Worker calls this Space (it is private). Every endpoint takes one JSON string and returns one JSON
string; contract v1 lives in nolabs_engine/contracts.py (mirror of crates/core/src/proto/engine.rs). Audio is written
to R2 directly; responses carry metadata only. render_cpu and render_gpu stream one item result per finished segment
as `generating` events before the full response (see handlers.py).
"""
from nolabs_engine import runtime

# Size native thread pools from the container's CPU allowance before torch exists (os.cpu_count() reports the host).
THREADS = runtime.cpu_allowance()
runtime.configure_threads(THREADS)

import traceback  # noqa: E402

import gradio as gr  # noqa: E402
import spaces  # noqa: E402,F401 - must be imported before torch so ZeroGPU can patch CUDA

from nolabs_engine import storage  # noqa: E402
from nolabs_engine.errors import FailureKind  # noqa: E402
from nolabs_engine.handlers import EndpointFailure, Endpoints  # noqa: E402
from nolabs_engine.service import Service  # noqa: E402

STATUS: dict[str, str] = {"kokoro": "not loaded", "ming": "not loaded"}


def _load_ming():
    try:
        from nolabs_engine.engines import ming_gpu

        engine = ming_gpu.load()
        STATUS["ming"] = "ready"
        return engine
    except Exception as e:  # noqa: BLE001 - the standard tier keeps working without the GPU tier
        STATUS["ming"] = f"failed: {type(e).__name__}: {e}"[:300]
        traceback.print_exc()
        return None


def _load_kokoro():
    try:
        import torch

        from nolabs_engine.engines.kokoro_engine import KokoroEngine

        runtime.configure_torch(torch, THREADS)
        engine = KokoroEngine()
        STATUS["kokoro"] = "ready"
        return engine
    except Exception as e:  # noqa: BLE001
        STATUS["kokoro"] = f"failed: {type(e).__name__}: {e}"[:300]
        traceback.print_exc()
        return None


MING = _load_ming()  # ZeroGPU wants CUDA models built at import time
KOKORO = _load_kokoro()
print(f"nolabs engine: threads={THREADS} kokoro={STATUS['kokoro']} ming={STATUS['ming']}", flush=True)
ENDPOINTS = Endpoints(Service(storage.from_env(), KOKORO, MING, statuses=STATUS), passthrough=(gr.Error,))


def _gradio_error(failure: EndpointFailure) -> Exception:
    """Bad requests and engine errors become gr.Error with the contract's prefixes; ZeroGPU errors are re-raised as
    they came so the Worker can read their text."""
    if failure.kind is FailureKind.PASSTHROUGH and isinstance(failure.original, gr.Error):
        return failure.original
    return gr.Error(failure.message, print_exception=failure.kind is FailureKind.ENGINE)


def _gradio(handler):
    def call(raw: str) -> str:
        try:
            return handler(raw)
        except EndpointFailure as failure:
            raise _gradio_error(failure) from None
    call.__name__ = handler.__name__
    return call


def _gradio_stream(handler):
    """A generator handler stays a generator function, so Gradio sends each yield as a `generating` event and the
    last one as `complete`."""
    def call(raw: str):
        try:
            yield from handler(raw)
        except EndpointFailure as failure:
            raise _gradio_error(failure) from None
    call.__name__ = handler.__name__
    return call


with gr.Blocks(title="nolabs engine") as demo:
    gr.Markdown("Private speech engine for nolabs-reader. Use the API.")
    request, response = gr.Textbox(visible=False), gr.Textbox(visible=False)
    # (handler, concurrency id): one CPU job at a time (Kokoro and reference preparation share 2 vCPU), one GPU job
    # at a time, and health with no concurrency limit so it answers while a render runs.
    bindings = ((_gradio_stream(ENDPOINTS.render_cpu), "cpu"), (_gradio_stream(ENDPOINTS.render_gpu), "gpu"),
                (_gradio(ENDPOINTS.prepare_reference), "cpu"), (_gradio(ENDPOINTS.health), None))
    for handler, queue in bindings:
        gr.Button(handler.__name__, visible=False).click(
            handler, request, response, api_name=handler.__name__,
            concurrency_limit=1 if queue else None, concurrency_id=queue,
        )

demo.queue(default_concurrency_limit=4, max_size=64).launch(show_error=True)
