# Raw: on-device Kokoro benchmark (2026-10-30)

Scripts and logs behind [../../19-on-device-tts.md](../../19-on-device-tts.md). Run with Python 3.12 + onnxruntime 1.23.2 (uv venv) on the owner's Intel Mac (i7-9750H, 6 cores / 12 threads, 32 GB). `ids.json` holds the misaki 0.9.4 phonemes, token ids, style index and int8 durations for the 4 test segments, so `bench_ids.py` needs no G2P packages.

- `bench.py`: misaki G2P + ORT, thread sweep (run 1).
- `save_ids.py`: writes `ids.json`, checks int8 rerun determinism.
- `bench_ids.py`: wall + process CPU time, duration parity vs int8, port of kokoro 0.9.4 `join_timestamps`.
- `bench-log.txt`: all three runs with load averages.
