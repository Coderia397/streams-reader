# Progress log

Progress log, newest first. Each entry: done, blocked, next.

## 2026-10-30

### Late evening, brand mark v2 (D18)
- Done: the pixel book mark is in `brand/src/mark.txt`.
  - Wrote `brand/brandkit/pixel.py` and rewrote `icons.py` so every export is drawn per size with whole-pixel cells.
  - Regenerated all exports with `uv run --script brand/build.py` (about 26 s): web, Android, macOS .icns, Windows .ico, Linux, Tauri, OG, the new `dist/email/` and the contact sheet.
  - `brand/tests` has 25 tests, all passing (verified 2026-10-30). The tests include an exact 16/32/48 px grid match on Void and Paper, a check that ember appears only in the -ember variant, and Android layer checks.
  - v1 is archived in `brand/archive/v1/`.
- Open: `crates/web/tools/make_icons.py` still draws v1 icons. To unblock, copy `brand/dist/web/*` into the web app. There is no `crates/mail` yet; its logos are in `brand/dist/email/`.
- Open: the owner has not yet seen the v2 contact sheet (`brand/dist/contact-sheet.png`). The spots wait for the voxel mascot renders.

### Evening, design research
- Done: studied ElevenReader Android v1.7.30 over ADB (35 screenshots plus UI dumps, read-only) and the public elevenreader.io pages; wrote [12-design-benchmark.md](12-design-benchmark.md) with the inventory, feature parity gaps and the design brief. Screenshots are in `raw/design-private/` (gitignored).
- Side effect: an accidental playback of about 45 s moved one title's position from 7:03 to 7:48 in the owner's ElevenReader account; nothing else changed.
- Blocked: the logged-in ElevenReader web reader. The Claude in Chrome extension was not connected (3 attempts). Unblock: connect the extension.
- Next: owner picks a logo concept; web agent applies the token fixes and component specs from note 12.

### Evening, build phase (in progress)
- Done: read all chat exports (Claude, ChatGPT; Gemini archived); research workflow (6 agents) verified models, ZeroGPU, Cloudflare; architecture v1 → five-lens review (4.5/10) → v2 (projected 7/10); R2 bucket `nolabs-media` + bucket-scoped S3 token; Space `ibrahimar397/nolabs-engine` created and running engine v1; measured Kokoro 2.5x realtime on CPU and Ming 25.8 GPU-s for 27.7 s audio (1 GPU run used ~19:08 UTC); shared Rust contracts in `crates/core`; private repo `Coderia397/nolabs-reader` created, first commit pushed.
- Running: build workflow `wf_598e8a73-8f8` with 7 agents (core, space, import, player, web, backend, xtask), each owning one directory.
- Next: integrate, independent review agents, deploy with xtask (bootstrap token, setup, migrate, passphrase, build, deploy, space), smoke test, browser test of the live site, commit.
- Blocked: nothing. Waiting on the user only for the product name/subdomain and the on-phone screen-off test.
