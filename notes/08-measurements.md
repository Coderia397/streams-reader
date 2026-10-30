# Measurements

Finding. Kokoro renders 2.5x faster than realtime on the Space CPU; Ming renders about 1.07 s of audio per GPU-second, so the daily ZeroGPU budget buys about 4.5 minutes of expressive audio. Kokoro is the everyday narrator; Ming is for short expressive or cloned passages until a bulk GPU path exists.

## 2026-10-30

All calls: plain Gradio HTTP API (`POST /gradio_api/call/<api>` then SSE), `Authorization: Bearer <HF write token>`, from the Mac over the user's hotspot, Space `ibrahimar397/nolabs-engine` commit 7cf28f5 (engine v1, Ogg Opus 48 kbps, before contract v1). Script: `scratchpad/measure/call.py`.

| What | Input | Result | Status |
|---|---|---|---|
| Space build + model load | preload_from_hub Kokoro + Ming; ZeroGPU tensor packing 2.96 GB | about 70 s packing; RUNNING at 19:01 UTC | verified |
| `health` | — | 1.4 s wall; kokoro ready, ming ready; Python 3.10.13; `os.cpu_count()` = 192 (host cores, not the Space's allowance) | verified |
| Kokoro `render_cpu`, af_heart | 708 chars, 3 segments | 42.8 s audio in 17.2 s server time (17.9 s wall) = **2.5x realtime** including ffmpeg encode and R2 upload; 49 word timings in segment 0; 250,664 bytes Ogg Opus 48 kbps | verified |
| Ming `render_gpu`, US_Emma (IP task) | 499 chars, 3 paragraphs, 1 GPU call | 27.7 s audio, **25.8 GPU-seconds**, 29.0 s server, 29.6 s wall; 0 errors | verified |

### Derived (assumed until re-measured)
- Ming GPU seconds per input char: 25.8 / 499 = 0.052. Budget policy uses `base_s = 8`, `s_per_char = 0.06` (15 % margin).
- Daily expressive audio at the 280 s cap: about 280 × 1.07 ≈ 300 s ≈ 5 min, minus per-call overhead → about 4.5 min.
- Largest single call that passes admission on a fresh window: 300 / 1.5 = 200 s requested → about 3,000 chars at 0.06 s/char, capped at 180 s.
- Kokoro at 2.5x realtime keeps up with playback up to about 2x speed with prefetch; faster playback relies on pre-render at import.
- `torch.set_num_threads(os.cpu_count())` asks for 192 threads on a 2-vCPU allowance (assumed oversubscription). Fixed in contract v1 to use the cgroup CPU quota; re-measure after.

### GPU runs used today
1 run (the Ming measurement above) at about 19:08 UTC. The window resets about 19:08 UTC on 2026-10-31.

## Open items
- Re-measure Kokoro after the thread fix.
- Quota attribution: confirm the authenticated call drew on the account's 5 minutes (check `hf spaces zero-gpu quota` or the error text on the next limit hit).
- Runs limit: probe with a 4th call on 2026-10-31.
