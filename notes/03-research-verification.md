# Research verification (facts current on 2026-10-30)

Finding. The free ZeroGPU quota (5 GPU-minutes a day per account, plus a possible limit of 3 runs a day) rules out GPU narration of whole books. Kokoro on the Space CPU uses no quota, so it becomes the default narrator; Ming-3.0-tts-0.5B (MIT) on the GPU is the expressive and cloning tier. Raw report: [raw/2026-10-30-research.md](raw/2026-10-30-research.md) (5 researchers + 1 critic, 242 tool calls, run `wf_10897a7b-128`).

## 2026-10-30

### Hugging Face ZeroGPU
- Quota: free account 5 min/day, anonymous 2 min, PRO 40 min. Charged per calling account, not per Space; resets 24 h after first use. Verified (HF docs `spaces-zerogpu`).
- `streams-engine` and the new Space share the same 5 minutes. The engine's `@spaces.GPU` is a placeholder that is never called, so in practice the full 5 minutes are available. Verified by reading `deploy/space/app.py` in `Coderia397/streams-engine`.
- Run-count limit: `spaces` 0.51.3 has a separate "exceeded your ZeroGPU runs limit" error branch. HF staff said 3 runs/day for free users in April 2026. Assumed still active; open until measured.
- `@spaces.GPU` default duration 60 s; on the `large` size the requested duration is multiplied by 1.5 at admission. Verified (spaces 0.51.3 source). Largest single request that fits a fresh quota: about 200 s. Free maximum duration: undocumented, open.
- Hardware: half an RTX PRO 6000 Blackwell, 48 GB (`large`, default). Flavor string is still `zero-a10g`. Verified.
- Supported: Python 3.10.13 / 3.12.12, torch 2.8.0–2.13.0, Gradio SDK only. `torch.compile` unsupported. Models go to `cuda` at module level. Verified.
- CPU-only Gradio endpoints in a ZeroGPU Space use neither GPU seconds nor runs (scheduling only happens inside the `@spaces.GPU` wrappers). Verified in source.
- Default Space environment: 2 vCPU, 16 GB RAM, 50 GB ephemeral disk; sleeps after 48 h without use. That ZeroGPU Spaces get the same CPU allowance is assumed.
- API: `POST /gradio_api/call/<name>` → `event_id`; `GET /gradio_api/call/<name>/<event_id>` → SSE (`generating`, `complete`, `error`, `heartbeat` every 15 s). An `Authorization: Bearer` token charges the caller's account quota (HF docs). Verified locally on Gradio 6.31.0 by the researcher.
- Since July 2026 Gradio Spaces need a paid plan except up to 2 ZeroGPU Spaces for free accounts. Verified.

### Python dependency constraint (verified with `uv pip compile`, Python 3.10, x86_64 manylinux)
- Gradio ≥ 6.18 needs huggingface-hub ≥ 1.2; transformers 4.46.3 (Ming, Chatterbox) needs < 1.0. Newest usable Gradio: 6.17.3.
- This set resolves: gradio 6.17.3, spaces 0.51.3, torch 2.11.0, kokoro 0.9.4 (misaki 0.9.4, spacy 3.8.18), transformers 4.46.3, numpy 1.26.4, pynini 2.1.6.post1, the Ming pins from the community Space, boto3. Lock file: `scratchpad/ming/req.lock.txt` (copy into the Space).

### TTS models
| Model | Licence (code / weights) | Use here | Evidence |
|---|---|---|---|
| Kokoro-82M (kokoro 0.9.4) | Apache-2.0 / Apache-2.0 | Default narrator on CPU; 54 voices, 28 English | Provider-voice Elo 1062. Needs `espeak-ng`. Word timestamps only when KPipeline owns the model. |
| Ming-3.0-tts-0.5B (inclusionAI) | MIT / MIT | Expressive tier on GPU: clone, built-in voices with emotion variants, voice design, free-form instruction, 7 emotions, 14 paralinguistic tags | Cloned-voice Elo 1068 (#2 open), provider-voice 1187. Community ZeroGPU Space `mrfakename/ming-3.0-tts-0.5b` runs it with torch from the image and vendored code. |
| Fish Audio Drama 3 | not OSI (Community License) | Not used | Best open-weights scores (1085 / 1278) but 41.5 GB and xlarge = 2x quota |
| Chatterbox Turbo | MIT / MIT | Not used | Cloned Elo of Chatterbox 935; PyPI pins torch 2.6 |
| Qwen3-TTS | Apache-2.0 | Not used | About 0.82x realtime baseline; Qwen3 TTS row on the arena is probably the API model |
| MOSS-TTS-Nano | Apache-2.0 | Not used tonight | No emotion control; needs transformers 4.57+ |
| OmniVoice, Higgs TTS 3, IndexTTS-2 | NC or custom | Not used | Not OSI |

Corrections to the chats: `sekkit/Ming-3.0-tts-0.5B` (cited by Claude) is an unofficial copy; the official repo is `inclusionAI/Ming-3.0-tts-0.5B`. Qwen3 CustomVoice has 9 speakers (ChatGPT was right, Answer A's "49+" was wrong). Chatterbox 1024 is its native-voice score; 935 is cloned (ChatGPT was right).

### Cloudflare (free plan)
- workers-rs `worker` 0.8.7 supports D1, R2, KV, Durable Objects (SQLite), Workers AI, Queues, cron, streaming. A test Worker built with `worker-build` 0.8.7 and no Node. Verified by the researcher.
- Limits: 100k requests/day, 10 ms CPU per request (network wait excluded), 50 subrequests, 64 MiB Worker size, 5 cron triggers per account (1 used by `consumet-keepalive`, checked 2026-10-30). No wall-time limit while the client is connected; `waitUntil` 30 s; queue consumers 15 min.
- D1: 5 GB per account, 500 MB per database, 2 MB per row. R2: 10 GB-month, 1M class A, 10M class B ops, free egress. Queues: 10k ops/day free. Durable Objects: free, SQLite only.
- Workers AI: 10,000 neurons/day. Director: `@cf/google/gemma-4-26b-a4b-it` (Apache-2.0, JSON schema output), about 50 neurons per 1k-in/1.5k-out call. Fallback `@cf/qwen/qwen3-30b-a3b-fp8`. ASR: `@cf/openai/whisper-large-v3-turbo`, 46.63 neurons per audio minute. Always-on fallback TTS: `@cf/myshell-ai/melotts` (MIT), 18.63 neurons per audio minute.
- Deploy without Node: `PUT /accounts/{id}/workers/scripts/{name}` multipart with `index.js` and `index_bg.wasm`; Global API Key headers accepted. Static assets need the assets upload session.

### Frontend
- Leptos 0.8.22 CSR + trunk built a router + Media Session + service worker skeleton: wasm 210,638 B (84,610 B gzip). Media Session types need `--cfg=web_sys_unstable_apis`. Verified by the researcher.
- Android Chrome shows the media notification only for media over 5 s from an audio element. Ogg Opus plays on Android Chrome and iOS 18.4+.
- EPUB: zip + quick-xml (the `epub` crate is GPL-3.0). Articles: `dom_smoothie` (MIT). PDF: pdf.js (Apache-2.0).
- Screen-off playback on the user's Samsung with One UI battery optimisation: open (test on device).

## Open items
- 3 runs/day limit and maximum `@spaces.GPU` duration: measure on the new Space with 4 calls.
- Kokoro speed on the Space CPU and Ming speed on the GPU: measure (decides chunk sizes and daily expressive minutes).
- Screen-off playback on the phone: needs the user's phone.
