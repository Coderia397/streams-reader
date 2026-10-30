# Progress log

Progress log, newest first. Each entry: done, blocked, next.

## 2026-10-30

### Late evening, phase 1 accounts (branch of the accounts agent; see note 15)
- Done: Supabase migrations 0001–0004 applied to `atxbmgfuxjtcwmoaldqy` (profiles, invites, documents, progress, bookmarks, voices, budget_shares; forced RLS; invite-only sign-up hook + trigger; passwords disabled; Realtime on progress/documents); auth config applied (email OTP 6 digits/10 min, redirects, Send Email hook, manual linking); owner account seeded as admin; live check 22/22; advisors clean except HIBP (Pro only). Worker/server: Supabase JWT auth (ES256 JWKS cache + optional HS256), audio URL tokens, ownership via PostgREST with D1 cache, per-user GPU share, `/hooks/auth-email` with `crates/mail` (8 languages, Brevo). xtask: `supabase-migrate`, `supabase-auth`, `supabase-owner`, `supabase-check`, `invite`, `auth-provider`; deploy binds the new vars/secrets; smoke/status run as a throwaway user. All native tests and clippy (native + wasm) pass.
- Blocked: sign-in emails until the Worker is deployed at `listen.streams.watch`; OAuth until provider credentials exist.
- Next: lead merges, runs D1 `migrate`, `build`, `deploy`, attaches the domain, `smoke`; web app sign-in (note 15, "What the web app must change").

### Evening, build phase (in progress)
- Done: read all chat exports (Claude, ChatGPT; Gemini archived); research workflow (6 agents) verified models, ZeroGPU, Cloudflare; architecture v1 → five-lens review (4.5/10) → v2 (projected 7/10); R2 bucket `nolabs-media` + bucket-scoped S3 token; Space `ibrahimar397/nolabs-engine` created and running engine v1; measured Kokoro 2.5x realtime on CPU and Ming 25.8 GPU-s for 27.7 s audio (1 GPU run used ~19:08 UTC); shared Rust contracts in `crates/core`; private repo `Coderia397/nolabs-reader` created, first commit pushed.
- Running: build workflow `wf_598e8a73-8f8` with 7 agents (core, space, import, player, web, backend, xtask), each owning one directory.
- Next: integrate, independent review agents, deploy with xtask (bootstrap token, setup, migrate, passphrase, build, deploy, space), smoke test, browser test of the live site, commit.
- Blocked: nothing. Waiting on the user only for the product name/subdomain and the on-phone screen-off test.
