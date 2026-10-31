# Fish Audio bake-off stage worker, run inside the private Kaggle kernel by fish_kernel.py as
#   python fish_stage.py <refs|drama3|s2pro|asr>
# Never imported locally. Inputs: /tmp/fish/jobs.json, /tmp/fish/fish-speech (pinned clone), the dataset's ref wavs.
# Outputs: /kaggle/working/raw/<label>/<passage>/NN.flac (24 kHz PCM 16), /kaggle/working/logs/<stage>.json.
#
# drama3: fishaudio/drama-3-int4 is a GPTQ int4 checkpoint meant for SGLang-Omni's Marlin kernels (SM80+). The T4 is
# SM75, so it is loaded here into fish-speech's own PyTorch Drama 3 implementation (fish_omni) with two changes:
# - routed experts stay int4 on the GPU (qweight int32 [K/8, N], fp16 scales per 128 inputs, symmetric, offset 8) and
#   are unpacked per forward for the experts a token selects (pure PyTorch, no custom kernel);
# - the int4 dense projections (attention q/k/v/o, linear-attention in_proj_qkv / in_proj_z / out_proj) are unpacked
#   to fp16 once at load.
# The backbone is spread over cuda:0 and cuda:1 by bytes; the codebook tower and the codec sit on cuda:1. fp16 with
# the fp32 residual stream that fish_omni uses before SM80. CUDA graphs are off (they need a single device).
import gc
import json
import math
import os
import re
import shutil
import subprocess
import sys
import time
import traceback

STAGE = sys.argv[1]
WORK = "/tmp/fish"
OUT = "/kaggle/working"
spec = json.load(open(f"{WORK}/jobs.json"))
sys.path.insert(0, f"{WORK}/fish-speech")
os.makedirs(f"{OUT}/logs", exist_ok=True)
LOG_PATH = f"{OUT}/logs/{STAGE}.json"
log = {"stage": STAGE, "items": [], "errors": [], "events": []}
T0 = time.time()


def save():
    log["elapsed_s"] = round(time.time() - T0, 1)
    with open(LOG_PATH, "w") as f:
        json.dump(log, f, indent=1)


def event(msg):
    print(f"[{STAGE} {time.time() - T0:7.1f}s] {msg}", flush=True)
    log["events"].append([round(time.time() - T0, 1), msg])
    save()


def smi():
    return subprocess.run("nvidia-smi --query-gpu=index,memory.used,memory.total --format=csv,noheader",
                          shell=True, capture_output=True, text=True).stdout.strip()


import numpy as np  # noqa: E402
import soundfile as sf  # noqa: E402
import torch  # noqa: E402

OUT_SR = 24_000


def resample(wave, sr, target):
    if sr == target:
        return wave
    import librosa
    return librosa.resample(wave.astype(np.float32), orig_sr=sr, target_sr=target, res_type="soxr_hq")


def mem(devices):
    return {str(d): {"peak_alloc_gib": round(torch.cuda.max_memory_allocated(d) / 2**30, 2),
                     "peak_reserved_gib": round(torch.cuda.max_memory_reserved(d) / 2**30, 2)} for d in devices}


# ------------------------------------------------------------------------------------------------------------ refs
def make_asr():
    from transformers import pipeline
    kw = dict(model=spec["asr"]["repo"], revision=spec["asr"]["revision"], device="cuda:0")
    try:
        return pipeline("automatic-speech-recognition", dtype=torch.float16, **kw)
    except TypeError:
        return pipeline("automatic-speech-recognition", torch_dtype=torch.float16, **kw)


def stage_refs():
    asr = make_asr()
    refs = {}
    for voice, r in spec["refs"].items():
        wave, sr = sf.read(f"{spec['input_dir']}/{r['file']}", dtype="float32")
        res = asr({"raw": resample(wave, sr, 16_000), "sampling_rate": 16_000},
                  generate_kwargs={"language": "en", "task": "transcribe"})
        text = res["text"].strip()
        refs[voice] = {**r, "transcript": text, "duration_s": round(wave.size / sr, 2)}
        event(f"ref {voice}: {wave.size / sr:.2f} s: {text}")
    log["refs"] = refs
    json.dump(refs, open(f"{WORK}/refs.json", "w"), indent=1)
    json.dump(refs, open(f"{OUT}/refs.json", "w"), indent=1)


