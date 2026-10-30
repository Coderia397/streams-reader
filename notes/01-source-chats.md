# Source chats: what each file is

Finding. Three chats were provided as six files (md + json each). The JSON files mirror their markdown exactly; the Gemini export holds no TTS content and was archived.

## 2026-10-30

| Chat | Files (after reorganisation) | Messages | Content |
|---|---|---|---|
| Claude | `sources/claude/chat.md`, `chat.json` | 17 | Full TTS reader research, ends with the ZeroGPU + Rust layering plan |
| ChatGPT | `sources/chatgpt/chat.md`, `chat.json` | 18 | Same research from another model; reviews the Claude answer and "Answer A" |
| Gemini | `archive/gemini-export/chat.md`, `chat.json` | 6 | Only the attempts to export the chat (console scripts, ~206 KB of browser console log). No TTS content. |

Original file names: `chat-export (1).md` / `chat-export (4).json` (Claude), `tts_reader_chat_transcript.*` (ChatGPT), `chat-export.md` / `chat-export (3).json` (Gemini).

How the mirror claim was checked (verified): a script split each JSON message into paragraphs, normalised to lowercase alphanumerics, and searched the markdown for each paragraph longer than 40 characters. Result: 0 missing paragraphs across 17 (Claude) and 18 (ChatGPT) messages.

Gemini check (verified): `grep -i` for kokoro, drama, zerogpu, gradio, chatterbox, qwen3, cloudflare, fish returned 0 hits in the Gemini markdown. The browser tab title in the pasted console log was "Self-Hosted Open-Source TTS Reader Stack - Google Gemini", so the chat existed but the scraper only captured the last 6 visible turns.

Assumed: "Answer A" pasted into the Claude chat (Qwen3-TTS + Kokoro, Fish S2-Pro, Voicebox) is the Gemini answer. The ChatGPT chat reviews the same text in its message 11.

Decision (user, 2026-10-30): Gemini files can be removed. They were moved to `archive/` rather than deleted.

## Open items
- None. If the Gemini chat content is wanted, it has to be re-exported from gemini.google.com with the whole conversation scrolled into view first.
