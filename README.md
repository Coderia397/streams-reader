<div align="center">

# Streams Reader

**Your library, read aloud.** Documents, articles and books in natural voices — including your own.
Free, open source, and part of the [Streams](https://streams.watch) family.

[listen.streams.watch](https://listen.streams.watch) · [Architecture](notes/06-architecture.md) · [Decisions](notes/05-decisions.md)

</div>

---

## What it is

Streams Reader is a free replacement for paid "read it to me" apps. Paste text, share a link, or open an EPUB, PDF,
Markdown or text file, and it reads aloud with the sentence and word highlighted as they are spoken. It keeps your
place, plays with the screen off, works offline for anything already played, and lets you record your own voice to
listen in it.

| | |
|---|---|
| **Standard voices** | [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) (Apache-2.0) — 28 English voices, unlimited |
| **Expressive voices** | [Ming-3.0-tts-0.5B](https://huggingface.co/inclusionAI/Ming-3.0-tts-0.5B) (MIT) — emotion, built-in narrators, your cloned voice |
| **Platforms** | Web today; Android, macOS, Windows and Linux apps (Tauri 2) in progress |
| **Languages** | Interface in English, Arabic, Spanish, French, German, Portuguese, Hindi and Chinese |

## How it is built

```
 App (Rust + Leptos, WebAssembly)  ──►  Cloudflare Worker (Rust)  ──►  Speech engine (Hugging Face ZeroGPU Space)
   reader, player, importers            API, render queue, cache        Kokoro on CPU · Ming on GPU
            │                                 │  D1 · R2 · Queues                │
            └──────── Supabase (accounts, library, progress sync) ◄──────────────┘  audio straight to R2
```

Everything runs on free tiers with no card: Cloudflare Workers, D1, R2 and Queues; a ZeroGPU Space; Supabase.

| Path | What lives there |
|---|---|
| `crates/core` | Pure shared logic: wire contracts, text normalisation and segmentation, cache keys, GPU budget, job states |
| `crates/import` | EPUB, HTML (readability), Markdown, text and PDF importers |
| `crates/player` | Gapless MediaSource player with Media Session, sleep timer and a real-browser test harness |
| `crates/web` | The app (Leptos 0.8, trunk, SCSS) |
| `crates/site` | Static, multilingual landing site generator |
| `crates/server`, `crates/worker` | API services (natively tested) and their Cloudflare Worker adapters |
| `crates/mail` | Auth emails in 8 languages, sent through Brevo |
| `space/` | The Python speech engine for the Space (contract v1) |
| `brand/` | One token file for every colour, radius and size; app icons and illustrations |
| `media/` | The 3D film studio for promo videos and illustrations |
| `xtask/` | Build and deploy without Node: Cloudflare, D1, Space, Supabase, smoke tests |
| `notes/` | Research, decisions, measurements and plans — start at [`notes/00-index.md`](notes/00-index.md) |

## Develop

Requirements: Rust (stable, `wasm32-unknown-unknown` target), [trunk](https://trunkrs.dev), `worker-build`, and
[uv](https://docs.astral.sh/uv/) for the Python parts. No Node.js.

```sh
cargo test                      # native crates
cd crates/web && trunk serve    # the app
cargo xtask --help              # build, deploy, smoke test
```

## Credits

Kokoro-82M by hexgrad (Apache-2.0) · Ming-3.0-tts by inclusionAI / Ant Group (MIT) · UI icons by
[Remix Icon](https://remixicon.com) · Fonts Inter and Literata (SIL OFL) · Public-domain books from
[Standard Ebooks](https://standardebooks.org) (CC0).

## Licence

[MIT](LICENSE)