# ------------------------------------------------------------------------------------------------------------ asr
def stage_asr():
    asr = make_asr()
    out = {}
    for root, _, files in os.walk(f"{OUT}/raw"):
        for f in sorted(files):
            if not f.endswith(".flac"):
                continue
            p = os.path.join(root, f)
            wave, sr = sf.read(p, dtype="float32")
            res = asr({"raw": resample(wave, sr, 16_000), "sampling_rate": 16_000}, return_timestamps=True,
                      generate_kwargs={"language": "en", "task": "transcribe"})
            out[os.path.relpath(p, f"{OUT}/raw")] = res["text"].strip()
    log["transcripts"] = out
    event(f"transcribed {len(out)} clips")
    del asr
    gc.collect()
    torch.cuda.empty_cache()
    mos = {}
    try:
        pred = torch.hub.load(spec["mos"]["hub"], spec["mos"]["model"], trust_repo=True).to("cuda:0").eval()
        for rel in out:
            wave, sr = sf.read(f"{OUT}/raw/{rel}", dtype="float32")
            x = torch.from_numpy(resample(wave, sr, 16_000))[None].to("cuda:0")
            with torch.inference_mode():
                mos[rel] = round(float(pred(x, 16000).reshape(-1)[0]), 3)
        # the clone references too, as the ceiling of what the clone can sound like
        for voice, r in spec["refs"].items():
            wave, sr = sf.read(f"{spec['input_dir']}/{r['file']}", dtype="float32")
            x = torch.from_numpy(resample(wave, sr, 16_000))[None].to("cuda:0")
            with torch.inference_mode():
                mos[f"ref_{voice}"] = round(float(pred(x, 16000).reshape(-1)[0]), 3)
    except Exception:  # noqa: BLE001
        log["errors"].append("mos: " + traceback.format_exc()[-3000:])
    log["mos"] = mos
    event(f"UTMOS for {len(mos)} clips")


