# VoxCPM 1.5 / VoxCPM2: light-candidate measurement

Measurement of openbmb/VoxCPM1.5 (0.8B, speed) and openbmb/VoxCPM2 (2B, Arabic and style control) on a private Kaggle T4 for the voice search (note 39). Conclusion: open, run in progress.

## 2026-10-31

### Facts (verified 2026-10-31 on the model cards and GitHub)
- VoxCPM1.5: 0.8B, BF16, Apache-2.0, English and Chinese only, 44.1 kHz VAE, released 2025-12-05; card RTF 0.17 on RTX 4090; "limited direct control over ... emotion or speaking style" (https://huggingface.co/openbmb/VoxCPM1.5).
- VoxCPM2: 2B, Apache-2.0, 30 languages including Arabic, 48 kHz out, about 8 GB VRAM, released 2026-04; card RTF about 0.30 on RTX 4090 and about 0.13 with Nano-vLLM; style instruction in parentheses before the text ("(slightly faster, cheerful tone)..."), voice design from a text description with no reference; 1,009,312 downloads last month (https://huggingface.co/openbmb/VoxCPM2, https://github.com/OpenBMB/VoxCPM).
- Ports listed on GitHub: VoxCPM-ONNX (CPU), Nano-vLLM-VoxCPM, vLLM-Omni, llama.cpp-omni and VoxCPM.cpp (GGUF, CPU/Metal/CUDA/Vulkan), VoxCPMANE (Apple Neural Engine).

### Plan (open)
- Code and work files (scratchpad, not in the repo): `scratchpad/voice-search/measure/2-voxcpm/code/` (`vox_kaggle.py` jobs/push/pull/collect/clips, `vox_kernel.py` launcher, `vox_worker.py` speed and renders, `vox_score.py` Whisper + UTMOS, `vox_analyze.py` summary). Dataset `lbrahimali/streams-voxcpm-measure-inputs`, kernel `lbrahimali/streams-voxcpm-measure` (private, T4 x2, internet on).
- Pins: voxcpm 2.0.3 (PyPI), torch 2.8.0+cu128, Python 3.11 uv venv; weights VoxCPM1.5 8cdc4038, VoxCPM2 32279eff; Whisper large-v3-turbo 41f01f3f, UTMOS22 strong (SpeechMOS v1.2.0), as notes 37 and 38. cfg 2.0, 10 diffusion steps (package defaults), seed 1234, no denoiser.
- GPU 0 runs 1.5 and GPU 1 runs 2 in parallel. Speed: fresh process per config (fp16 + torch.compile, fp16 eager, bf16 eager), cold 1- and 3-sentence streamed requests, then 3 warm of each; renders with the fastest config; CPU speed on the Kaggle host after the GPU work.
- Not run on this Mac: it is an Intel i7-9750H (no MPS or MLX), and the disk had 2.2 GB free (`df -h /`, 2026-10-31 11:42 UTC) against about 2 GB (1.5) and 4.6 GB (2) of weights.
- Voices: Kokoro af_heart 14 s reference (note 32), the two LibriVox references of note 37. Sets: 1.5 continuation clone per voice; 2 reference clone per voice, plain, with a style tag per segment, and per sentence with director styles; 2 af_heart "ultimate" clone (reference + continuation); 2 voice design; Arabic (3 MSA paragraphs) with af_heart clone, male clone and voice design.
- Acceptance checks: (A1) < 5 % failed items; (A2) RTF per config; (A3) first audio for 1 and 3 sentences, cold and warm; (A4) WER (CER for Arabic) and UTMOS; (A5) prosody index with the frozen calibration, as a filter; (A6) Opus 32 kbps clips per passage in `measure/2-voxcpm/clips`.

### Run log
- 2026-10-31 11:02-11:37 UTC: kernel push refused 8 times ("Maximum batch GPU session count of 2 reached").
- 2026-10-31 11:43 UTC: found and fixed a bug before any run: the worker's `sync()` called itself instead of `torch.cuda.synchronize()`, which would have failed every CUDA item with a RecursionError. Dataset re-uploaded; push retried every 5 min.

## Open items
- Results: unblocked when the kernel gets a GPU slot and finishes.
