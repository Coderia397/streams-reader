> Correction 2026-10-30: the user's phone is a **Google Pixel 8, Android 17 (SDK 37), Chrome installed**, verified over ADB (`adb devices -l`, `getprop`). The "A12" below came from the Claude chat and is superseded; Samsung One UI battery steps do not apply.

# What the chats concluded, where they agree, and where the Claude chat was wrong

Finding. Both chats end at the same shape: Rust for everything except a thin Python/PyTorch model call in the user's second free ZeroGPU Gradio Space. The Claude chat missed that Cloudflare (free, no card, already in use) can be the always-on backend and cache; the ChatGPT chat proposed Workers + R2. The plan merges both.

## 2026-10-30

### User requirements (collected from both chats)
- Replace the ElevenLabs Reader subscription. Personal use only. Android phone (the Claude chat refers to an A12).
- Expressive reading inferred from the text itself, high quality, many preset voices, cloning of the user's own voice.
- FOSS preferred (user: "probably FOSS is better"); the user manages everything ("think of me as a Linux guy").
- £0 infrastructure. No credit card (Oracle account banned, no other card). Free trials are unwanted; renewing free tiers are acceptable.
- The laptop is not a server (2019 MacBook Pro 16", Intel, 32 GB RAM, 8 GB GPU, not on 24/7).
- Multilingual (Arabic) is nice to have, not required.
- Rust wherever possible, multiple layers, compiled; Gradio ZeroGPU Space as the GPU backend. Free accounts get 2 ZeroGPU Spaces; slot 1 is `streams-engine` (recommendation engine for the user's Streams app).
- "I just want it to just work": layered architecture covering speed, quality, expressiveness and formats.

### How each chat's recommendation moved
Claude: Drama 3 → Drama 3 + LLM director + Qwen VoiceDesign presets + ASR check + Kokoro fallback → free tiers (Modal, Fish free API, Gemini voices, Kaggle) → FOSS (Ming-3.0-tts 0.5B, Chatterbox Multilingual V3 for Arabic, MOSS-TTS-Nano on CPU, llama.cpp director, whisper.cpp) → Oracle VM + Modal → no card: phone as backend + Kaggle + Piper → ZeroGPU Space #2 + Kaggle bulk + Piper offline → Rust `reader-core` crate (PyO3 wheel for the Space, UniFFI for Android), thin Python Space shell, Kotlin playback service, Rust Gradio client.

ChatGPT: MOSS-TTS v1.5 → Drama 3 first (after checking the Claude answer) → keeps Qwen3 + Kokoro as secondary → free: Gemini 3.8 Flash TTS + Cloudflare Workers + R2 → Rust app + thin Python Gradio adapter on ZeroGPU, Drama 3 to test, Qwen3 alternatives, director, persistent cache.

### Agreed by both (adopted)
- All inference off the phone; the phone is a client with a local audio cache.
- Clone from a clean reference clip plus its exact transcript; record the reference with real expressive range.
- Paragraph-sized chunks with surrounding context; render ahead of playback; cache everything.
- An LLM "director" pass in front of the TTS model, exposed as a toggle.
- An ASR check on generated chunks (better than a duration heuristic).
- An engine-agnostic API so models can be swapped by configuration.
- Kokoro is a preset/fallback engine, not the main expressive engine.
- ZeroGPU requires the Gradio SDK and a Python/PyTorch call inside `@spaces.GPU`; Docker Spaces cannot use ZeroGPU.

### Where the Claude chat was wrong or inconsistent (checked against the ChatGPT chat)
1. Fixed seed for voice consistency (Claude msg 2). ChatGPT msg 10 cites Fish docs: a fixed seed does not fix timbre. Correction: consistency comes from the saved reference clip + transcript and fixed sampling settings; the seed only aids reproducibility.
2. Chatterbox score (Claude msg 2 says 1024 "on the same arena" as the cloned-voice scores). ChatGPT msg 10: 1024 is the native-voice score; controlled (cloned) is 935. Claude msg 4 itself later uses 935, so the chat is internally inconsistent.
3. Per-chunk `loudnorm` (Claude msg 4, from Answer A). ChatGPT msg 13: per-paragraph dynamic normalisation flattens emotional dynamics. Correction: measure each chunk's integrated loudness (EBU R128) and apply one static gain toward a chapter target; no dynamic compression.
4. Director output format. Claude: the LLM rewrites the paragraph with tags, then a diff rejects word changes. ChatGPT: the LLM returns structured annotations with character offsets and code inserts the tags. Adopted: structured spans, so the source text can never be altered.
5. "Without a card there's no free Linux box that stays on" (Claude msg 12) led to "the phone is the backend". True for VMs, but Cloudflare Workers + D1 + R2 is a free, always-on, no-card backend the user already uses (R2 bucket and 15 Workers exist). ChatGPT msg 15 proposed Workers + R2. Adopted.
6. "Hugging Face now requires a paid plan to create Gradio Spaces" (Claude msg 12). The user corrected it (free ZeroGPU Spaces, up to 2); Claude conceded in msg 14. The user's existing Space runs on `zero-a10g` hardware (verified via HF API, 2026-10-30).
7. ZeroGPU quota figures (Claude: 5 GPU min/day, ~3 runs/day reported April 2026; ChatGPT: 5 min/day shared across all of a user's ZeroGPU Spaces). Open: being verified in research, see [03-research-verification](03-research-verification.md).
8. Model facts dated after the assistant's knowledge cutoff (Ming-3.0-tts, Drama 3, MOSS-TTS-Nano, Chatterbox Multilingual V3, arena scores). Open: being verified, see 03.

### Not carried forward
- Paid or trial compute (Modal, rented 4090s, Fish free API ending 2026-11-30, Gemini TTS): user wants FOSS and self-managed.
- Oracle VM: account banned.
- Laptop as server: user ruled it out.
- Drama 3 as the main engine: Fish Audio Community License is not FOSS, the FP8 weights are ~37 GiB, and a ZeroGPU Space has a non-persistent disk. Kept as an optional future engine.

## Open items
- ZeroGPU quota and model facts: unblocked by the research workflow (note 03).
