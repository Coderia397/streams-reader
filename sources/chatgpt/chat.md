# Chat transcript: Self-hosted / free TTS reader research

Exported: 2026-10-30 (Europe/London)

This export contains the visible user and assistant messages in this conversation. It does not include hidden system/developer instructions or internal tool payloads. Citation reference IDs are preserved as they appeared in the conversation; they may not resolve outside the original chat.

---

## 1. USER

best foss alt for 11labs gen 4, i want ot use it for my phone I am okay with building a wrapper around it and building an app for it on the phone so that it can replace my 11 Labs reader subscription.I know how to code in multiple languages, so don't worry about that part.You just do the research for me, and I can make it run in a server, which is technically cheaper. Much cheaper.I just want as long... I just want it to be as expressive and has many voices, including voice cloning, my own or something like that. So, and stuff like that, etc., etc. Including also normal common presets of voices and good voices. Or just the model to run somewhere. I don't care, just anything, please.

---

## 2. ASSISTANT (commentary)

I’ll compare the strongest genuinely open models and self-hostable projects against the features that matter for a phone reader: long-form stability, expressive delivery, voice variety, cloning, streaming latency, hardware cost, and licence restrictions. I’ll distinguish model weights that are actually usable from projects that are merely open-source wrappers around hosted services.

---

## 3. ASSISTANT (commentary)

The strongest options are not all in the same category. The current field separates into lightweight, cheap narration models (especially Kokoro), voice-cloning models with more expressive control (Chatterbox and Qwen3-TTS), and heavier “maximum quality / experimentation” systems (Fish Speech and newer research releases). I’m checking the actual model cards and licences now, because several popular recommendations are not as free to use as their GitHub pages initially suggest.

---

## 4. ASSISTANT (commentary)

A particularly relevant find is MOSS-TTS: its maintainers explicitly recommend one model for long-form narration and cloning, while offering a separate 100M-parameter version that can run on CPU. That maps unusually well to a phone reader with a self-hosted backend. Qwen3-TTS and Chatterbox remain strong alternatives, especially for generated/custom voices and cross-language cloning.

---

## 5. ASSISTANT (commentary)

One correction to my initial shortlist: there is a newer contender worth testing before settling on anything. **Higgs TTS 3 (4B)** reports strong results in a cross-model evaluation that includes Qwen3-TTS, Fish Audio S2 Pro and MOSS-TTS v1.5, particularly for emotional and paralinguistic delivery. The important question is whether its weights and serving stack are genuinely self-hostable under a licence you can use—not just whether the demo sounds good.

---

## 6. USER

as well it has to be high quality, right? And stuff like that. So I want the cloning to also produce expressive stuff, just based on the actual text itself and the quality of expressiveness, right? All of that stuff, all together.It doesn't necessarily, I, like, I can do the hard work from my end, building backend, building front end, etc., etc.

---

## 7. ASSISTANT

Yes. That changes the recommendation: you are not just looking for a TTS model that can imitate your voice. You want a **self-hosted narration engine** that can take an ordinary piece of text, understand its emotional context, and read it naturally in a chosen or cloned voice.

For example, it should recognise that a passage is becoming tense, that a character is asking a question, that a sentence is sarcastic, or that a paragraph is reflective. It should express those differences through pacing, intonation, emphasis and tone without you having to manually insert emotion tags into every paragraph.

There is no open model I would confidently claim matches ElevenLabs v4 in every respect. But there are some genuinely promising options, and a combination of models may give you a better result than relying on one.

## My top recommendations

### 1. MOSS-TTS v1.5 — best starting point for your reader

<badge color="success">My recommended starting point</badge>

