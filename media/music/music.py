# Streams Reader promo music bed: ACE-Step v1 3.5B (Apache-2.0), fallback MusicGen (CC-BY-NC weights).
import subprocess, sys, os, time, json, traceback
OUT = "/kaggle/working"
PROMPT = ("ambient, lo-fi, cinematic, calm, hopeful, warm felt piano, soft analog pads, gentle sub bass, "
          "light brushed drums, vinyl crackle, night, dreamy, instrumental, 78 bpm")
log = {"prompt": PROMPT, "runs": []}
def sh(c):
    print("+", c, flush=True); r = subprocess.run(c, shell=True); return r.returncode
t0 = time.time()
ok = False
try:
    sh("nvidia-smi")
    rc = sh("pip install -q --no-deps git+https://github.com/ace-step/ACE-Step.git")
    rc2 = sh("pip install -q diffusers loguru pypinyin py3langid==0.2.2 hangul-romanize num2words spacy cutlet 'fugashi[unidic-lite]' pytorch_lightning peft librosa soundfile accelerate")
    log["pip_deps_rc"] = rc2
    log["pip_rc"] = rc
    import torch
    from acestep.pipeline_ace_step import ACEStepPipeline
    torch.backends.cudnn.enabled = False  # T4 has no cuDNN engine for bf16 conv2d (DCAE decoder)
    bf16 = torch.cuda.is_available() and torch.cuda.is_bf16_supported()
    dtype = "bfloat16" if bf16 else "float32"
    log["dtype"] = dtype; log["gpu"] = torch.cuda.get_device_name(0) if torch.cuda.is_available() else None
    pipe = ACEStepPipeline(checkpoint_dir=None, dtype="bfloat16", torch_compile=False, cpu_offload=True)
    for i, seed in enumerate([20261030, 7741]):
        t = time.time()
        path = f"{OUT}/acestep_bed_{i}.wav"
        pipe(audio_duration=60.0, prompt=PROMPT, lyrics="[instrumental]", infer_step=60, guidance_scale=15.0,
             scheduler_type="euler", cfg_type="apg", omega_scale=10.0, manual_seeds=str(seed),
             guidance_interval=0.5, guidance_interval_decay=0.0, min_guidance_scale=3.0, use_erg_tag=True,
             use_erg_lyric=False, use_erg_diffusion=True, oss_steps="", guidance_scale_text=0.0,
             guidance_scale_lyric=0.0, save_path=path)
        log["runs"].append({"model": "ACE-Step/ACE-Step-v1-3.5B", "seed": seed, "file": os.path.basename(path),
                            "seconds": round(time.time() - t, 1)})
        ok = True
except Exception as e:
    log["acestep_error"] = traceback.format_exc()[-3000:]
    print(log["acestep_error"], flush=True)
if False:
    try:
        sh("pip install -q transformers scipy")
        from transformers import AutoProcessor, MusicgenForConditionalGeneration
        import torch, scipy.io.wavfile as wf
        proc = AutoProcessor.from_pretrained("facebook/musicgen-medium")
        m = MusicgenForConditionalGeneration.from_pretrained("facebook/musicgen-medium").to("cuda")
        inp = proc(text=[PROMPT], padding=True, return_tensors="pt").to("cuda")
        t = time.time()
        a = m.generate(**inp, max_new_tokens=1500, do_sample=True, guidance_scale=3.0)
        sr = m.config.audio_encoder.sampling_rate
        wf.write(f"{OUT}/musicgen_bed_0.wav", sr, a[0, 0].cpu().numpy())
        log["runs"].append({"model": "facebook/musicgen-medium", "file": "musicgen_bed_0.wav", "seconds": round(time.time() - t, 1)})
    except Exception:
        log["musicgen_error"] = traceback.format_exc()[-3000:]
        print(log["musicgen_error"], flush=True)
log["total_seconds"] = round(time.time() - t0, 1)
json.dump(log, open(f"{OUT}/music_log.json", "w"), indent=1)
print(json.dumps(log, indent=1))
