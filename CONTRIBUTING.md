# Contributing to Streams Reader

Thanks for helping. Streams Reader is a Rust project end to end, with a small Python speech engine; no Node.js.

## Before you start
- Read [`notes/00-index.md`](notes/00-index.md): the architecture, decisions and plans live in `notes/`.
- For anything larger than a small fix, open an issue first so we can agree on the approach.

## Set up
- Rust stable with the `wasm32-unknown-unknown` target, [trunk](https://trunkrs.dev), `worker-build`.
- [uv](https://docs.astral.sh/uv/) for Python (the speech engine, brand and media tools).

```sh
cargo test                                   # native crates
cargo clippy --all-targets -- -D warnings
cargo clippy -p nolabs-web -p nolabs-player -p nolabs-worker --target wasm32-unknown-unknown -- -D warnings
cd space && uv run --python 3.12 --with pytest --with numpy --with pyloudnorm --with soundfile python -m pytest -q tests
```

## How we write code
- Small, focused modules; doc comments explain *why*, not *what*.
- No `unwrap()` outside tests (the workspace denies it). Formatting follows `rustfmt.toml`.
- Pure logic goes in `crates/core` (or `crates/server` behind ports) so it can be tested natively.
- Styles use only brand tokens (`brand/dist/tokens`); `brand/check_tokens.py` fails the build on raw colours or radii.
- Every visible string goes through the translation files (8 languages).
- Contracts between components live in `crates/core/src/proto`; change both sides together and bump the version.

## Commits and pull requests
- One logical change per commit, with a message that says what and why.
- Add an entry under **Unreleased** in [`CHANGELOG.md`](CHANGELOG.md).
- CI must be green: formatting, clippy (native and wasm), tests, engine tests.

## Licence
By contributing you agree that your contributions are licensed under the [MIT licence](LICENSE).
