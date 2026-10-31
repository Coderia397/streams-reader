# Higgs TTS 3 4B expressiveness bakeoff

Plan and results for Higgs TTS 3 4B (Boson AI) through the transformers port `multimodalart/higgs-audio-v3-tts-4b-transformers` on a private Kaggle T4 x2 kernel, scored with `tools/expressiveness/prosody.py` against the Kokoro baseline (notes/20). Conclusion: untagged Higgs scores 82 % (female) and 85-92 % (male) of the human prosody index, above every Kokoro voice (af_heart 68 %), with mean WER 0.02 and RTF 1.6 on one T4; emotion tags raise the index to 76-128 % but on the male voice they move whole sentences up an octave with raised F3 and spectral centre, which looks like a change of voice rather than acting (assumed, not listened). The licence is research and non-commercial; the app cannot ship it without a Boson commercial licence.

## 2026-10-31

### Plan
- Code: `tools/expressiveness/bakeoff/higgs/` (`higgs_bakeoff.py` driver, `higgs_kernel.py` Kaggle script, `higgs_directions.json` hand tags, `analyze.py` artefact evidence). Kernel `lbrahimali/streams-higgs-bakeoff`, private, T4 x2, internet on, job spec embedded in the script.
- Pins (verified on the HF API 2026-10-31): port 30f01593, codec `bosonai/higgs-audio-v2-tokenizer` 815a03f1, Whisper large-v3-turbo 41f01f3f; transformers 5.5.4; Kaggle image torch 2.11.0+cu128.
- Conditions per voice and passage: plain (seed 1234), plain_s2 (seed 99), expr (`<|prosody:expressive_high|>` per segment), tagged (hand tags per sentence, one call per segment), tagged_sent (same tags, one call per sentence), director (Gemma director output mapped to Higgs tags). 36 sets, 206 items.
- Acceptance checks: (A1) kernel completes with < 5 % failed items; (A2) every set scored with frozen calibration; (A3) Whisper WER per set; (A4) RTF and peak VRAM; (A5) licences read on the model cards on 2026-10-31.

