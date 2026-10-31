"""Director output -> what Kokoro-82M can do. Pure functions, no model; render_local.py executes the result.

Every number here is a mapping decision measured in notes/33 (the recommended set is RECOMMENDED below CONDITIONS).
Inputs are the validated director sentences from director.py (`plan` entries).
"""
from __future__ import annotations

from dataclasses import dataclass, field

# Boundary pauses the app uses today (crates/server/src/config.rs PAUSES).
BOUNDARY_PAUSE_MS = {"sentence": 250, "paragraph": 600, "section": 1200}

# ---------------------------------------------------------------------------------------------------------- speed
# Direction of the tempo change by emotion when the director's pace is "normal"; pace slow/fast overrides it.
EMOTION_TEMPO = {"neutral": 0.0, "happy": 0.5, "surprised": 1.0, "angry": 1.0, "fearful": 0.5, "sad": -1.0,
                 "disgusted": -0.5}
SPEED_MIN_PCT, SPEED_SPAN_PCT = 4.0, 8.0     # |change| = 4 % + 8 % x intensity, so 4-12 %


def speed_of(s: dict) -> float:
    inten = s["intensity"]
    if s["pace"] == "slow":
        direction = -1.0
    elif s["pace"] == "fast":
        direction = 1.0
    else:
        direction = EMOTION_TEMPO.get(s["emotion"], 0.0) if inten >= 0.4 else 0.0
    if direction == 0.0:
        return 1.0
    pct = SPEED_MIN_PCT + SPEED_SPAN_PCT * inten
    return round(1.0 + direction * pct / 100.0, 3)


# ---------------------------------------------------------------------------------------------------------- pauses
def pause_after(s: dict, use_director: bool) -> int:
    if use_director:
        return int(s["pause_after_ms"])
    return BOUNDARY_PAUSE_MS[s["boundary"]] if s["last_in_segment"] else BOUNDARY_PAUSE_MS["sentence"]


# ---------------------------------------------------------------------------------------------------- voice blends
# Partner voices per narrator: "animated" for high-arousal emotions, "soft" for low-arousal ones.
PARTNERS = {
    "af_heart": {"animated": "af_bella", "soft": "af_nicole"},
    "bf_emma": {"animated": "bf_isabella", "soft": "af_nicole"},
    "am_michael": {"animated": "am_puck", "soft": "am_echo"},
    "bm_george": {"animated": "bm_fable", "soft": "bm_lewis"},
}
AROUSAL = {"happy": "animated", "surprised": "animated", "angry": "animated", "fearful": "soft", "sad": "soft",
           "disgusted": "soft"}
BLEND_MAX = 0.45          # partner weight = BLEND_MAX x intensity
BLEND_MIN_INTENSITY = 0.3


def blend_of(voice: str, s: dict) -> dict[str, float]:
    kind = AROUSAL.get(s["emotion"])
    partner = PARTNERS.get(voice, {}).get(kind) if kind else None
    if not partner or s["intensity"] < BLEND_MIN_INTENSITY:
        return {voice: 1.0}
    w = round(BLEND_MAX * s["intensity"], 3)
    return {voice: 1.0 - w, partner: w}


# ------------------------------------------------------------------------------------------------------ characters
# Character voices: same accent family as the narrator first, never the narrator's own voice.
CHARACTER_POOL = {
    "a": {"female": ["af_bella", "af_sarah", "af_nicole"], "male": ["am_michael", "am_fenrir", "am_puck"]},
    "b": {"female": ["bf_isabella", "bf_emma"], "male": ["bm_george", "bm_fable", "bm_lewis"]},
}


def cast(narrator: str, characters: list[dict]) -> dict[str, str]:
    """name -> Kokoro voice. Unknown gender keeps the narrator voice (assumed safest)."""
    lang = narrator[0]
    used, out = {narrator}, {}
    for c in characters:
        pool = CHARACTER_POOL[lang].get(c.get("gender"), [])
        voice = next((v for v in pool if v not in used), None)
        if voice:
            used.add(voice)
            out[c["name"]] = voice
    return out