# --------------------------------------------------------------------------------------------------- int4 helpers
def unpack_int4(qweight, scales, dtype):
    """qweight int32 [..., K/8, N] (value k at bits 4*(k%8) of word k//8), scales [..., K/G, N] -> W^T [..., K, N]."""
    shifts = torch.arange(0, 32, 4, device=qweight.device, dtype=torch.int32).view(8, 1)
    q = (qweight.unsqueeze(-2) >> shifts) & 0xF                       # [..., K/8, 8, N]
    lead, k8, n = qweight.shape[:-2], qweight.shape[-2], qweight.shape[-1]
    groups = scales.shape[-2]
    w = (q.to(dtype) - 8).reshape(*lead, groups, (k8 * 8) // groups, n) * scales.to(dtype).unsqueeze(-2)
    return w.reshape(*lead, k8 * 8, n)


def make_int4_experts():
    import torch.nn as nn
    import torch.nn.functional as F

    class Int4MoEExperts(nn.Module):
        """Routed experts kept int4: w13 = gate||up [E, H/8, 2I], w2 [E, I/8, H] (+ fp16 group scales)."""

        def __init__(self, num_experts, hidden, intermediate, *, fp8=False, dtype=None, device=None):
            super().__init__()
            g = 128
            self.num_experts, self.hidden, self.intermediate, self.fp8 = num_experts, hidden, intermediate, False
            self.register_buffer("w13_qweight", torch.empty((num_experts, hidden // 8, 2 * intermediate),
                                                            dtype=torch.int32, device=device))
            self.register_buffer("w13_qscales", torch.empty((num_experts, hidden // g, 2 * intermediate),
                                                            dtype=torch.float16, device=device))
            self.register_buffer("w2_qweight", torch.empty((num_experts, intermediate // 8, hidden),
                                                           dtype=torch.int32, device=device))
            self.register_buffer("w2_qscales", torch.empty((num_experts, intermediate // g, hidden),
                                                           dtype=torch.float16, device=device))

        def forward(self, x, topk_ids, topk_weights):
            tokens, top_k = topk_ids.shape
            if topk_ids.numel() <= 64:
                ids = topk_ids.reshape(-1)
                w13 = unpack_int4(self.w13_qweight[ids], self.w13_qscales[ids], x.dtype)    # [M, H, 2I]
                w2 = unpack_int4(self.w2_qweight[ids], self.w2_qscales[ids], x.dtype)       # [M, I, H]
                rows = x.repeat_interleave(top_k, dim=0).unsqueeze(1)                       # [M, 1, H]
                gate, up = torch.bmm(rows, w13).squeeze(1).chunk(2, dim=-1)
                out = torch.bmm((F.silu(gate) * up).unsqueeze(1), w2).squeeze(1)
                out = out.float() * topk_weights.reshape(-1, 1).float()
                return out.view(tokens, top_k, self.hidden).sum(dim=1).to(x.dtype)
            flat = topk_ids.reshape(-1)
            order = torch.argsort(flat, stable=True)
            token_rows = order // top_k
            pair_w = topk_weights.reshape(-1)[order]
            counts = torch.bincount(flat, minlength=self.num_experts).tolist()
            out = torch.zeros(tokens, self.hidden, dtype=torch.float32, device=x.device)
            start = 0
            for e, c in enumerate(counts):
                if c == 0:
                    continue
                rows, w = token_rows[start:start + c], pair_w[start:start + c]
                start += c
                w13 = unpack_int4(self.w13_qweight[e], self.w13_qscales[e], x.dtype)
                w2 = unpack_int4(self.w2_qweight[e], self.w2_qscales[e], x.dtype)
                gate, up = (x[rows] @ w13).chunk(2, dim=-1)
                out.index_add_(0, rows, ((F.silu(gate) * up) @ w2).float() * w[:, None].float())
            return out.to(x.dtype)

    return Int4MoEExperts


EXPERT_RE = re.compile(r"model\.language_model\.layers\.(\d+)\.mlp\.experts\.(\d+)\.(gate_proj|up_proj|down_proj)\."
                       r"(qweight|scales)$")
DENSE_RE = re.compile(r"(.+)\.(qweight|scales|qzeros|g_idx)$")


def int4_backbone_stream(ckpt, n_experts, inter, stats):
    """Checkpoint tensors for load_backbone_weights: stacked int4 experts under the Int4MoEExperts buffer names,
    dense int4 projections unpacked to fp32 [N, K] `.weight` (the loader casts and clamps to fp16)."""
    from safetensors import safe_open
    index = json.load(open(f"{ckpt}/model.safetensors.index.json"))["weight_map"]
    shards = sorted(set(index.values()))
    skip = ("audio_decoder.", "audio_decoder_mtp_shared.", "audio_decoder_mtp.")
    stage_e = {}   # layer -> dict of stacked tensors + count
    dense = {}
    for shard in shards:
        with safe_open(f"{ckpt}/{shard}", framework="pt", device="cpu") as f:
            for key in f.keys():
                if key.startswith(skip):
                    continue
                m = EXPERT_RE.match(key)
                if m:
                    layer, e, proj, kind = int(m[1]), int(m[2]), m[3], m[4]
                    t = f.get_tensor(key)
                    st = stage_e.get(layer)
                    if st is None:
                        st = stage_e[layer] = {"n": 0}
                    name = ("w2_" if proj == "down_proj" else "w13_") + ("qweight" if kind == "qweight" else "qscales")
                    if name not in st:
                        if name.startswith("w13"):
                            shape = (n_experts, t.shape[0], 2 * t.shape[1])
                        else:
                            shape = (n_experts, *t.shape)
                        st[name] = torch.empty(shape, dtype=t.dtype)
                    if proj == "down_proj":
                        st[name][e].copy_(t)
                    else:
                        half = t.shape[1]
                        sl = slice(0, half) if proj == "gate_proj" else slice(half, 2 * half)
                        st[name][e][:, sl].copy_(t)
                    st["n"] += 1
                    if st["n"] == n_experts * 6:
                        prefix = f"model.language_model.layers.{layer}.mlp.experts."
                        for nm in ("w13_qweight", "w13_qscales", "w2_qweight", "w2_qscales"):
                            yield prefix + nm, st[nm]
                        del stage_e[layer]
                        stats["expert_layers"] += 1
                    continue
                m = DENSE_RE.match(key)
                if m:
                    base, kind = m[1], m[2]
                    d = dense.setdefault(base, {})
                    d[kind] = f.get_tensor(key)
                    if len(d) == 4:
                        qz = d["qzeros"]
                        if not bool((qz == torch.tensor(0x88888888 - 2**32, dtype=torch.int32)).all()):
                            raise ValueError(f"{base}: qzeros are not the symmetric 8")
                        k = d["qweight"].shape[0] * 8
                        if not torch.equal(d["g_idx"].long(), torch.arange(k) // (k // d["scales"].shape[0])):
                            raise ValueError(f"{base}: g_idx is not sorted groups")
                        w = unpack_int4(d["qweight"], d["scales"].float(), torch.float32).T.contiguous()
                        stats["dense"] += 1
                        del dense[base]
                        yield base + ".weight", w
                    continue
                yield key, f.get_tensor(key)
    if stage_e or dense:
        raise ValueError(f"incomplete int4 tensors: experts layers {sorted(stage_e)[:5]}, dense {sorted(dense)[:5]}")


def load_drama3_int4(ckpt, devices, tower_device, dtype):
    from tokenizers import Tokenizer
    from fish_speech.models.text2semantic.fish_omni import moe as fmoe
    from fish_speech.models.text2semantic.fish_omni import model as fmodel
    from fish_speech.models.text2semantic.fish_omni.backbone import (CHECKPOINT_BACKBONE_PREFIX,
                                                                     CHECKPOINT_LM_HEAD_KEY, FishOmniBackbone,
                                                                     load_backbone_weights)
    from fish_speech.models.text2semantic.fish_omni.config import FishOmniConfig
    from fish_speech.models.text2semantic.fish_omni.tower import FishOmniAudioDecoder, load_audio_decoder_weights

    raw = json.load(open(f"{ckpt}/config.json"))
    raw.pop("quantization_config", None)
    config = FishOmniConfig.from_dict(raw)
    fmoe.MoEExperts = make_int4_experts()
    with torch.device("meta"):
        backbone = FishOmniBackbone(config.text, fp8_experts=False, dtype=dtype)
        decoder = FishOmniAudioDecoder(config, dtype=dtype)
    backbone.materialize(devices)
    stats = {"expert_layers": 0, "dense": 0}
    load_backbone_weights(backbone, int4_backbone_stream(ckpt, config.text.num_experts,
                                                         config.text.moe_intermediate_size, stats))
    decoder.to_empty(device=tower_device)
    from pathlib import Path
    load_audio_decoder_weights(decoder, fmodel.iter_checkpoint_tensors(
        Path(ckpt), skip_prefixes=(CHECKPOINT_BACKBONE_PREFIX, CHECKPOINT_LM_HEAD_KEY)))
    tok = Tokenizer.from_file(f"{ckpt}/tokenizer.json")
    log["int4_load"] = stats
    return fmodel.FishOmniModel(config, backbone, decoder, tok).eval()


def selftest_unpack():
    """Round trip against quantize.py's packing (value k at bits 4*(k%8) of word k//8, minus 8, times scale)."""
    g = torch.Generator().manual_seed(0)
    n, k = 16, 256
    q = torch.randint(0, 16, (n, k), generator=g, dtype=torch.int64)
    qt = q.T.contiguous().view(k // 8, 8, n)
    word = torch.zeros(k // 8, n, dtype=torch.int64)
    for j in range(8):
        word |= qt[:, j, :] << (4 * j)
    qweight = ((word + 2**31) % 2**32 - 2**31).to(torch.int32)
    s = torch.rand(k // 128, n, generator=g) + 0.5
    ref = ((q.float() - 8) * s.T.repeat_interleave(128, 1)).T          # [K, N]
    got = unpack_int4(qweight, s, torch.float32)
    err = float((got - ref).abs().max())
    log["unpack_selftest_max_abs_err"] = err
    if err > 1e-6:
        raise AssertionError(f"int4 unpack mismatch {err}")


# ------------------------------------------------------------------------------------------------------- synthesis
def encode_ref(path, codec, device):
    wave, sr = sf.read(path, dtype="float32")
    if wave.ndim > 1:
        wave = wave.mean(1)
    wave = resample(wave, sr, codec.sample_rate)
    dtype = next(codec.parameters()).dtype
    x = torch.from_numpy(wave).to(device=device, dtype=dtype)[None, None]
    with torch.inference_mode():
        idx, lengths = codec.encode(x, torch.tensor([x.shape[-1]], device=device, dtype=torch.long))
    return idx[0, :, : lengths[0]].cpu()


def render(model_key, model, decode_one_token, device, codec, codec_device, gen_kwargs, budget_s, devices):
    from fish_speech.models.text2semantic.inference import decode_to_audio, generate_long
    refs = json.load(open(f"{WORK}/refs.json"))
    ref_codes = {v: encode_ref(f"{spec['input_dir']}/{r['file']}", codec, codec_device) for v, r in refs.items()}
    log["ref_frames"] = {v: int(c.shape[1]) for v, c in ref_codes.items()}

    def synth(text, voice, seed):
        torch.manual_seed(seed)
        torch.cuda.manual_seed_all(seed)
        for d in devices:
            torch.cuda.synchronize(d)
        t0 = time.perf_counter()
        codes = []
        for r in generate_long(model=model, device=device, decode_one_token=decode_one_token, text=text,
                               prompt_text=[refs[voice]["transcript"]], prompt_tokens=[ref_codes[voice]],
                               **gen_kwargs):
            if r.action == "sample":
                codes.append(r.codes.cpu())
        for d in devices:
            torch.cuda.synchronize(d)
        t1 = time.perf_counter()
        if not codes:
            raise RuntimeError("no audio frames")
        merged = torch.cat(codes, dim=1)
        with torch.inference_mode():
            audio = decode_to_audio(merged.to(codec_device), codec).float().cpu().numpy()
        for d in devices:
            torch.cuda.synchronize(d)
        t2 = time.perf_counter()
        if not np.isfinite(audio).all():
            raise FloatingPointError("non-finite audio")
        return audio, int(merged.shape[1]), len(codes), t1 - t0, t2 - t1

    audio, frames, chunks, g, d = synth("The library opens at nine on weekdays.", "female", 1)
    rms = float(np.sqrt(np.mean(audio**2)))
    log["smoke"] = {"audio_s": round(audio.size / codec.sample_rate, 2), "frames": frames, "gen_s": round(g, 2),
                    "rms": round(rms, 4)}
    event(f"smoke: {log['smoke']}")
    if rms < 1e-3 or audio.size / codec.sample_rate < 0.8:
        raise RuntimeError(f"smoke test produced silence or almost nothing: {log['smoke']}")

    for it in spec["items"]:
        if time.time() - T0 > budget_s:
            log["errors"].append("time budget reached")
            event("time budget reached")
            break
        label = f"fish_{model_key}_{it['voice']}_{it['cond']}"
        path = f"{OUT}/raw/{label}/{it['passage']}/{it['file']}"
        os.makedirs(os.path.dirname(path), exist_ok=True)
        rec = {"label": label, "passage": it["passage"], "file": it["file"]}
        try:
            audio, frames, chunks, g, d = synth(it["spoken"], it["voice"], it["seed"])
            sf.write(path, resample(audio, codec.sample_rate, OUT_SR), OUT_SR, subtype="PCM_16")
            dur = audio.size / codec.sample_rate
            rec.update({"audio_s": round(dur, 3), "frames": frames, "chunks": chunks, "gen_s": round(g, 3),
                        "decode_s": round(d, 3), "rtf_gen": round(g / dur, 3), "rtf_total": round((g + d) / dur, 3),
                        "frames_per_s": round(frames / g, 2)})
        except Exception as e:  # noqa: BLE001
            rec["error"] = traceback.format_exc()[-1500:]
            event(f"{label}/{it['passage']}/{it['file']}: {e!r}")
        log["items"].append(rec)
        log["memory"] = mem(devices)
        log["nvidia_smi_now"] = smi()
        if "audio_s" in rec:
            event(f"{label}/{it['passage']}/{it['file']}: {rec['audio_s']} s audio, gen {rec['gen_s']} s, "
                  f"RTF {rec['rtf_total']}")
        save()


def download(repo, revision, patterns, local):
    from huggingface_hub import snapshot_download
    t = time.time()
    root = snapshot_download(repo, revision=revision, allow_patterns=patterns, local_dir=local)
    size = sum(os.path.getsize(os.path.join(dp, f)) for dp, _, fs in os.walk(root) for f in fs)
    log["download"] = {"repo": repo, "revision": revision, "seconds": round(time.time() - t, 1),
                       "gib": round(size / 2**30, 2)}
    event(f"downloaded {repo}@{revision[:10]}: {size / 2**30:.2f} GiB in {time.time() - t:.0f} s")
    return root


def stage_drama3():
    os.environ["FISH_DRAMA3_CUDA_GRAPH"] = "0"
    from fish_speech.models.text2semantic.inference import load_codec_model
    m = spec["models"]["drama3"]
    selftest_unpack()
    local = "/tmp/hf/drama3"
    try:
        ckpt = download(m["repo"], m["revision"], m["files"], local)
        n = torch.cuda.device_count()
        devices = [torch.device(f"cuda:{i}") for i in range(n)]
        log["devices"] = [torch.cuda.get_device_name(d) + f" sm{torch.cuda.get_device_capability(d)}"
                          for d in devices]
        tower = devices[-1]
        t = time.time()
        model = load_drama3_int4(ckpt, devices, tower, torch.float16)
        torch.cuda.synchronize()
        log["load_s"] = round(time.time() - t, 1)
        log["memory_after_load"] = mem(devices)
        log["nvidia_smi_after_load"] = smi()
        event(f"model loaded in {log['load_s']} s; {log['nvidia_smi_after_load']}")
        codec = load_codec_model(f"{ckpt}/codec.pth", str(tower), torch.float16)
        log["nvidia_smi_after_codec"] = smi()
        render("drama3", model, None, str(tower), codec, str(tower), m["gen"], spec["budget_s"]["drama3"], devices)
    finally:
        shutil.rmtree(local, ignore_errors=True)


def stage_s2pro():
    from fish_speech.models.text2semantic.inference import init_model, load_codec_model
    m = spec["models"]["s2pro"]
    local = "/tmp/hf/s2pro"
    try:
        ckpt = download(m["repo"], m["revision"], m["files"], local)
        dev = "cuda:0"
        log["devices"] = [torch.cuda.get_device_name(0) + f" sm{torch.cuda.get_device_capability(0)}"]
        t = time.time()
        model, decode_one_token = init_model(ckpt, dev, torch.float16, compile=False)
        model.config.max_seq_len = m["cache_len"]
        with torch.device(dev):
            model.setup_caches(max_batch_size=1, max_seq_len=m["cache_len"], dtype=torch.float16)
        model._cache_setup_done = True
        torch.cuda.synchronize()
        log["load_s"] = round(time.time() - t, 1)
        log["memory_after_load"] = mem([torch.device(dev)])
        log["nvidia_smi_after_load"] = smi()
        codec = load_codec_model(f"{ckpt}/codec.pth", dev, torch.float16)
        event(f"model loaded in {log['load_s']} s; {smi()}")
        render("s2pro", model, decode_one_token, dev, codec, dev, m["gen"], spec["budget_s"]["s2pro"],
               [torch.device(dev)])
    finally:
        shutil.rmtree(local, ignore_errors=True)


try:
    log["torch"] = torch.__version__
    {"refs": stage_refs, "asr": stage_asr, "drama3": stage_drama3, "s2pro": stage_s2pro}[STAGE]()
    log["ok"] = True
except Exception:  # noqa: BLE001
    log["ok"] = False
    log["errors"].append(traceback.format_exc()[-4000:])
    print(log["errors"][-1], flush=True)
finally:
    save()
    gc.collect()
