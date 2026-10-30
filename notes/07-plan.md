# Build plan and acceptance checks

Plan. Interface first: the shared Rust contract (`crates/core` proto and function signatures) is written before anything else, then the crates are built in parallel by separate agents in separate directories, then integrated, reviewed by independent agents, deployed and tested end to end on the live URL. Each check below is marked verified / open / failed at the end of the session.

## 2026-10-30

### Order of work
1. Space measurements: Kokoro speed on the Space CPU (1,000 characters), Ming speed on the GPU (one batch of about 600 characters). At most 2 GPU runs tonight.
2. Workspace skeleton + `crates/core` contract (proto DTOs, engine contract v1, signatures).
3. Parallel build: core internals + tests; server + worker + migrations; web app; import; xtask; Space contract v1 update.
4. Integration: `cargo test`, clippy, wasm builds, pytest.
5. Independent review agents per component; fixes.
6. Deploy (xtask), smoke test, browser test of the live site in Chrome, fixes.
7. Notes, handoff, git commit and push to a private GitHub repo.

### Acceptance checks — milestone A
| # | Check | How verified | Status |
|---|---|---|---|
| A1 | Space builds and `/health` reports Kokoro ready | HTTP call with HF token | open |
| A2 | Kokoro renders 1,000 chars; RTF recorded | timed call, note 08 | open |
| A3 | Login with generated passphrase; wrong passphrase rejected; lockout after 20 | smoke test + unit tests | open |
| A4 | Paste import → document in library | browser on live URL | open |
| A5 | URL import of a real article | browser on live URL | open |
| A6 | EPUB import (public-domain book) | browser on live URL | open |
| A7 | Press play → audio within 5 s on a cached first window, within 15 s cold | browser on live URL, timed | open |
| A8 | Gapless playback across 10+ segments (MSE) | browser, listening + buffered ranges | open |
| A9 | Speed 0.5–3x, ±15 s, sleep timer, tap sentence to play | browser | open |
| A10 | Resume at the saved position after reload | browser | open |
| A11 | Rolling cache: replay a played segment with network off | browser devtools offline | open |
| A12 | Installable (manifest, icons, service worker) | Chrome install criteria in devtools | open |
| A13 | 20 min screen-off on the user's phone | user's phone | open (needs user) |
| A14 | All Rust tests, clippy `-D warnings`, wasm builds, pytest green | CI-equivalent run | open |

### Acceptance checks — milestone B
| # | Check | How verified | Status |
|---|---|---|---|
| B1 | Ming preset voice renders a paragraph; GPU seconds and RTF recorded | live call, note 08 | open |
| B2 | Clone flow: record (or upload) → reference prepared → preview plays | browser + live Space | open |
| B3 | Emotion segment renders through `emotion` mode | live call | open |
| B4 | GPU ledger blocks requests past the daily cap; quota shown in UI | unit test + UI | open |
| B5 | GPU job is never resubmitted after a failure (max_retries 0, marker check) | unit test | open |

## Open items
- A13 needs the user's phone.
