"""Built-in voice ids the Space accepts. Mirrors `crates/core/src/voices.rs`; kept free of torch so contracts can
validate requests without loading a model."""
from __future__ import annotations

# English Kokoro-82M voices (the 28 listed in crates/core/src/voices.rs::KOKORO).
KOKORO_VOICES: frozenset[str] = frozenset({
    "af_heart", "af_bella", "af_nicole", "af_aoede", "af_kore", "af_sarah", "af_alloy", "af_nova", "af_sky",
    "af_jessica", "af_river",
    "am_michael", "am_fenrir", "am_puck", "am_echo", "am_eric", "am_liam", "am_onyx", "am_santa", "am_adam",
    "bf_emma", "bf_isabella", "bf_alice", "bf_lily",
    "bm_george", "bm_fable", "bm_lewis", "bm_daniel",
})

# English built-in voices of Ming-3.0-tts-0.5B (crates/core/src/voices.rs::MING). None has emotion variants, so
# emotion on a preset goes through an anchor clip (see engines/ming_plan.py).
MING_PRESETS: frozenset[str] = frozenset({"US_Emma", "US_James", "US_Stella", "UK_George", "UK_Charlotte"})


def kokoro_lang_code(voice_id: str) -> str:
    """Kokoro pipeline language: `a` (American) or `b` (British), the first letter of the voice id."""
    return voice_id[0]
