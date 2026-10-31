# Progress log

Progress log, newest first. Each entry: done, blocked, next.

## 2026-10-31

### Explore catalogue pipeline (see note 23-explore-catalog)
- Done:
  - `tools/catalog/` contains `build.py` (uv), `secat/`, the Rust `epub2body` around `nolabs_import::from_epub`, `shelves.toml` and the tests (17 pass, 1 strict xfail).
  - 66 Standard Ebooks titles on 11 shelves were built and uploaded to R2 `nolabs-media/catalog/v1/`: 397 objects, 38.7 MB, checked by reading them back.
  - `supabase/migrations/0005_catalog.sql` adds `documents.catalog_id`. It is written but not applied.
- Blocked: nothing.
- Next (other owners):
  - Worker: `GET /api/catalog[/…]` and `POST /api/documents/{id}/from-catalog`.
  - Web app: the Explore screen.
  - Apply 0005.
  - Fix the importer's creator-role bug (several roles per creator, so the author is lost).

### Expressiveness harness and Kokoro baseline
- Done: `tools/expressiveness/` (passages, app-segmenter bin, render.py, fetch_human.py, prosody.py, calibration.json, results/baseline.{json,md}); 4 Kokoro voices x 3 passages rendered via the live API, two flat controls, three LibriVox ceiling clips; EI defined and validated (monotone -0.86 < Kokoro -0.49..0.32 < human 0.89). See note 20.
- Blocked: nothing. No ZeroGPU spent.
- Next: verify human excerpt content; measure Ming and prompting experiments against the frozen calibration.

### 00:40-01:10, after the usage-limit reset
- Done: committed brand v2 (pixel mark, central tokens), landing site v1, film studio, native-apps scaffold; merged accounts phase 1 into main (conflicts in notes resolved; brand's D18 renumbered D26; D25 recorded after the first attempt silently failed); removed merged branches and worktrees; repo hygiene (CHANGELOG with versioning policy, CONTRIBUTING, SECURITY, issue/PR templates, Dependabot, .editorconfig, .gitattributes); GitHub: vulnerability alerts, automatic security fixes, secret scanning + push protection on, area/priority labels, tag v0.1.0 at the first live deploy (c4b5c43).
- Resumed after the limit: film studio (Ming voice-over + rig QA), native apps architect, on-device TTS research, player find-fix loop (round 2+), expressiveness study; launched app-complete workflow (finish redesign on brand v2, accounts wiring, player web fix, critic loop, Worker serving site at / and app at /app).
- Blocked: GitHub release page for v0.1.0 — both the env token and gh's stored login are fine-grained tokens without Contents: write (HTTP 403 on POST /releases). Unblock: owner adds Contents read/write to the token.
- Film v1.1 done 01:30 (Ming voice-over: 38/280 GPU-s, 1/3 runs used; rig QA 8 passes, 0 metric issues; outputs local in media/dist). Open bug reported by the film agent: once in the app capture, after a chapter heading the first paragraph was skipped in audio and highlight — reproduce in the live find-fix loop after deploy (check segmenter heading boundary + player first-segment append).
- Domain: reader.streams.watch attached 01:25 via `cargo xtask domain`; Google sign-in live (D34).
- Agent admin 01:35: paused workflow motion-and-film-v2 (run wf_80ae0c27-54b: product motion reel + Blender film v2) to free Kaggle GPU sessions for the expressive bake-off and save usage; film v1.1 already ships. Resume later with Workflow scriptPath workflows/scripts/motion-and-film-v2-wf_80ae0c27-54b.js + resumeFromRunId wf_80ae0c27-54b. Kept: app-complete, landing-v2, player loop, expressiveness study, bake-off (wf_e6f34261-3b1), parallel tracks.
- Safety net: hourly self check-in (session cron, :13 past each hour) resumes anything stopped by a usage limit.
- Next: when app-complete lands → D1 migrate 0002, deploy secrets, build, deploy, smoke, Pixel test, live find-fix loop; then landing v2, Blender film v2, native apps builds in CI.

## 2026-10-30

### Late evening, brand mark v2 (D18)
- Done: the pixel book mark is in `brand/src/mark.txt`.
  - Wrote `brand/brandkit/pixel.py` and rewrote `icons.py` so every export is drawn per size with whole-pixel cells.
  - Regenerated all exports with `uv run --script brand/build.py` (about 26 s): web, Android, macOS .icns, Windows .ico, Linux, Tauri, OG, the new `dist/email/` and the contact sheet.
  - `brand/tests` has 25 tests, all passing (verified 2026-10-30). The tests include an exact 16/32/48 px grid match on Void and Paper, a check that ember appears only in the -ember variant, and Android layer checks.
  - v1 is archived in `brand/archive/v1/`.
- Open: `crates/web/tools/make_icons.py` still draws v1 icons. To unblock, copy `brand/dist/web/*` into the web app. There is no `crates/mail` yet; its logos are in `brand/dist/email/`.
- Open: the owner has not yet seen the v2 contact sheet (`brand/dist/contact-sheet.png`). The spots wait for the voxel mascot renders.

### Late evening, phase 1 accounts (branch of the accounts agent; see note 17)
- Done: Supabase migrations 0001–0004 applied to `atxbmgfuxjtcwmoaldqy` (profiles, invites, documents, progress, bookmarks, voices, budget_shares; forced RLS; invite-only sign-up hook + trigger; passwords disabled; Realtime on progress/documents); auth config applied (email OTP 6 digits/10 min, redirects, Send Email hook, manual linking); owner account seeded as admin; live check 22/22; advisors clean except HIBP (Pro only). Worker/server: Supabase JWT auth (ES256 JWKS cache + optional HS256), audio URL tokens, ownership via PostgREST with D1 cache, per-user GPU share, `/hooks/auth-email` with `crates/mail` (8 languages, Brevo). xtask: `supabase-migrate`, `supabase-auth`, `supabase-owner`, `supabase-check`, `invite`, `auth-provider`; deploy binds the new vars/secrets; smoke/status run as a throwaway user. All native tests and clippy (native + wasm) pass.
- Blocked: sign-in emails until the Worker is deployed at `listen.streams.watch`; OAuth until provider credentials exist.
- Next: lead merges, runs D1 `migrate`, `build`, `deploy`, attaches the domain, `smoke`; web app sign-in (note 17, "What the web app must change").
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
