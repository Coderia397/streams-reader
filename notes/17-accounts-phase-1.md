# Phase 1: accounts for several users (Supabase), built and applied

Finding, decisions and reference for phase 1 of the v3 plan (note 14, D10/D11). Conclusion: the Supabase project `atxbmgfuxjtcwmoaldqy` holds invite-only accounts and each user's rows under forced RLS (verified live, 22/22 checks); the Worker verifies Supabase access tokens (ES256 via JWKS, HS256 optional), charges GPU time per user, and sends localized auth emails through Brevo. The Worker is built but NOT deployed (the lead deploys after merging).

## 2026-10-30

### What was applied to Supabase (verified)
- Migrations `supabase/migrations/0001..0004` applied with `cargo xtask supabase-migrate` (ledger `private.nolabs_migrations`, each file + ledger row in one transaction). Tables: `profiles` (settings + `is_admin`), `invites` (optional `email` binding), `documents`, `progress`, `bookmarks`, `voices`, `budget_shares`; `private.allowed_emails`. RLS enabled and forced on every table; owner-only policies; anon has no table access.
- Functions: public RPCs are SECURITY INVOKER (`invite_status`, `reserve_invite`, `create_invite`, `gpu_share`); the privileged parts are SECURITY DEFINER in schema `private` (not exposed by PostgREST), each with `search_path = ''` and EXECUTE revoked from PUBLIC; anon/authenticated may execute only the private functions the wrappers need.
- Sign-up gate: auth hook "before user created" = `pg-functions://postgres/private/hook_before_user_created` plus trigger `on_auth_user_created` that spends the invite atomically and creates the profile and budget share. Rules: owner email (in `private.allowed_emails`), or `user_metadata.invite_code`, or an invite bound to the verified email (OAuth path, `reserve_invite`).
- Passwords disabled: trigger `private.no_passwords` blanks `auth.users.encrypted_password` on every write, so password sign-in always fails (Supabase has no switch for this).
- Auth config (`cargo xtask supabase-auth`, re-read and compared after PATCH): site URL `https://listen.streams.watch`; redirect allow list `https://listen.streams.watch/**`, `http://localhost:*/**`, `tauri://localhost`, `tauri://localhost/**`, `nolabs://auth`, `nolabs://auth/**`; email OTP 6 digits, 600 s; `rate_limit_email_sent` 30/h; Send Email hook → `https://listen.streams.watch/hooks/auth-email` (secret in `~/.config/nolabs/supabase.env` as `SEND_EMAIL_HOOK_SECRET`); `security_manual_linking_enabled` true. OAuth providers Google/Azure/Apple left disabled (no credentials yet).
- Realtime publication `supabase_realtime` includes `public.progress` and `public.documents`.
- Owner account created via the Admin API (`cargo xtask supabase-owner --email <owner>`), `is_admin = true`, GPU share NULL (= the rest of the cap), email in `private.allowed_emails`. User id `088b18e9-086a-4c8e-94b3-97c8fce220e2`.
- JWT signing: project signs sessions with ES256 (kid `00e9aeae-028f-44eb-a7b4-155e964e569c`, public JWKS at `/auth/v1/.well-known/jwks.json`); the legacy HS256 key is "previously_used" (verified with GET `/v1/projects/{ref}/config/auth/signing-keys`).

### Measured / verified results
- Live behaviour check `python3 -I supabase/tests/live_rls_check.py` (2026-10-30, after 0004): 22/22 PASS — invite code, email-bound invite, refusal without invite, spent invite refused, no stored password, password sign-in fails, locale copied to profile, magic-link session without password, owner insert, cross-user select/update/insert refused, progress cannot reference another user's document, older progress never replaces newer, users cannot set `is_admin`, default GPU share 60 s, non-admins cannot create or list invites, `invite_status` for anon, `reserve_invite` binds once, anon reads nothing, internal functions 404 over PostgREST. Throwaway users deleted (0 left).
- Sign-up without/with a bad invite over the public API: 403 with the hook's message (verified with `/auth/v1/signup`).
- `cargo xtask supabase-check` (runs after every migrate): 5/5 SQL rules OK; security advisor: one WARN `auth_leaked_password_protection` accepted (HIBP needs the Pro plan, PATCH answered 402; passwords are impossible anyway); performance advisor: only INFO `unused_index` on 7 indexes (empty database). Earlier findings fixed by 0002–0004: definer RPCs callable by anon/authenticated, unindexed FKs, RLS-without-policy notes.
- `cargo test` (native workspace): core 133, import 85, mail 25, server 118, xtask 89, others 9 — all pass. `cargo clippy --all-targets -D warnings` clean; `cargo clippy -p nolabs-worker --target wasm32-unknown-unknown -D warnings` clean.
- Worker release wasm (`cargo build --release -p nolabs-worker --target wasm32-unknown-unknown`): 1,937,388 bytes raw, 548,949 bytes `gzip -9` (limit 3 MB compressed on the free plan).

