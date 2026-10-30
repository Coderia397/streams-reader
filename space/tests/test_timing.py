from dataclasses import dataclass

from nolabs_engine.engines.timing import place_tokens, shift_words


@dataclass
class Tok:
    text: str
    start_ts: float | None
    end_ts: float | None


def test_offsets_are_code_point_indices_into_the_exact_text():
    text = "  “Café” — naïve 😀 friends!"
    tokens = [Tok("“", 0.0, 0.05), Tok("Café", 0.05, 0.4), Tok("”", 0.4, 0.4), Tok("—", 0.4, 0.5),
              Tok("naïve", 0.5, 0.9), Tok("😀", 0.9, 1.0), Tok("friends", 1.0, 1.4), Tok("!", 1.4, 1.4)]
    words: list = []
    cursor = place_tokens(text, tokens, 0.0, 0, words)
    assert [text[cs:ce] for _, _, cs, ce in words] == ["Café", "naïve", "friends"]
    assert words[0] == (50, 400, 3, 7)
    assert cursor == text.index("friends") + len("friends")


def test_offset_of_later_chunks_and_untimed_or_rewritten_tokens():
    text = "Dr. Smith paid $5 today."
    words: list = []
    cursor = place_tokens(text, [Tok("Dr.", 0.0, 0.3), Tok("Smith", 0.3, None)], 0.0, 0, words)
    # "five dollars" is the G2P's rewrite and does not occur in the text; it is skipped, not mis-placed.
    place_tokens(text, [Tok("five dollars", 0.0, 0.5), Tok("today", 0.5, 0.9)], 2.0, cursor, words)
    assert words == [(0, 300, 0, 3), (2500, 2900, 18, 23)]


def test_repeated_words_map_left_to_right():
    text = "no, no, no"
    words: list = []
    place_tokens(text, [Tok("no", 0, 0.1), Tok("no", 0.2, 0.3), Tok("no", 0.4, 0.5)], 0.0, 0, words)
    assert [cs for _, _, cs, _ in words] == [0, 4, 8]


def test_shift_words_moves_by_trimmed_lead_and_clamps_to_speech():
    words = [(100, 300, 0, 4), (350, 900, 5, 9), (1200, 1500, 10, 14)]
    assert shift_words(words, lead_ms=150.4, speech_ms=1000) == (
        (0, 150, 0, 4), (200, 750, 5, 9), (1000, 1000, 10, 14))