**Project:** [OpenMOSS/MOSS-TTS](https://github.com/OpenMOSS/MOSS-TTS)  
**Models:** [MOSS-TTS-v1.5](https://huggingface.co/OpenMOSS-Team/MOSS-TTS-v1.5) · [MOSS-TTS-Local-Transformer-v1.5](https://huggingface.co/OpenMOSS-Team/MOSS-TTS-Local-Transformer-v1.5)

This is where I would begin. Its main strengths are long-form narration, zero-shot voice cloning, multilingual speech and maintaining natural prosody over extended passages. The v1.5 model supports 31 languages, including English and Arabic, and its model documentation describes generation of continuous speech up to an hour. It also includes pause and duration controls, which are useful for reading books rather than producing isolated voice clips. <Cite refs={["turn637967view5","turn637967view6","turn637967view7","turn578473view2"]}/>

There are two versions worth looking at:

- **MOSS-TTS-v1.5 (8B Delay):** the one I would investigate first for long-form narration and stable voice cloning.
- **MOSS-TTS-Local-Transformer-v1.5:** a roughly 4B-class alternative designed with local inference and streaming in mind. It supports 48 kHz stereo output. <Cite refs={["turn559196view1","turn578473view2","turn140099search0"]}/>

An especially interesting part of the MOSS ecosystem is **MOSS-VoiceGenerator**. Rather than cloning a voice from a recording, it generates a new speaker identity from a text description. You can describe the voice you want and generate a sample. The project describes it as a voice-design component that can be used with downstream TTS systems. <Cite refs={["turn755215search1","turn755215search4"]}/>

That gives you a possible way to build a voice library: create different voices from descriptions, save their generated recordings, and use those recordings as reference voices for narration. You could offer presets such as a warm British narrator, a restrained documentary voice, a deep American voice or a more animated conversational voice. The consistency of each resulting voice would need testing.

The licensing is also attractive: the MOSS model family is released under Apache 2.0. <Cite refs={["turn733459view2"]}/>

**My assessment:** the strongest starting point if you want a genuinely open, self-hosted system that handles books and articles, with room to build a voice catalogue around it. I would test how naturally it changes emotional delivery from plain text, because long-form stability does not automatically mean the most convincing emotional performance.

### 2. Fish Audio Drama 3 — the one to audition for maximum expressiveness

<badge color="info">High-end expressive candidate</badge>

**Project:** [Fish Audio / fish-speech](https://github.com/fishaudio/fish-speech)  
**Model:** [Drama 3](https://huggingface.co/fishaudio/drama-3)

This is probably the most interesting model to test if expressive performance is your biggest priority.

Drama 3 is a 35B-parameter mixture-of-experts model, with around 3B parameters active per token. Its capabilities include voice cloning, long-form speech, multiple speakers, emotional direction and natural-language instructions. It also supports expressive delivery such as whispering, laughter, crying and anger. You can give it direction and revise the performance, rather than being limited to a fixed set of voice settings. <Cite refs={["turn578473view3","turn733459view7","turn542800search1"]}/>

The distinction I would test is whether it can deliver a convincing performance from the *plain text alone*, without explicit instructions. Its ability to follow emotional directions is useful, but that is not quite the same thing as understanding the prose and choosing the right delivery by itself.

There are two substantial drawbacks:

First, its published minimum hardware requirement is a 48 GB Nvidia GPU for the FP8 configuration. That makes it considerably more expensive to host than the smaller models. <Cite refs={["turn559196search0","turn559196search1"]}/>

Second, it is not permissively licensed FOSS. It uses the Fish Audio Community License. The terms allow personal hobbyist use, but hosted TTS services for third parties are restricted and may require a separate written licence. The licence also includes requirements concerning voice-cloning consent. Read the terms before building anything intended for other users. <Cite refs={["turn758751view0"]}/>

**My assessment:** audition this if you can access a sufficiently powerful GPU. It could be an excellent expressive engine, but I would not make it the foundation of a public app without resolving the licensing first.

### 3. Qwen3-TTS — excellent voice design and a useful second engine

**Project:** [QwenLM/Qwen3-TTS](https://github.com/QwenLM/Qwen3-TTS)

Qwen3-TTS is worth testing because it provides several capabilities in one family of models.

Its 1.7B models include:

- **VoiceDesign:** create a voice from a natural-language description.
- **CustomVoice:** use one of its supplied speaker voices, with instructions for delivery.
- **Base:** clone a voice from a short reference recording and its transcript.

The supplied CustomVoice model includes nine speaker voices, and the instruction system can guide delivery with descriptions such as being happy or angry. The Base cloning model is a separate checkpoint, however, and does not offer the same direct instruction controls in the model's documented interface. <Cite refs={["turn414466view0","turn578473view1"]}/>

That distinction matters for your requirements. You want to clone your own voice and then have that clone read naturally and expressively. Qwen provides voice cloning and voice design, but you should not assume its strongest voice-direction features transfer directly to the cloned-voice checkpoint.

It is Apache 2.0, and the 1.7B model is a fairly manageable size compared with Drama 3. One limitation is language coverage: its documented supported languages are ten major languages, and Arabic is not among them. <Cite refs={["turn422790search0","turn733459view4"]}/>

**My assessment:** a very good additional engine for building a range of voices. I would compare its cloned output against MOSS rather than make it your only model.

### 4. Higgs TTS 3 — promising text-driven emotion, but licensing is restrictive

**Model:** [Boson AI Higgs TTS 3](https://huggingface.co/bosonai/higgs-tts-3-4b)

This one deserves attention because its published evaluation specifically tested speech generated without manually inserted style tags. Its authors reported strong results for emotional and non-verbal performance compared with several competing models. It also supports voice cloning, a wide range of expressive controls and many languages. These are the developers' own evaluation results, not an independent guarantee that it will outperform the alternatives on your reading material. <Cite refs={["turn494774view2","turn559196view0","turn637967view1"]}/>

Unfortunately, its Research and Non-Commercial licence is restrictive. It does not give you the permissions you would want to embed the model in an application or offer it as a hosted TTS service. There are separate grants for certain creator uses, but those should not be confused with permission to build a general-purpose reading app around it. <Cite refs={["turn637967view0","turn637967view2"]}/>

**My assessment:** one of the most interesting models to compare when evaluating emotional inference, but I would not build your product around it without obtaining the appropriate licence.

### 5. Chatterbox Multilingual V3 — useful lighter-weight alternative

**Project:** [Resemble AI / Chatterbox](https://github.com/resemble-ai/chatterbox)

Chatterbox Multilingual V3 is around 0.5B parameters, supports voice cloning and lists 23 languages, including English and Arabic. The project is MIT-licensed. Its Turbo and Nano variants offer additional trade-offs for faster or smaller inference. <Cite refs={["turn578473view0","turn733459view6"]}/>

I would keep it as a cheaper comparison model or secondary option. It is considerably easier to experiment with than a 35B model, but its documented feature set does not give me the same confidence in long-form, text-driven emotional narration as MOSS or Drama 3.

### 6. Kokoro — good preset voices, but not a complete replacement

**Model:** [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M)

Kokoro is an 82M-parameter Apache 2.0 model. It is an impressive quality-to-size option and is worth considering for a lightweight fallback or a simple preset-voice mode. However, it does not provide the zero-shot voice cloning and voice-design capabilities you are looking for. <Cite refs={["turn917814search3","turn549841search1","turn549841search2"]}/>

I would not use it as the main engine for your app.

---

## How I would rank them for your requirements

| Model | Voice cloning | Text-driven expression | Long-form | Licence / concern |
|---|---|---|---|---|
| **MOSS-TTS v1.5** | Yes | Worth testing | Strong focus | Apache 2.0 |
| **Fish Drama 3** | Yes | Strong direction controls; test untagged text | Yes | Community licence, 48 GB GPU |
| **Qwen3-TTS 1.7B** | Yes | Strong voice design and CustomVoice instructions | Test for your use | Apache 2.0 |
| **Higgs TTS 3** | Yes | Particularly promising in its own untagged-text evaluation | Test for your use | Research / non-commercial restrictions |
| **Chatterbox V3** | Yes | More limited documented controls | Test for your use | MIT |
| **Kokoro** | No | Limited | Suitable for ordinary reading | Apache 2.0 |

## The architecture I would build

I would keep the TTS engine on your server and make the phone a relatively thin client. The phone should send text and a selected voice profile to the server, then stream back audio. It does not need to run the large model itself.

For the first version, I would build around:

- **MOSS-TTS-v1.5** as primary long-form narration and cloning.
- **MOSS-VoiceGenerator** for creating voice identities and presets.
- **Qwen3-TTS** as an alternative for voice design and cloning.

For long-form reading, I would not send each sentence as a separate generation request. Give the model enough surrounding text to understand the paragraph or scene, then generate manageable audio chunks. Preserve the order, pre-generate the next chunk while the current one plays, and cache the output so the user can return to a previous position without regenerating it.

For the voice-cloning side, keep a reference recording and its exact transcript for each cloned speaker. Give the model a clean recording with a natural range of expression. A completely flat reference recording can limit the character of the resulting voice; the voice sample is not just an identity label, it can influence the speaking style too. ElevenLabs' own cloning guidance makes a similar point about the speaking style of the source recording. <Cite refs={["turn716015search2"]}/>

## The most important test: emotional inference without instructions

This is where I would be particularly rigorous. A model that can produce `[angry]`, `[whispering]` or `[sad]` speech is not necessarily a model that can understand a piece of writing and independently decide how it should sound.

I would test each serious candidate using the *same* three passages:

1. **Narrative prose:** a passage that shifts from calm description to danger or suspense.
2. **Dialogue:** a passage containing questions, sarcasm, anger, uncertainty and changes in speaker.
3. **Reflective prose:** a passage that requires restraint, pauses and emphasis rather than theatrical acting.

Run each passage first as plain text, without emotion tags or extra instructions. That is the test that matters most for your reader. Then run it with style directions to see how much control the model offers when you need it.

Compare the results for natural emphasis, believable emotional transitions, pronunciation, voice consistency, pacing and whether the model starts sounding exaggerated after several minutes. A short demo can sound excellent while a chapter-length recording becomes monotonous or inconsistent.

## My final recommendation

I would build the first prototype with **MOSS-TTS-v1.5 and MOSS-VoiceGenerator**. That gives you a promising combination of long-form narration, cloning, custom voice creation, multilingual support and a permissive licence.

Then I would audition **Fish Audio Drama 3** against it if you can access a 48 GB Nvidia GPU. Fish is the candidate I would investigate for the highest level of expressive performance, but its licence and hardware requirements make it a less straightforward foundation.

I would also test **Higgs TTS 3** as a quality reference, because its reported untagged-text results are directly relevant to your central requirement. I would not build the app around it without permission under its licence.

One final distinction: ElevenLabs v4 is designed to deliver expressive speech, while its multilingual v2 model is specifically positioned for stable long-form generation. So you are effectively trying to combine the strengths of expressive v4 with the long-form reliability of a dedicated narration engine. <Cite refs={["turn716015search4","turn716015search2"]}/>

**The key is not simply finding the model with the most emotion tags. It is finding one that can read ordinary prose with convincing emotional interpretation, while preserving the identity of your cloned voice for an entire book.** That is what I would make the deciding benchmark.

---

## 8. USER

What do you think about this answer from another model?

I'll check what's current as of this month for open models that do context-driven expressive cloning at high quality.

**Fish Audio Drama 3** is the one to run. It came out on 13 October 2026: a 35B mixture of experts with about 3B parameters active per token, and it takes acting direction in plain language. It leads every downloadable model on both blind arenas. On native voices it scores 1278 Elo, against 1334 for Eleven v4. On the cloned voice arena, which tests exactly what you're asking for, it scores 1085, against 1156 for Eleven v4. So it sits roughly 55 to 70 Elo behind v4 and ahead of every other model with public weights. The licence is fine for a personal reader: research and noncommercial use are free, commercial use is free under US$500K revenue, and you only need a paid licence to offer it to others as a TTS or cloning service.

This replaces my earlier shortlist. On the same arena, Chatterbox scores 1024 and Qwen3 TTS scores 929.

**Expression driven by the text.** Drama 3 performs from direction you give it: a bracketed cue at the start of a line sets the read, a new cue partway through changes it from that point, and paired tags apply direction to just the words inside them. Eleven v4 does this inference step for you internally (it tracks the context of the text and adjusts expression as it reads). You can replicate it with a director pass in front of the model:

1. Split the book into paragraphs. Give a small instruct LLM the previous and current paragraph, and have it return the current paragraph with `[direction]` and `<emphasis>` tags inserted.
2. Strip the tags and diff the result against the source. Reject any output where a word changed.
3. Send the tagged text to Drama 3 with your saved voice and fixed sampling settings and seed, so the voice stays consistent across chunks.

**Cloning.** It needs about 30 to 60 seconds of speech plus an exact transcript. Get the transcript by running ASR on your recording. The clone inherits delivery from the reference, so record yourself reading something with emotional range. Encode the reference once and store the codes. Use the original FP8 experts, because the W4A8 community quant goes silent more often with real voice clips.

**Preset voices.** The model makes up new voices when you give it no reference; the ComfyUI pack demonstrates one made from a single line and then reused for new text. Generate a batch, keep the voices you like as saved references, and that becomes your preset library. Clones of your own recordings go in the same library.

**Hardware.**

- **Full quality:** one Ada or Hopper card with 48 GB or more, such as an L40S or H100. Fish's documentation calls for Hopper or Ada GPUs, and the original FP8 checkpoint uses about 36 GiB on the GPU.
- **Budget:** a rented 24 GB RTX 4090 with expert offloading. It peaks at 20.3 GiB of VRAM and 38.9 GiB of system RAM and runs slightly faster than playback. drama-rs does the same in Rust, on CUDA or Apple Silicon. 4090 rentals start around $0.58/hr, so an hour of finished audio costs about that.
- **Serving pattern:** render chapters ahead of the listening position and cache them as Opus files. The phone then only streams files.

**Backup pick: Ming-3.0-tts.** It's a 0.5B model with preset voices and voice cloning, MIT licensed, with a 14 tag paralinguistic system and voice design from a text description. Its cloned voice score is 1068, and its confidence interval overlaps Drama 3's. That makes it the cheap option, or a fast preview engine alongside Drama 3.

**Voxtral 3**, which also ranks high, has no published weights and is API only.

Sources:

- [Fish Audio Drama 3 launch post](https://fish.audio/blog/fish-audio-drama-3/)
- [Artificial Analysis Provider Voice leaderboard](https://artificialanalysis.ai/text-to-speech/leaderboard/provider-voice)
- [Artificial Analysis Controlled Voice leaderboard](https://artificialanalysis.ai/text-to-speech/leaderboard/controlled-voice)
- [drama3-offloading (24 GB 4090)](https://github.com/darth-feir/drama3-offloading)
- [drama-rs](https://docs.rs/crate/drama-rs/0.3.0)
- [ComfyUI-Drama3 quant notes](https://huggingface.co/PedroMarinhoDev/Drama-3-ComfyUI)
- [Ming-3.0-tts release notes](https://www.orcarouter.ai/es/blog/ming-3-0-tts-0-5b-quiet-release)

---

## 9. ASSISTANT (commentary)

The other answer is substantially better targeted than my earlier shortlist: it identifies Drama 3 as the leading open-weight candidate and correctly separates native-voice testing from cloned-voice testing. I’ve verified those two leaderboard figures. There are, however, a couple of important qualifications—especially one in the licence wording and one in how the benchmark scores should be interpreted.

---

## 10. ASSISTANT

I think this is a substantially better answer than my original shortlist. Having checked its sources against the current documentation and benchmarks, **I would change my recommendation: Fish Audio Drama 3 should be your first model to test.**

The other model has identified something particularly important: Drama 3 isn't simply a voice-cloning model. It is designed to produce directed performances, including long-form narration, dialogue and changes in emotional delivery.

However, there are a few things I would correct before you commit to it.

## 1. The benchmark evidence is genuinely impressive

I've independently checked both Artificial Analysis leaderboards. The numbers in the other answer are correct.

| Benchmark | Drama 3 | Eleven v4 |
|---|---:|---:|
| Native voices | 1278 ± 18 | 1334 ± 15 |
| Controlled cloned voices | 1085 ± 16 | 1156 ± 12 |

These are two different tests:

- **Native voices:** Each model uses its own supplied voices.
- **Controlled voices:** Every model uses the same eight cloned voices, four American and four British.

The second benchmark is particularly relevant to your requirements. Drama 3 is substantially more competitive when working with cloned voices than older open models such as Chatterbox. <Cite refs={["turn327712search0","turn516905view0","turn123932view1"]}/>

There is one qualification: these are Elo ratings from blind human preferences, not percentages of audio quality. The 71-point difference does not mean Drama 3 is a particular percentage worse than ElevenLabs.

Also, the confidence intervals for Drama 3 and Ming overlap on the cloned-voice benchmark. Drama 3 scores 1085 ± 16, while Ming scores 1068 ± 15. That means Drama's apparent advantage over Ming is not conclusive from this test alone. <Cite refs={["turn516905view0","turn287954view2"]}/>

One error in the other answer: Chatterbox's score of 1024 is its *native-voice* score. Its current controlled-voice score is 935. I also couldn't verify the quoted Qwen3-TTS score of 929 against the current controlled-voice leaderboard. <Cite refs={["turn327712search0","turn123932view1"]}/>

## 2. The LLM director idea is excellent

This is arguably the most interesting part of the proposed architecture.

There is an important distinction between:

- A model that can produce expressive speech when instructed.
- A model that independently understands the emotional meaning of ordinary text and chooses an appropriate performance.

Drama 3 provides the first capability particularly well. Its official examples demonstrate natural-language directions, changes in delivery halfway through sentences, laughter, crying, whispers, and scoped emphasis tags. <Cite refs={["turn299005view0","turn299005view1","turn859778search0"]}/>

The other model's idea of putting an LLM in front of Drama 3 addresses the second capability.

I would implement it slightly differently.

**Proposed narration pipeline:** original text → LLM director → annotation and validation → Fish Audio Drama 3 → finished narration.

I'd have the LLM return structured annotations with character offsets rather than rewrite the entire paragraph. Your backend can insert the tags deterministically. Give it enough surrounding context to understand the scene, characters, relationships and emotional progression. The goal isn't to make every sentence theatrical; sometimes the correct performance is restrained.

## 3. The hardware claims check out, with an important qualification

Fish Audio's official deployment documentation specifies 48 GB of GPU memory for its original FP8 Drama 3 configuration. The original weights are approximately 36.9 GiB. <Cite refs={["turn859778search8","turn859778search9"]}/>

The third-party [drama3-offloading](https://github.com/darth-feir/drama3-offloading) project reports running on an RTX 4090 with 20.3 GB peak GPU memory and 38.9 GB system RAM, generating 31 minutes of audio in 27 minutes. This is an experimental independent implementation, not an official Fish Audio benchmark. <Cite refs={["turn299005view3","turn712717view0"]}/>

The [drama-rs](https://github.com/apiplant/drama-rs) project implements inference in Rust, with CUDA and Apple Silicon MLX support. Its Q4 configuration is about 21 GiB, and its documentation recommends at least 32 GB unified memory for practical Apple Silicon use. <Cite refs={["turn516905view4","turn287954view1"]}/>

I would investigate both, but initially prioritise original FP8 weights with expert offloading. You want the best possible quality, so I wouldn't immediately sacrifice precision to make deployment easier. Community quantisation notes report that W4A8 can produce silent outputs more often with real cloned voices. <Cite refs={["turn485030view2","turn485030view3"]}/>

## 4. One important correction about voice consistency

The other answer suggests using a fixed seed to keep the voice consistent across chunks. Fish Audio's own documentation explicitly says that fixing the seed does not fix the timbre. <Cite refs={["turn859778search3","turn485030view4"]}/>

For consistent narration, prioritise a saved reference recording and exact transcript; reuse the same reference identity; keep speaker IDs and sampling parameters consistent; and provide appropriate preceding context. A fixed seed helps reproducibility, but is not a substitute for voice conditioning.

## 5. Licensing: be careful with the term FOSS

Drama 3 uses the **Fish Audio Community License v1.0**, not MIT, Apache 2.0 or an unrestricted open-source licence. Personal, non-commercial hobbyist use is explicitly permitted. Commercial use is also permitted under certain conditions below US$500,000 in annual revenue. However, offering a hosted TTS or voice-cloning service to third parties requires a written agreement with Fish Audio, regardless of revenue. <Cite refs={["turn299005view2","turn123932view5"]}/>

For your intended private reader, where you run the server and use it yourself, the non-commercial personal-use permission is relevant. For a publicly available reading application, obtain written permission before deployment. Being under the revenue threshold does not automatically solve the hosted-service restriction.

## 6. Ming-3.0 is a stronger backup than I initially thought

[Ming-3.0-tts](https://github.com/inclusionAI/Ming-3.0-tts) deserves consideration. It provides a 0.5B dense version and a 7.9B mixture-of-experts version with 1.3B active parameters. It supports cloning, voice design, natural-language delivery instructions and 14 paralinguistic tags. Its licence is MIT. <Cite refs={["turn903573view3","turn327712search4"]}/>

The leaderboard results are surprisingly competitive, especially given its small model family. Include the 0.5B model in testing, but don't assume the benchmark score for the Ming model family necessarily represents identical performance from both checkpoints.

## My revised verdict

| Priority | Model | Reason |
|---|---|---|
| 1 | **Fish Audio Drama 3** | Best overall open-weight candidate for expressive cloned narration |
| 2 | **Ming-3.0-tts** | MIT licence, small model, surprisingly competitive cloning |
| 3 | **MOSS-TTS v1.5** | Worth benchmarking specifically for long-form consistency |
| 4 | **Qwen3-TTS** | Additional voice-design and cloning alternative |

My preferred architecture would be Drama 3 + an LLM director + a voice-profile manager + a persistent audio cache. The key experiment is to take an entire chapter of a novel, clone your voice, run it through Drama 3 with and without the LLM director, and compare the results.

---

## 11. USER

Okay, and what about this one?

For a self-hosted replacement for ElevenLabs Reader that delivers zero-shot voice cloning, preset voices, and context-driven expressiveness directly from the text, **Qwen3-TTS (1.7B)** combined with **Kokoro-82M** is the strongest stack to build around.\
\
Because TTS models are LLM-based (predicting audio tokens from text tokens), they infer prosody, pacing, and emotion directly from sentence semantics and punctuation rather than reading word-by-word. However, no single open-weight model wins across every metric, so your backend should expose a unified `/v1/audio/speech` endpoint that routes between two models depending on the task.

### 1. Primary Engine (Cloning + Text-Driven Expressiveness): Qwen3-TTS (1.7B)

**Qwen3-TTS** (`Qwen3-TTS-12Hz-1.7B-Base` and `CustomVoice`) is the best all-around open-weight (Apache 2.0) model for expressive cloning and reading.

* **Context-Aware Prosody & Instruct Control:** Because it is built on an LLM backbone trained on 5M+ hours of speech, it naturally shifts cadence and intonation based on the text. It also supports an instruct mode where you can pass natural language delivery prompts alongside the text (e.g., `"Warm, slow narrative tone"`).
* **Voice Cloning:** Clones accurately from 3 to 10 seconds of reference audio. **Crucial implementation detail:** Always pass both the reference audio (`--ref-audio`) *and* the exact transcript of that reference audio (`--ref-text`). Cloning without the transcript causes a steep drop in similarity and prosody.
* **Presets:** Ships with 49+ built-in voice timbres.
* **Hardware & Speed:** Needs ~6–8 GB VRAM (or 10–12 GB with full KV cache). Do not use the default Hugging Face inference script in production—it is slow. Run it via `faster-qwen3-tts` or `qwen3-tts-triton` (or vLLM), which brings Time-To-First-Audio (TTFA) down to ~120ms.

### 2. High-End Expressive Alternative: Fish Speech S2-Pro (4.4B)

If Qwen3-TTS isn't expressive enough for dramatic fiction and you are willing to pay for heavier GPU compute, use **Fish Audio S2-Pro**.

* **Why it wins on pure quality:** It uses a Dual-AR architecture (4B slow AR + 400M fast AR) aligned with reinforcement learning (GRPO) for acoustic preference and emotional range. In blind tests, it is the closest model to ElevenLabs Gen 4.
* **Inline Natural Language Tags:** You can inject free-form bracketed instructions anywhere in the text stream—such as `[whisper in small voice]`, `[sigh]`, or `[sarcastic tone]`—and it adjusts the cloned voice dynamically.
* **The Catch:** It requires ~16–24 GB VRAM to run comfortably ( best run on an RTX 3090/4090 or rented L4/A10G via SGLang, which caches the cloned voice prefix KV state for ~100ms streaming latency). Its weights are also under the restrictive Fish Audio Research License, which is fine for a personal phone app but prohibits commercial resale.

### 3. Secondary Engine (Zero-Cost Daily Presets): Kokoro-82M

For reading standard articles, documentation, or PDFs when you don't need a custom cloned voice, run **Kokoro-82M** (Apache 2.0) alongside your main model.

* **Why include it:** At just 82M parameters, it runs faster than real-time on a basic CPU (or directly on-device on your phone via ONNX) and has 54 built-in preset voices (including solid US and UK English presets like `af_heart`, `af_bella`, and `bf_emma`). It cannot clone voices, so use it strictly as your fast, zero-compute preset engine.

### 4. Other Notable Contenders (And Their Trade-Offs)

* **Chatterbox / Chatterbox-Turbo (MIT):** By Resemble AI. Exceptional 1:1 voice cloning similarity and an `exaggeration` slider (0.0 to 2.0) to dial up emotional range. However, it caps out at 1,000 speech tokens (~40 seconds) per pass before silently dropping text, requiring strict 300-character sentence chunking.
* **MOSS-TTS v1.5 (OpenMOSS):** Outputs native 48 kHz stereo audio with strong punctuation-driven prosody and explicit `[pause Xs]` tags. Great for long-form audiobooks, though heavier to host than Qwen3-TTS.
* **IndexTTS-2:** The strongest model for extreme emotional range on cloned voices (uses dedicated emotion vectors or text-inferred emotion), but pushing emotion intensity too high causes the voice timbre to drift away from the clone.

| Model | License | Cloning | Presets | Context Expressiveness | VRAM Needed |
| --- | --- | --- | --- | --- | --- |
| **Qwen3-TTS (1.7B)** | Apache 2.0 | 3–10s (+transcript) | 49+ voices | High + Instruct mode | 6–8 GB |
| **Fish S2-Pro (4.4B)** | Research Only | 10–30s | Multi-speaker | Highest + inline tags | 16–24 GB |
| **Chatterbox** | MIT | 5–10s | Curated pack | Moderate (`exaggeration` param) | 4–6 GB |
| **MOSS-TTS v1.5** | Open Weights | 5–15s | Built-in pack | High (48kHz, pause tags) | 8–12 GB |
| **Kokoro-82M** | Apache 2.0 | None | 54 voices | Good natural cadence | CPU / <1 GB |

### How to Architect Your Reader App & Backend

When building your wrapper and phone client to replace ElevenLabs Reader, implement these four pipeline rules so long-form reading doesn't break:

1. **Sentence-Boundary Chunking & Lookahead Queue:** Never feed an entire chapter or article into an autoregressive TTS model at once; attention drift will degrade the voice or cause skipped lines. Have your backend split articles by sentence/paragraph (~300–600 characters per chunk). Stream chunk 1 to the phone immediately via HTTP chunked transfer or WebSockets while the server pre-generates chunks 2 and 3 in the background.
2. **Optional LLM Emotion Tagging Pass (For Fish S2-Pro / Qwen3):** If you want maximum dramatic expressiveness on stories without manual tagging, run a tiny local LLM (like Qwen3-0.6B or 1.7B) as a preprocessor on your text chunks to automatically inject tone instructions or inline tags based on the surrounding paragraph context before passing it to the TTS engine.
3. **Loudness Normalization (`loudnorm`):** Cloned voices in Qwen3-TTS and Fish Speech often wander in volume by 2–3 LUFS between separate paragraph generations. Pipe your raw PCM output through an `ffmpeg` filter (`loudnorm=I=-20:TP=-2`) before sending the opus/AAC stream to your phone so volume stays locked across an entire book.
4. **Length Sanity Check:** Autoregressive TTS models occasionally hallucinate silence or drop the final clause of a sentence. English prose averages `0.05` to `0.06` seconds per character; if a generated audio chunk's duration deviates by more than 40% from the expected character length, have your backend automatically re-roll that sentence with `seed + 1` before streaming it to your client.

If you want a ready-made backend reference before writing your own, inspect **Voicebox** (`https://github.com/jamiepine/voicebox`)—it already wraps Qwen3-TTS, Chatterbox, Kokoro, and Hume TADA behind a clean local API.

---

## 12. ASSISTANT (commentary)

The main structural flaw is already clear: the answer treats Qwen3’s cloning and expressive-instruction capabilities as though they are all available in one 1.7B checkpoint. They are split across different checkpoints. That matters because your requirement is specifically to clone *your* voice and then have it perform text-driven emotional narration—not merely choose between a cloned voice and an instructed preset voice. I’m checking the remaining technical claims before giving you a verdict.

---

## 13. ASSISTANT

This is a sensible proposal, and I actually like the architecture. However, I would change its model selection and correct several technical claims.

My assessment: **Qwen3-TTS + Kokoro is a strong budget-conscious foundation, but I wouldn't call it the strongest combination for your particular requirement: expressive narration using your own cloned voice.**

There is one fairly important distinction that the other answer overlooks.

## 1. The biggest problem: Qwen3-TTS has separate models for cloning and expressiveness

Qwen3-TTS 1.7B comes in three particularly relevant versions:

| Checkpoint | Main capability | Voice cloning | Instruction control |
|---|---|---|---|
| 1.7B Base | Clone existing voices | Yes | No documented instruct interface |
| 1.7B CustomVoice | Supplied speaker voices | No zero-shot user cloning | Yes |
| 1.7B VoiceDesign | Generate voices from descriptions | Creates new voices | Yes |

This is confirmed by the official Qwen repository. <Cite refs={["turn951091view3","turn951091view4"]}/>

This matters enormously. You upload a recording of yourself. The model learns your voice. You then give it a chapter containing excitement, sadness, sarcasm, suspense and dialogue. You want it to understand those emotions and reproduce them in your voice.

The problem is that the officially documented natural-language instruction interface belongs to CustomVoice and VoiceDesign, not the Base cloning checkpoint. The Base model can still generate speech with contextual prosody, but you shouldn't assume you can give it instructions such as “Read this paragraph angrily, then gradually become quieter and more uncertain.” That is not a documented feature of its cloning interface.

One interesting thing: Qwen officially documents a workflow for generating a voice with VoiceDesign and subsequently converting that recording into a reusable voice-cloning prompt. That's useful for designing your own library of synthetic speakers, although it isn't the same as cloning your actual voice. <Cite refs={["turn227535view4"]}/>

**Another factual correction:** Qwen3 CustomVoice has nine supplied speaker timbres, not 49+. <Cite refs={["turn951091view4"]}/>

## 2. Kokoro is an excellent addition

**Kokoro-82M** is Apache 2.0, lightweight and has 54 voices across nine languages. There are several British English voices, including Alice, Emma, Isabella, Lily, Daniel, Fable, George and Lewis. <Cite refs={["turn193347view2","turn610966search10"]}/>

It is an excellent lightweight option for reading ordinary articles, documents and other material. Its limitations are that it doesn't clone voices and doesn't offer the natural-language emotional direction you are looking for. I would use it as an inexpensive preset-voice engine, not as your main expressive narration model.

## 3. Fish Audio S2-Pro is worth investigating, but the answer understates Drama 3

Fish Audio S2-Pro is a legitimate candidate. Its documented features include voice cloning from approximately 10–30 seconds of reference audio, inline expressive directions, multiple speakers and long-form speech generation. <Cite refs={["turn193347view5","turn690419view4"]}/>

However, Fish's current installation documentation specifies at least 24 GB of GPU memory for S2-Pro, rather than 16–24 GB as the answer suggests. CPU inference is supported, although it may not be suitable for interactive reading. <Cite refs={["turn193347view6"]}/>

The current Artificial Analysis controlled-voice comparison in the answer gave Drama 3 a 1085 Elo score, Eleven v4 1156, and Fish S2-Pro 1006. Drama 3 is more competitive on that test. <Cite refs={["turn951091view0","turn951091view1","turn227535view0"]}/>

One licensing detail: S2-Pro uses the Fish Audio Research License. Personal, non-commercial use is permitted, but commercial use requires a separate written licence. It is not unrestricted FOSS. <Cite refs={["turn507630view2","turn193347view7"]}/>

## 4. Several implementation recommendations need adjustment

**Chunking:** 300–600 characters isn't a universal optimum. Use model-specific limits and sentence/paragraph boundaries. Preserve enough surrounding context for the narration to remain coherent. Kokoro, for example, has a documented input context limit of 510 tokens. <Cite refs={["turn227535view9"]}/>

**Loudness normalization:** applying `loudnorm` independently to every paragraph can flatten the emotional dynamics. Generate the chapter, join its chunks and apply a consistent final mastering stage instead. The claimed 2–3 LUFS variation should be measured rather than treated as universal.

**Detecting missing text:** 0.05–0.06 seconds per character is a rough duration estimate, not a reliable way to detect omitted words. Run ASR on generated audio and compare its transcript against the original text. Forced alignment can help locate omissions and timing errors.

**Low latency:** Qwen's developers advertise streaming generation with latency as low as 97 ms. That is a model-level capability, not a guarantee that a complete server-to-phone request will consistently start playing within 120 ms. <Cite refs={["turn193347view4","turn474404academia72"]}/>

Also, not all TTS models are LLM-based. Kokoro is a lightweight TTS architecture; its existence is a good example of why that generalisation is misleading.

## 5. What I would actually build

I would separate the models according to their strengths rather than force one to do everything:

- **Fish Audio Drama 3:** highest-priority expressive narration candidate.
- **Qwen3-TTS Base 1.7B:** alternative for cloned voices.
- **Qwen3-TTS CustomVoice:** instructable preset voices.
- **Kokoro-82M:** large preset collection and fast fallback.

Use shared infrastructure: LLM director → voice-profile manager → model router → audio generation → caching → phone streaming.

Voicebox ([github.com/jamiepine/voicebox](https://github.com/jamiepine/voicebox)) integrates Qwen3-TTS, Kokoro, Chatterbox and Hume TADA behind a unified backend abstraction. It may be useful as a code reference, although it is primarily a local desktop application rather than a ready-made phone-oriented server. <Cite refs={["turn435091search2","turn435091search1"]}/>

**My conclusion:** keep Qwen3-TTS and Kokoro in the project, but don't abandon Fish Drama 3. Your most important requirement isn't simply generating speech cheaply; it's having a cloned voice convincingly perform an entire book.

---

## 14. USER

And also what about if I just wanted it for free?
use hugging face or anything or google architectures or any free ones for backend
Just to be clear, we're not using my laptop, right? Because I can't. I don't think I can use my laptop. First of all, it's not on 24/7. Second of all, batteries. Third of all, many things. My laptop is MacBook, 32 gigabytes RAM and 8 gigabytes GPU. It's a 2019, so it has all the issues that it comes with. Anyhow, so what are you gonna say to me? Because, like, this whole question is complicated and rabbit hole based, and I just want it to just work. You're not looking for one solution. You're looking for a layered architecture that would be speed good, quality good, expressiveness good, and format good, and etc., etc.

---

## 15. ASSISTANT

I think we can simplify this considerably.

**If your priority is spending absolutely £0, not using your MacBook, and having something accessible from your phone whenever you want, I would actually change my recommendation again.**

There is a Google option that is particularly relevant: **Gemini 3.8 Flash TTS**. It now supports voice replication, custom voice design, expressive speech generation and long-form narration through a hosted API.

That means you may not need to rent a GPU, download a 35B model, or maintain an inference server at all.

## 1. The option I would try first: Google Gemini TTS

**Gemini 3.8 Flash TTS** and **Gemini 3.8 Flash-Lite TTS** were presented as free-tier hosted options, with Flash for higher-quality expressive narration and Flash-Lite for faster generation.

The answer described Google's voice replication workflow as using a 10–30-second recording plus a separate consent statement, then reusing a returned voice ID. It linked to Google's [speech generation documentation](https://ai.google.dev/gemini-api/docs/speech-generation), [voice replication documentation](https://ai.google.dev/gemini-api/docs/voice-replication) and [pricing](https://ai.google.dev/gemini-api/docs/pricing).

It also noted that free-tier quotas are limited and that Google may use free-tier submitted content to improve its products, so private documents and voice recordings require careful privacy consideration. <Cite refs={["turn150847view1","turn150847view3","turn757506view1","turn757506view0","turn579021search1","turn439829view2"]}/>

## 2. Where the backend runs

The proposed setup used Cloudflare Workers as a small API/orchestration layer and Cloudflare R2 for cached audio. Workers would hold API secrets and communicate with Google's hosted speech API; R2 would store generated audio so that repeated listening does not trigger repeated generation.

The answer listed Cloudflare Workers' free-plan request allowance and R2's free storage and operation allowances, with links to the [Workers docs](https://developers.cloudflare.com/workers/), [Workers limits](https://developers.cloudflare.com/workers/platform/limits/), [R2 docs](https://developers.cloudflare.com/r2/) and [R2 pricing](https://developers.cloudflare.com/r2/pricing/). <Cite refs={["turn579021search0","turn579021search5","turn579021search3","turn579021search4"]}/>

## 3. Proposed architecture

Phone application → Cloudflare Workers → Google Gemini TTS → Cloudflare R2 audio cache → phone playback.

The first version would generate paragraph-sized chunks, cache them and play them sequentially. The phone would not run the model.

## 4. Free GPU hosting

The answer advised against making free notebook environments the permanent backend. It described Hugging Face ZeroGPU as offering a limited daily GPU allowance, Google Colab as unsuitable for persistent media-serving or general web-service hosting on free runtimes, and Kaggle as a notebook environment rather than a persistent always-on server. <Cite refs={["turn439829view3","turn162891view9","turn439829view4","turn162891view10","turn660317search7","turn660317search10"]}/>

## 5. Open-source fallback

The suggested models to keep in reserve were Kokoro-82M for lightweight preset voices, Ming-3.0-TTS for MIT-licensed cloning and voice design, and Qwen3-TTS 1.7B as an Apache 2.0 voice-cloning alternative. The answer noted that free model weights do not make the compute required to run them free.

## Recommendation at that point

The suggested initial stack was Google Gemini TTS for hosted speech generation, Cloudflare Workers for the backend, Cloudflare R2 for storage, Gemini voice replication for cloning, Flash for higher-quality narration and Flash-Lite for faster narration. It was explicitly qualified as a free managed service subject to quotas and provider terms, not unlimited or self-hosted inference.

---

## 16. USER

we can still make it fully Rust with multiple layers and stuff like that, and even front-end and back-end, and compile it and everything, and still can be on the gradio.
Gradio spaces are free on Hugging Face if you use the zero GPU. You can run up to two. I already have one running as a recommendation engine that learns from Thingy for a Netflix app, actual Netflix app. That is exact one-to-one. Anyways, I am falling into the loop of flexing my app.

---

## 17. ASSISTANT

Yes. In fact, I think this is probably the architecture we should settle on. You can build the application predominantly in Rust, compile the components, deploy the inference service on Hugging Face Gradio Spaces, and access it from your phone.

There is just one distinction: **Gradio is the hosting and GPU-allocation interface, not a requirement to write your entire application in Python.**

## 1. Rust everywhere practical, Python only where necessary

| Component | Technology | Responsibility |
|---|---|---|
| Mobile frontend | Rust | Reading interface, playback, library, settings |
| Reader core | Rust | EPUB/PDF parsing, text extraction, chapter management |
| Text processing | Rust | Chunking, punctuation, dialogue and context preservation |
| Emotion director | Rust + LLM API | Emotional analysis and speech-direction annotations |
| API/backend | Rust | Authentication, requests, model selection, caching |
| TTS inference | Python + PyTorch | Loading and executing the speech model |
| GPU hosting | Hugging Face ZeroGPU | Dynamic GPU allocation |
| Audio storage | Object storage | Generated audio and chapter caching |

The Python component would be deliberately small. ZeroGPU's documented allocation mechanism uses the Python Gradio SDK and `@spaces.GPU`; the supported approach is to retain the model's supported Python/PyTorch inference runtime and call it from a Rust application layer. <Cite refs={["turn174724view0","turn665895view1"]}/>

## 2. Hugging Face can host the inference layer

The answer stated that eligible free personal accounts can host up to two ZeroGPU Spaces, subject to account requirements including verified email and account age. It also described the shared GPU allocations as 48 GB by default and 96 GB for larger allocations. <Cite refs={["turn174724view2","turn174724view3","turn665895view2"]}/>

It also highlighted the limitation: free accounts receive five minutes of GPU usage daily, shared across their ZeroGPU Spaces, rather than five minutes per Space. Thus the existing recommendation engine and a TTS Space would share the same allowance. <Cite refs={["turn174724view1","turn665895view2"]}/>

This makes ZeroGPU useful for development, testing and occasional generation, but not a dependable unlimited audiobook service. Drama 3 could be tested there, subject to allocation, memory overhead and runtime compatibility.

## 3. Proposed architecture

Rust mobile application → Rust application backend → Hugging Face Gradio Space with a thin Python adapter → Drama 3 / Qwen3-TTS inference → audio cache/storage → phone playback.

The Gradio Space can expose API endpoints that a Rust backend can call using HTTP. Hugging Face Spaces expose API documentation/OpenAPI information for the Gradio app. <Cite refs={["turn665895view3","turn665895view4"]}/>

## 4. Can the entire thing be Rust and run on Hugging Face?

A Docker Space can run a compiled Rust HTTP server, but Docker Spaces do not support ZeroGPU. For free ZeroGPU, the recommendation was to keep Gradio/Python as a thin inference adapter and put the rest of the application in Rust. <Cite refs={["turn665895view5","turn174724view0"]}/>

## Final recommendation at that point

- Rust for the application and business logic.
- A thin Python/Gradio integration for ZeroGPU.
- Fish Audio Drama 3 as the first high-quality model to test.
- Qwen3-TTS and other models as interchangeable alternatives.
- An emotion-director layer for contextual narration.
- Persistent audio caching.
- A phone application that communicates only with your API.

The main limitation remains ZeroGPU's five-minute daily free quota, shared with your existing Space.

---

## 18. USER

How do I get this chats on PDF? Or actually something AI-friendly. the whole chat , so json or md

---