### Decisions (assistant, under the lead's delegation, 2026-10-30)
- D18 Worker keeps no Supabase key beyond the publishable one. Ownership is checked by querying PostgREST with the caller's token (RLS decides); positive answers cached in D1 `owner_cache` for 5 min, negative never cached, dropped on delete. Rejected: deploying the service-role key (one leak exposes every account).
- D19 Invite redemption happens atomically in the sign-up trigger, not in a client-called `redeem_invite(code)` as first specified: a client call after sign-up could be skipped, letting one code admit many users. `reserve_invite(code, email)` (anon) binds an invite to an email for OAuth sign-ups, which cannot carry metadata. Rejected: a public definer `redeem_invite`.
- D20 Password sign-in impossible (trigger blanks passwords); email OTP + magic link + OAuth only. xtask mints test sessions with Admin `generate_link` + `verify`, so no password is needed anywhere.
- D21 Per-user GPU share: a Ming job must fit both the deployment budget (280 s, 3 runs, shared) and the caller's share (`gpu_share` RPC: invited users 60 s default, admin = cap minus all non-admin shares). The share is stored on the job (`jobs.gpu_share_s`) so the consumer and cron re-check without a token. Jobs from before accounts (no user) run against the whole cap.
- D22 Audio URLs for media elements: `POST /api/audio-token` → `a1.<user>.<exp>.<hmac>` valid 6 h, bound to the user, valid for any audio key (renders are shared); `GET /api/audio/{key}?t=…`. Key `AUDIO_URL_KEY` (Worker secret, generated into `~/.config/nolabs/app.env`).
- D23 Brand in one constant: `nolabs_core::brand` (`NAME = "Streams Reader"`, `SITE_URL`, `SENDER_EMAIL`, `APP_SCHEME = "nolabs"`). Emails, sender display name, xtask auth settings and CORS read it. Exceptions that carry the name as text: refusal messages in `supabase/migrations/*.sql`.
- D24 Bearer auth replaces the same-origin check; CORS allowlist = site, `tauri://localhost`, `http(s)://tauri.localhost`, `http://localhost:<port>`, `http://127.0.0.1:<port>`; no credentials mode. CSP `connect-src` adds the Supabase URL and its `wss://` form for direct PostgREST/Realtime.

### Identity linking rules (owner request, 2026-10-30)
- One person, one account: Supabase links a new identity automatically when the provider reports a verified email equal to an existing account's email (built-in, always on). Unverified emails never link.
- Apple private relay (`@privaterelay.appleid.com`) never matches: the settings page offers `supabase.auth.linkIdentity({ provider: 'apple' })` while signed in (manual linking enabled). First sign-up with a relay address needs an invite like any new account.
- New accounts need an invite whatever the provider (hook); the owner email is always allowed.
- Rules written down and tested in `crates/core/src/auth.rs` (`arrival`, `identity_rows`, `is_private_relay`); the settings page contract is `IdentityRow` (provider, label, linked identity, can_link, can_unlink — the last identity cannot be unlinked).
- Enabling a provider: `cargo xtask auth-provider <google|azure|apple> --client-id <id> [--secret-from <env file>]` (secret read from `~/.config/nolabs/oauth.env` keys `GOOGLE_CLIENT_SECRET` / `AZURE_CLIENT_SECRET` / `APPLE_CLIENT_SECRET`, never argv). Redirect URI for every console: `https://atxbmgfuxjtcwmoaldqy.supabase.co/auth/v1/callback`. Azure uses tenant URL `https://login.microsoftonline.com/common` (personal accounts work).

### Worker API v2 (contract for the apps)
- Every `/api` call: `Authorization: Bearer <supabase access_token>`, `x-nolabs-api: 2`. 401 → refresh the session (supabase-js does it) and retry once; 409 `upgrade_required` → reload/update.
- Routes: `GET /api/session` → `{user_id, email, expires_at_ms}`; `POST /api/audio-token` → `{token, expires_at_ms}`; `PUT/GET /api/documents/{id}/body`; `DELETE /api/documents/{id}`; voices, render, render status, audio, fetch, status as before. `GET /api/status` adds `my_gpu {used_s, share_s, resets_at_ms}`.
- Gone (moved to Supabase): `POST/DELETE /api/session`, `GET /api/library`, `POST /api/documents`, `GET /api/documents/{id}`, `PUT/POST /api/documents/{id}/progress`.
- R2 layout change: bodies at `docs/<user_id>/<doc_id>/body.json`.

