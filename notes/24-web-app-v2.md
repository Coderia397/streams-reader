# Web app v2: brand v2, accounts, player fix (crates/web)

Finding + reference. Conclusion: `crates/web` is finished on brand v2 and the accounts contract of note 17; it builds (trunk debug, post-build token check), passes 128 native tests and wasm clippy with `-D warnings`, and every screen renders against a local mock in Ink and Paper, English and Arabic, at 390x844 and 1440x900. Not yet deployed or tested against the live Worker and Supabase.

## 2026-10-31

### What was built (verified by build, tests and screenshots)
- Design on brand v2: every stylesheet uses `brand/dist/tokens/_tokens.scss` (`@use … as t`); `brand/check_tokens.py crates/web/style` reports 0 findings. The check runs in `tools/postbuild.py` (through `tools/check_style.py`, which also rejects raw font sizes and hex colours in Rust); the separate pre-build hook was removed. Verified: a probe file with `#ff0000` and `border-radius: 7px` makes the check exit 1.
- Icons: `tools/make_icons.py` and its v1 PNGs deleted. `index.html` copies `brand/dist/web/*` (favicons, apple-touch, icon-192/512, maskable, safari-pinned-tab) to `/app/assets/brand/icons/` and `brand/spot/` to `/app/assets/brand/spot/` at build time; the manifest points there (theme `#0B0B0C` filled from tokens). Spot paths are built only in `config::spot` (swap point for the voxel mascot renders, D17).
- Logo: the pixel mark is read at compile time from `brand/mark/streams-reader-mark-currentcolor.svg` (`components/logo.rs`), drawn at 32/48 px (whole cells), white on the blue tile for sign-in and the splash, blue at the top of the desktop rail.
- Ember is now used only for the spoken word and recording; the Expressive badge, sleep badge, favourite heart and budget bars moved to brand roles.
- App under `/app`: trunk `public_url = "/app/"`, router base `/app`, service worker `/app/sw.js` scope `/app/`, manifest `start_url`/`scope` `/app/`, share target `/app/`.
- Accounts (note 17), no JS SDK:
  - `auth/`: email code. An existing account signs in with `create_user: false`. GoTrue's `otp_disabled` answer shows the invite field. A new account is checked with `rpc/invite_status`, then sent with `create_user: true` and `data {invite_code, locale}`. `/app/auth/confirm?token_hash&type` verifies the email link. OAuth buttons appear only for providers on in `GET /auth/v1/settings`; they use PKCE to `/app/auth/callback`, and a new account calls `rpc/reserve_invite` first. The access token lives in memory and the refresh token in IndexedDB. The session is refreshed 60 s before expiry and on returning to the page, and a 401 triggers one refresh and one retry.
  - `api/`: every Worker call sends `Authorization: Bearer` and `x-nolabs-api: 2`, with credentials omitted. Audio uses `POST /api/audio-token`, then `/api/audio/{key}?t=`. The token is minted again 5 min before expiry or once after a 401/403. The Cache API key is the path without the token.
  - `db/` (PostgREST, RLS): library list (documents + progress), insert document (client id from `nolabs_core::ids`, `ready=false`, exact `body_bytes`) followed by PUT body to the Worker, progress upsert (`Prefer: resolution=merge-duplicates`, client `updated_at`, `fetch keepalive` on page hide), profile get/patch, bookmarks list/add/remove (reader More sheet), and the invite RPCs.
  - `model.rs` + `util/time.rs`: rows → `DocumentSummary`/`Progress`, RFC 3339 parse/format (pure, tested).
  - `state/profile.rs`: after sign-in the profile's default voice, speed and theme are applied, and later changes are written back (speed capped at the row's 3.0; Ink/Night both map to `dark`). The device's language is pushed to the profile and `user_metadata.locale`, also when it changes in Settings.
  - Settings → Account: email, sign-in methods from `nolabs_core::auth::identity_rows` (link/unlink), sign out, sign out everywhere. Delete account is hidden because the Worker has no route for it. No passphrase code or UI remains (grep verified).
- Player fix (task from the player testers): `PlayerBridge::jump_to` no longer sets `segment`. `enter_segment` reloads word timings when `words_for != segment` or the list is empty, and retries on the next snapshot when `session.words` comes back empty (`state/player/events.rs`, test `words_reload_for_a_new_segment_or_an_empty_list`).
- Other fixes found on the rendered page:
  - Three timers in the reader read disposed signals after leaving it, which caused a wasm panic in the console (fold timer, play-here timer, follow/observer timers). They now use `try_` accessors.
  - Delete in the library now has Undo: the item is hidden at once and deleted after the 6 s toast unless undone.
  - The GPU budget shown everywhere is now the user's own share, capped by the deployment's remaining time (`GpuBudget::of`, D21).
  - Generated covers use a single deep hue with an accent glow and a serif monogram. The old mixed-hue gradients looked muddy.
  - RTL titles in LTR lists are clamped in an outer span with `<bdi>` inside. Before, the clamp aligned them to the right.
  - The "new here" hint is a calm note, not an error.
  - The legal links are same-origin (`/privacy`, `/terms`), because the canonical host changed (D34).
  - "Streams Reader" in translated text uses a no-break space so titles do not split the name across lines. `app-name` stays plain.

### Verification (2026-10-31)
- `cargo test -p nolabs-web`: 128 passed.
- `cargo clippy -p nolabs-web --target wasm32-unknown-unknown -- -D warnings` and `cargo clippy -p nolabs-web --tests -- -D warnings`: clean.
- `trunk build` (debug) in `crates/web`: success. The post-build output was "tokens clean, loader externalised, 46 files precached". The debug `dist/` is 87 MB, mostly the unoptimised wasm.
- Real-page check: the scratch mock `app-complete/mock.py` serves dist under `/app` with the Worker's CSP. It mocks `/api` with the player harness's real Kokoro clips and word timings, and Supabase Auth/PostgREST under `/supa`. The app is built with `STREAMS_READER_AUTH_URL=http://127.0.0.1:8099/supa`. Screens were driven over CDP (`shoot.py`, headless Chrome Beta 158).
  - 23 screens per combination × 8 combinations = 184 screenshots; 0 failed scenes and 0 console errors on the final runs. The first run had a wasm panic from a disposed signal, which is now fixed.
  - Screens: sign-in, invite, code, welcome, library, add sheet, search empty, undo toast, reader paused, reader playing, folded dock, play from here, voice/speed/sleep/contents/more sheets, mini-player, voices, record, settings (top and end), not found.
  - Scripts and screenshots are in the session scratchpad and are not kept in the repo (assumed throwaway). Re-create the mock from this description if needed.

### Open
- The auth email links point at `<site>/auth/confirm` (crates/mail `hook.rs`), but the app route is `/app/auth/confirm`. Unblock: the Worker redirects `/auth/confirm` to `/app/auth/confirm` keeping the query, or mail builds `/app/auth/confirm`.
- Realtime cross-device sync (`postgres_changes` on progress/documents) is not wired. The library and positions refresh on open. Unblock: a small Realtime WebSocket client in `auth::on_signed_in`.
- Delete account: hidden until the Worker has a route that removes the user and their R2 objects.
- Not tested live: GoTrue error shapes for `otp_disabled` and the invite hook message are assumed from note 17 and Supabase docs. Unblock: a live sign-in with a new email after deploy.
- Undo deletes are lost if the tab closes within 6 s (the document stays). This is accepted.
- The Worker must serve `crates/web/dist` under `/app/` (owned by the deploy/Worker agent).
