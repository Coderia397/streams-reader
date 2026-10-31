# Voice quality and speed: owner guidance and the search plan

Brief. The owner's guidance on 2026-10-31 for fixing the voices, and how the search is run. Conclusion: open; the goal is human-sounding, emotional reading that comes back fast for any document size, chosen by the owner's ears.

## 2026-10-31
Owner guidance (guidance, not decisions; the lead finds the best answer):
- Current voices are not good enough: Kokoro af_heart sounds good but lacks expression; the others lack naturalness and quality (owner listening, 2026-10-31). The prosody index is a filter, not the judge (correction in note 35).
- Possibility 1: a middle-ground model, light enough to be fast and cheap, strong enough to match the expressive models (human emotion).
- Possibility 2: split rendering across several compute backends with a Worker scheduler that manages them smartly. The owner calls this a workaround, not the solution: worth having for speed even after a good model is found, but it does not replace finding the model.
- What counts is the result for the user: a short, medium or long document must not take long to render and send back. Improve quality, speed, the architecture and the process together.

Search plan (workflow launched 2026-10-31):
1. Survey of light expressive TTS models current in 2026 (size, speed on T4 and on CPU/Apple silicon/WebGPU, licence, emotion, cloning, Arabic).
2. Survey of compute: free and cheap GPU sources, Workers AI speech models, and the user's own device (WebGPU in the browser, ONNX in the native apps) as a renderer.
3. Speed architecture: time to first audio, render ahead of the playhead, parallel segments across backends, priorities, caching.
4. Measure the top light candidates on Kaggle T4 and locally (real-time factor, first-segment latency) and add their clips to the blind listening pack.
5. Synthesis: a recommended engine plus architecture, with numbers, for the owner to approve after listening.

## 2026-10-31 13:00: owner listening (bake-off clips, plain/untagged settings)
- The owner judged on the plain (untagged) renders, passage "The lighthouse keeper finished his supper at seven…". Status: verified (owner's ears), informal, not blind.
- Ranking: 1 Qwen3-TTS (clearly best); 2 Fish (likely); 3-4 Higgs and IndexTTS-2.5 (both good, order unsure); last Chatterbox ("definitely not").
- Owner direction (guidance): a mixture of models may be best (different models for different jobs), possibly combined with several servers/services. The recommendation must weigh this ranking above the prosody index.

## Open items
- Blind listening pack: waits for the bake-off and step 4.
- Engine choice: waits for the owner's listening.