### What the web app must change (phase 1 web work)
1. Sign-in screen: email field (+ invite code field for new users). New user: `supabase.rpc('invite_status', {code})` → then `supabase.auth.signInWithOtp({ email, options: { shouldCreateUser: true, data: { invite_code, locale } } })`. Existing user: `signInWithOtp({ email, options: { shouldCreateUser: false } })`. Then `supabase.auth.verifyOtp({ email, token: <6 digits>, type: 'email' })`. OAuth buttons (when enabled): first `supabase.rpc('reserve_invite', {code, email})` for new users, then `signInWithOAuth({ provider, options: { redirectTo: 'https://listen.streams.watch/auth/callback' } })`.
2. Route `/auth/confirm?token_hash=…&type=…` (links in emails): `verifyOtp({ token_hash, type })`, then go to the library.
3. Token storage: supabase-js session in localStorage (web) — PKCE flow; native apps store it in the OS keychain/keystore. Use `createClient(SUPABASE_URL, SUPABASE_ANON_KEY)` with the publishable key; never any other key.
4. Every Worker call: header `Authorization: Bearer ${session.access_token}` + `x-nolabs-api: 2`; cookies and `credentials` are not used.
5. Library, progress, bookmarks, settings: PostgREST directly (`documents`, `progress` upsert with `Prefer: resolution=merge-duplicates` and client `updated_at`, `bookmarks`, `profiles`); subscribe to Realtime `postgres_changes` on `progress` and `documents` for cross-device sync. Progress on page hide: `fetch(..., { keepalive: true })` to PostgREST (sendBeacon cannot set Authorization).
6. Import: generate the 16-char id (`nolabs_core::ids`), insert the `documents` row (`ready=false`, exact `body_bytes`), then `PUT /api/documents/{id}/body`; Worker sets `ready`.
7. Audio: `POST /api/audio-token` once (and again before `expires_at_ms`), then use `/api/audio/{key}?t=<token>` for `<audio src>` / MSE fetches.
8. Settings page: linked sign-in methods from `supabase.auth.getUserIdentities()` rendered with `nolabs_core::auth::identity_rows`; link = `linkIdentity`, unlink = `unlinkIdentity`; language change also `updateUser({ data: { locale } })` so emails follow.
9. CSP is already widened by the Worker for the Supabase origin and its WebSocket.

### Deploy (lead, after merge)
`cargo xtask migrate` (D1 0002: drops `documents`, `auth_failures`; adds user columns, `owner_cache`) → `cargo xtask build` → `cargo xtask deploy` (binds `SUPABASE_URL`, `SUPABASE_ANON_KEY`, `SITE_URL` vars; secrets `HF_TOKEN`, `AUDIO_URL_KEY`, `SEND_EMAIL_HOOK_SECRET`, `BREVO_API_KEY`, optional `SUPABASE_JWT_SECRET`; deletes retired `SESSION_KEY`, `PASSPHRASE_HASH`) → attach `listen.streams.watch` → `cargo xtask smoke` (throwaway user, cleaned up).

## Open items
- Sign-in emails fail until the Worker with `/hooks/auth-email` is deployed at `listen.streams.watch` (the hook is already enabled). Unblock: lead deploys and attaches the domain.
- Brevo click tracking might rewrite links in auth emails (account setting; assumed off as in Streams). Unblock: send one real email after deploy and check the link.
- OAuth providers disabled: need client ids/secrets. Apple needs a paid Apple Developer membership; its client secret is a JWT that expires after at most 6 months (re-run `auth-provider apple`). Unblock: the user creates the apps.
- Leaked-password protection (HIBP) needs the Pro plan; mitigated by D20.
- Shared render cache and per-user budget: a segment deferred on one user's share also waits for another user who requests the identical text and voice (content-addressed key). Rare; unblock only if it shows up (per-user claim of deferred rows).
- Single-user D1 data is not migrated: old documents' rows are dropped by D1 0002 and their R2 bodies stay at `docs/<id>/body.json`. Unblock: delete those objects by hand after checking nothing is needed.
- Supabase revocation: a signed-out access token stays valid until expiry (`jwt_exp` 3600 s). Accepted (same as every Supabase API).
- Account deletion leaves R2 bodies and D1 voice rows of the deleted user. Unblock: a cleanup step in the cron or an account-deletion route.
