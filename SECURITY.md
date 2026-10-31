# Security policy

## Reporting a vulnerability
Please do not open a public issue. Use GitHub's private
[security advisory form](https://github.com/Coderia397/streams-reader/security/advisories/new), or email
help@streams.watch. You will get a reply within a few days.

## Scope
- The web app and landing site at `listen.streams.watch`.
- The Cloudflare Worker API, the speech engine Space and the Supabase schema in this repository.

## What we already do
- Accounts on Supabase with row-level security forced on every table; no passwords stored.
- JWTs verified in the Worker against the project's JWKS; strict Content-Security-Policy; no CORS on the API.
- Secrets live only in Cloudflare and Hugging Face secret stores, never in the repository.
- Imported documents are rendered as plain text only.