# -------------------------------------------------------------------------------------------- emphasis and pitch
EMPH_SPLIT_PAUSE_MS = 90       # A: separate call from the emphasised word on, with this gap
EMPH_SPLIT_MIN_WORDS = 3       # ... only when at least this many words come before it
EMPH_DUR = 1.25                # B: the word's phonemes 25 % longer (in-call)
EMPH_PRE_FRAMES = 2            # B: +50 ms on the space before the word
EMPH_F0_ST = 1.5               # B+f0: pitch accent on the word
# Sentence register shift in semitones at intensity 1 (B+f0); the gap to the readers is pitch-level change between
# phrases (notes/20), which Kokoro does not make on its own.
REGISTER_ST = {"neutral": 0.0, "happy": 1.5, "surprised": 2.0, "angry": 1.5, "fearful": -1.0, "sad": -1.5,
               "disgusted": -1.0}
# Loudness by emotion at intensity 1, dB, applied after the Space's segment levelling (full condition only).
GAIN_DB = {"neutral": 0.0, "happy": 1.0, "surprised": 1.5, "angry": 3.0, "fearful": -1.5, "sad": -2.0,
           "disgusted": -0.5}


def register_of(s: dict) -> float:
    return round(REGISTER_ST.get(s["emotion"], 0.0) * s["intensity"], 2)


def gain_of(s: dict) -> float:
    return round(GAIN_DB.get(s["emotion"], 0.0) * s["intensity"], 2)


@dataclass(frozen=True)
class Condition:
    name: str
    unit: str = "segment"         # "segment": one call per paragraph (the app today); "sentence": one per sentence
    director_pauses: bool = False
    speed: bool = False
    emphasis: str | None = None   # None | "split" | "dur"
    f0: bool = False
    blend: bool = False
    characters: bool = False
    gain: bool = False
    narrator: str = "af_heart"
    notes: str = ""


CONDITIONS = [
    Condition("k_plain", notes="local Kokoro, one call per segment, boundary pauses: the app today"),
    Condition("k_sent", unit="sentence", notes="one call per sentence, boundary pauses: the cost of splitting"),
    Condition("a_pause", unit="sentence", director_pauses=True),
    Condition("a_speed", unit="sentence", director_pauses=True, speed=True),
    Condition("a_emph", unit="sentence", director_pauses=True, speed=True, emphasis="split"),
    Condition("a_blend", unit="sentence", director_pauses=True, speed=True, blend=True),
    Condition("a_chars", unit="sentence", director_pauses=True, speed=True, blend=True, characters=True),
    Condition("b_speed", director_pauses=True, speed=True, notes="segment call, per-sentence speed in the durations"),
    Condition("b_emph", director_pauses=True, speed=True, emphasis="dur"),
    Condition("b_f0", director_pauses=True, speed=True, emphasis="dur", f0=True),
    Condition("b_chars", director_pauses=True, speed=True, emphasis="dur", f0=True, characters=True),
    Condition("b_full", director_pauses=True, speed=True, emphasis="dur", f0=True, characters=True, blend=True,
              gain=True),
]
BY_NAME = {c.name: c for c in CONDITIONS}

# notes/33, 2026-10-31: one Kokoro call per sentence (or per quote/speech-tag part), Kokoro `speed` from speed_of,
# the director's pause_after_ms as appended silence, a partner-voice blend by emotion, character voices by gender.
# No emphasis splitting, no in-call duration/F0 hooks, no gain: they added nothing measurable or need a custom forward.
RECOMMENDED = BY_NAME["a_chars"]


@dataclass
class Call:
    """One Kokoro model call."""
    segment: int
    text: str
    sentences: list[int]                  # director sentence indices in this call
    word_sentence: list[int]              # per whitespace word of `text`: its sentence
    mix: dict[str, float]
    speed: float = 1.0
    ctrl: dict[int, dict] = field(default_factory=dict)   # word idx -> {dur, pre_frames, f0_st}
    gaps_ms: dict[int, int] = field(default_factory=dict)  # word idx w -> silence wanted before word w (in-call)
    pause_after_ms: int = 0
    gain_db: float = 0.0
    speaker: str | None = None
