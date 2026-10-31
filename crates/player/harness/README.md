# Player harness

Drives `nolabs_player::Player` in headless Chrome against real Kokoro clips and checks it against ground truth.

    uv run --script crates/player/harness/run.py            # build, run every scenario, write out/report.json
    uv run --script crates/player/harness/run.py --only 04  # scenarios whose name contains "04"
    --no-build   reuse dist/      --headful   show the browser      --raw   also write every event log
    --tmp DIR    where the throwaway Chrome profile goes (default: system temp)

Needs: Chrome (`/Applications/Google Chrome*.app`, or `CHROME=`), the wasm32 target, and the wasm-bindgen CLI at the
version in Cargo.lock (trunk's cache has it after one `trunk build`). About 2 min 15 s on the Intel Mac.

## Parts
- `src/lib.rs`: wasm wrapper (built as the `harness` example of the player crate, so no workspace change).
- `web/`: the page. It instruments MediaSource, SourceBuffer, the media element and the Media Session, serves fixture
  clips through the Provider with optional delays and failures (`configure({delay_ms, delays, fail, fatal})`, or the
  URL query `?delay=1200&delays=5:3000&fail=3:1&fatal=5:2`; `fail` is retryable, `fatal` is final). `?nomse=1`
  (scenario `config.query`) hides MediaSource so the player takes the blob fallback; the page then logs one
  synthetic build of fixture-length appends and reports `ct` as document time, and run.py skips `gapless`. It exposes
  `window.__harness` (load, play, pause, seek_by, jump_to, next, previous, set_rate, set_sleep, action, state, drain).
- `fixtures/`: 26 Kokoro af_heart clips (318.9 s), `manifest.json` with durations and word timings. Re-render with
  `fetch_fixtures.py --force` (logs in with the passphrase in `~/.config/nolabs/app.env`).
- `scenarios/*.json`: steps as data. Add a file to add a scenario.

## Scenario steps
`{"do": "seek_by", "args": [15000], "gap": 60}` runs a player command (`gap` = ms before the next step);
`{"action": "seekforward", "details": {"seekOffset": 10}}` calls a Media Session handler; `{"wait": ms}`;
`{"settle": true}` waits for the player to settle and judges the commands since the last settle;
`{"observe": ms}` checks playback over a window; `{"until": "Ended"}`; `{"configure": {...}}`;
`{"assert": {"state": "Paused", "segment": 16, "max_pos_ms": 300}}` (`"session": true` also requires every Media
Session handler and the metadata); `{"do": "reopen", "args": [4]}` replaces the player the way the web app opens
another document (new player, old one unloaded and dropped, new one loaded at that segment). Top level: `config`, `expect_events`
(`Ended`, `SleepFired`, `media_ended` counts), `allow_errors`.

Expected landings are derived from the command: seek_by from the position (or the previous unsettled target),
jump/next/previous from segment starts. Ground truth for the playhead is the element's `currentTime` mapped through
the appended segments (each starts where the buffer ended when it was appended); the timeline check holds those
lengths to the fixture durations (within 45 ms).

## Checks
tracking, seek, jump, timeline, gapless, rapid, words, media_session, ended, errors: see the docstring of `run.py`.