### Decision: synthetic reference voices, no LibriVox clones (decided by the assistant, 2026-10-31)
- The Higgs TTS 3 licence (Section IV(b)(ii)) forbids audio that clones or imitates any real person without that person's explicit, verifiable consent. LibriVox readers are real people who consented to public domain release of the recording, which is not consent to voice cloning.
- Run 1 (pushed 01:40 GMT, before this change reached Kaggle) cloned two LibriVox preambles. Its audio was deleted unscored; its timing and VRAM logs are kept (they do not depend on the voice).
- Run 2 references: Higgs zero-shot renders of a neutral 14 s text, seeds 1..40, Whisper WER <= 0.1, the candidate nearest 200 Hz (female) and 115 Hz (male) median F0.
- Rejected: LibriVox clones (licence), Kokoro voices as references (cloning another model's voice is permitted but tests Kokoro timbre, not Higgs).

### Run 1 (verified, kernel log, audio discarded)
- 182 items, 0 errors, fp16 on both T4s, synth stage 1190 s, whole kernel 1398 s.
- RTF on one T4 (fp16, transformers `generate_speech`, batch 1, warm): 2185 s generation for 1367 s of audio = 1.60 overall; per item median 1.65, range 1.50-2.65 (182 items); no item hit the token cap.
- Peak VRAM per worker: torch allocated 9.38 GB, reserved 9.93-10.06 GB, nvidia-smi 10.4 GB of 15 GB.

### Run 2 push (verified)
- 07:07 GMT one push refused ("Maximum batch GPU session count of 2 reached"); `scratchpad/higgs/wait_push.sh` polled every 5 min and pushed at 07:17 GMT when one slot was free (kernel version 2).

### Run 2 results (verified, `tools/expressiveness/bakeoff/higgs/results.md`, frozen calibration)
Measured with `uv run tools/expressiveness/prosody.py measure tools/expressiveness/bakeoff/higgs/audio`; 3 passages per set, % of human = ceiling_pct (Kokoro af_heart 68, am_michael 50, bf_emma 39, bm_george 21).

| set | female % | male % |
|---|---|---|
| plain (seed 1234) | 82 (76 / 89 / 82) | 85 (84 / 84 / 88) |
| plain_s2 (seed 99) | 81 (66 / 82 / 94) | 92 (84 / 110 / 84) |
| expr (expressive_high) | 98 (90 / 120 / 84) | 86 (88 / 104 / 65) |
| tagged (hand tags, per segment) | 76 (82 / 99 / 45) | 114 (115 / 128 / 100) |
| tagged_sent (hand tags, per sentence) | 91 (92 / 114 / 66) | 128 (113 / 158 / 112) |
| director (Gemma tags, per segment) | 84 (58 / 104 / 90) | 105 (83 / 133 / 100) |

Order in brackets: 1-suspense / 2-dialogue / 3-reflective.

- References (verified, run 2 log): female = zero-shot seed 3, 13.7 s, 209 Hz; male = seed 6, 12.5 s, 104 Hz; both WER 0.093 (only "nine"->"9", "ten"->"10", "drop box"->"dropbox"). Ref search took 215 s for 7 seeds.
- RTF (verified, run 2 worker logs): 2386 s generation for 1509 s of audio = 1.58 on one T4 fp16, per item median 1.62 (range 1.49-2.74, 206 items); 1.55 for paragraph calls, 1.71 for sentence calls. Model load 13-34 s. No item hit the token cap.
- Install (verified, run 1 and 2 logs): `pip install transformers==5.5.4` rc 0 on the Kaggle image (torch 2.11.0+cu128, CUDA 12.8); port loads with `trust_remote_code=True`; no errors in either run. Only problems: one push refused by the Kaggle 2-GPU-session limit (above), and run 1 redone because of the cloning clause.
- VRAM (verified): torch peak allocated 9.37-9.38 GB, reserved 9.87-9.91 GB, nvidia-smi peak 10.2-10.5 GB per T4. bf16 fallback never needed (fp16 warm-up sane).
- WER (verified, Whisper large-v3-turbo in the kernel): mean 0.021 over 206 clips, 7 clips above 0.2. Those are quote-mark tokens Whisper emits as words on the Tom line and the sfx onomatopoeia ("Haha", "Haah") dropped or kept; no skipped or repeated text found.
- Artefacts (verified, `analyze.py`, `artefacts.json`): no clipping, longest internal silence 0.85 s, median spectral flatness < 0.005, energy above 8 kHz -24 to -27 dB relative to 0-8 kHz on every set (codec at 24 kHz, no hiss signature).
- Per-clip median F0 (verified, Praat on the WebM clips): plain male 88-116 Hz in every clip; tagged male anger lines 231-325 Hz, longing lines 157-169 Hz. Formants on the anger clips: F3 2657-2903 Hz vs 2554-2563 plain, spectral centre 687-1175 Hz vs 389-409. Female plain stays 181-233 except the "Don't you dare" segment at 284-292 even untagged.
- Interpretation (assumed, no listening): untagged Higgs already acts the anger line on the female voice and keeps the narrator stable; tags on the male voice mostly add octave jumps and probable voice change, which inflates F0 level sd to 4.7-5.0 st (human 1.8). Timing stays less varied than human (ln-pause sd 0.28-0.43 vs 0.58) and the reflective passage is read at 3.7-4.2 syl/s vs the human 3.2.

### Licences (verified on the Hub 2026-10-31)
- Weights `bosonai/higgs-tts-3-4b` (sha 239f63fb): "Boson Higgs TTS 3 Research and Non-Commercial License Agreement", last updated July 8, 2026. Research and non-commercial use only; hosted use, API, SaaS or any product requires a separate written commercial licence (Section III). Section II-A Creator Use Grant allows monetised creator content with an acknowledgement, and excludes hosting or embedding in a product. Section IV(b)(ii) bans cloning a real person without explicit, verifiable consent. Not open source (licence text says so).
- Port `multimodalart/higgs-audio-v3-tts-4b-transformers` (sha 30f01593, weights and modelling code): "Boson Higgs Audio v3 Research and Non-Commercial License Agreement", last updated May 21, 2026; same research and non-commercial terms, without the Creator Use Grant.
- Codec `bosonai/higgs-audio-v2-tokenizer` (sha 815a03f1): "Boson Higgs Audio 2 Community License", based on the Meta Llama 3 Community License; commercial use allowed below 100,000 annual active users, "Built with Higgs Materials licensed from Boson AI USA, Inc." attribution.
- Re-checked 2026-10-31 11:42 GMT (HF API): weights 239f63fb, port 30f01593, codec 815a03f1 unchanged; licence ids `boson-higgs-tts-3-research-and-non-commercial-license` and `bosonai-higgs-audio-v3`. Kernel `lbrahimali/streams-higgs-bakeoff` status COMPLETE; no rerun pushed (results above stand).
- Consequence for Streams Reader: free to evaluate; not usable in the app (hosted product) without a Boson commercial licence.

## Open items
- Speaker similarity of the tagged male clips against the reference (ECAPA or WavLM-SV inside Kaggle) would turn the voice-change reading from assumed to verified. Unblocks: a decision that Higgs is worth pursuing despite the licence.
- A Boson commercial licence price. Unblocks: any product use.
