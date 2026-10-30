"""nolabs engine: the speech models behind nolabs-reader, served as Gradio API endpoints on a ZeroGPU Space.

Only the Cloudflare Worker calls this Space (it is private). Every endpoint takes and returns one JSON string; the contract
lives in nolabs_engine/contracts.py. Audio is written to R2 directly, the responses only carry metadata.
"""
import spaces  # noqa: F401  - must be imported before torch so ZeroGPU can patch CUDA

import json
import os
import platform
import time
import traceback

import gradio as gr

from nolabs_engine import contracts, storage
from nolabs_engine.service import Service

BOOT = time.time()
STATUS: dict = {"kokoro": "not loaded", "ming": "not loaded"}


def _load_ming():
    try:
        from nolabs_engine.engines import ming_engine

        ming_engine.load()
        STATUS["ming"] = "ready"
        return ming_engine
    except Exception as e:  # noqa: BLE001 - the standard tier keeps working without the GPU tier
        STATUS["ming"] = f"failed: {type(e).__name__}: {e}"
        traceback.print_exc()
        return None


def _load_kokoro():
    try:
        from nolabs_engine.engines.kokoro_engine import KokoroEngine

        engine = KokoroEngine()
        STATUS["kokoro"] = "ready"
        return engine
    except Exception as e:  # noqa: BLE001
        STATUS["kokoro"] = f"failed: {type(e).__name__}: {e}"
        traceback.print_exc()
        return None


MING = _load_ming()  # ZeroGPU wants CUDA models built at import time
KOKORO = _load_kokoro()
SERVICE = Service(storage.from_env(), KOKORO, MING)


def _guarded(fn):
    def call(raw: str) -> str:
        try:
            return fn(raw)
        except contracts.ContractError as e:
            raise gr.Error(f"bad request: {e}", print_exception=False)
        except gr.Error:
            raise
        except Exception as e:  # noqa: BLE001
            traceback.print_exc()
            raise gr.Error(f"engine error: {type(e).__name__}: {e}")
    call.__name__ = fn.__name__
    return call


@_guarded
def render_cpu(raw: str) -> str:
    req = contracts.parse_render(raw)
    if req.engine != "kokoro":
        raise contracts.ContractError("render_cpu only runs the kokoro engine")
    return SERVICE.render_kokoro(req).to_json()


@_guarded
def render_gpu(raw: str) -> str:
    req = contracts.parse_render(raw)
    if req.engine != "ming":
        raise contracts.ContractError("render_gpu only runs the ming engine")
    return SERVICE.render_ming(req).to_json()


@_guarded
def design_voice(raw: str) -> str:
    return json.dumps(SERVICE.design_voice(contracts.parse_design(raw)))


@_guarded
def catalog(_raw: str) -> str:
    from nolabs_engine.engines.kokoro_engine import VOICES as KOKORO_VOICES

    voices = [{"engine": "kokoro", "id": v.id, "name": v.name, "gender": v.gender, "accent": v.accent,
               "grade": v.grade} for v in KOKORO_VOICES]
    if MING is not None:
        voices += [{"engine": "ming", "id": vid, "name": name, "gender": gender, "accent": accent, "grade": ""}
                   for vid, (name, gender, accent) in MING.PRESETS.items()]
    return json.dumps({"v": contracts.CONTRACT_VERSION, "voices": voices})


@_guarded
def health(_raw: str) -> str:
    return json.dumps({"v": contracts.CONTRACT_VERSION, "engines": STATUS, "uptime_s": round(time.time() - BOOT),
                       "python": platform.python_version(), "cpus": os.cpu_count()})


with gr.Blocks(title="nolabs engine") as demo:
    gr.Markdown("Private speech engine for nolabs-reader. Use the API.")
    request, response = gr.Textbox(visible=False), gr.Textbox(visible=False)
    for fn, queue in ((render_cpu, "cpu"), (render_gpu, "gpu"), (design_voice, "gpu"), (catalog, None),
                      (health, None)):
        gr.Button(fn.__name__, visible=False).click(
            fn, request, response, api_name=fn.__name__,
            concurrency_limit=1 if queue else None, concurrency_id=queue,
        )

demo.queue(default_concurrency_limit=4, max_size=64).launch(show_error=True)
