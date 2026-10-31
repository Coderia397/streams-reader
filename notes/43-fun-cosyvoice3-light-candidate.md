# Fun-CosyVoice3 0.5B: light-candidate measurement

Measurement of FunAudioLLM/Fun-CosyVoice3-0.5B-2512 (Apache-2.0) on a private Kaggle T4 for the voice search (note 39). Conclusion: open, run in progress.

## 2026-10-31

### Plan (open)
- Code and work files (scratchpad, not in the repo): `scratchpad/voice-search/measure/3-cosyvoice3/` (`cv3_jobs.py` dataset builder, `cv3_kernel.py` Kaggle launcher, `cv3_worker.py` refs/latency/render/score, `reqs.lock` uv lock for Linux, Python 3.10). Dataset `lbrahimali/streams-cv3-light-inputs`, kernel `lbrahimali/streams-cv3-light` (private, T4, internet on).
- Pins: CosyVoice git 074ca6dc (main, 2026-10-31), weights revision 29e01c4e (HF, last modified 2026-02-03), torch 2.3.1+cu121, transformers 4.51.3, onnxruntime 1.18.0 (CPU, prompt features only).
- Not run on this Mac: the needed weights are 5.4 GB (llm 2.0, flow 1.3, Qwen BlankEN 1.0, speech tokenizer 1.0) and the disk had 2.9-3.6 GB free (`df -h /`, 2026-10-31 10:55 UTC). The CPU number comes from the Kaggle machine's CPU instead.
- Acceptance checks: (A1) < 5 % failed items; (A2) RTF per setting; (A3) first-audio time, 1 and 3 sentences, cold and warm; (A4) WER and UTMOS with the bake-off's tools; (A5) prosody index; (A6) Opus 32 kbps clips for the listening pack.

## Open items
- Results: unblocked when the kernel run finishes.
