"""Model weights pinned by commit, so a push to a model repo never changes the sound behind an existing cache key.

The same `repo@commit` strings go into the Worker vars `KOKORO_REV` / `MING_REV` (notes/09) and into every cache key,
and `health` reports them so the Worker can refuse to render when the two disagree. Commits read from the HF API
(`GET /api/models/<repo>`, field `sha`) on 2026-10-30. Bump them together with the README `preload_from_hub` lines.
"""
from __future__ import annotations

import os
from collections.abc import Iterable

from .voices import KOKORO_VOICES

KOKORO_REPO = "hexgrad/Kokoro-82M"
KOKORO_COMMIT = "f3ff3571791e39611d31c381e3a41a3af07b4987"
KOKORO_MODEL_FILE = "kokoro-v1_0.pth"
KOKORO_FILES: tuple[str, ...] = ("config.json", KOKORO_MODEL_FILE,
                                 *sorted(f"voices/{v}.pt" for v in KOKORO_VOICES))

MING_REPO = "inclusionAI/Ming-3.0-tts-0.5B"
MING_COMMIT = "7247f4c94a5b2e987404ed28a5cfec41b1a4ca61"
# Everything the runtime loads; the repo also holds a PDF report and README figures that are not needed.
MING_FILES: tuple[str, ...] = (
    "added_tokens.json", "campplus.onnx", "config.json", "configuration.json", "merges.txt", "model.safetensors",
    "special_tokens_map.json", "tokenizer.json", "tokenizer_config.json", "vocab.json",
)

MODEL_REVS: dict[str, str] = {
    "kokoro": f"{KOKORO_REPO}@{KOKORO_COMMIT}",
    "ming": f"{MING_REPO}@{MING_COMMIT}",
}


def snapshot(repo: str, commit: str, files: Iterable[str]) -> str:
    """Local folder holding `files` of `repo` at `commit`. Uses the build-time preload when present (no network), and
    downloads otherwise."""
    from huggingface_hub import snapshot_download

    patterns = list(files)
    try:
        local = snapshot_download(repo, revision=commit, allow_patterns=patterns, local_files_only=True)
        if all(os.path.isfile(os.path.join(local, name)) for name in patterns):
            return local
    except Exception:  # noqa: BLE001 - any cache miss means: fetch it
        pass
    return snapshot_download(repo, revision=commit, allow_patterns=patterns)
