import re

from conftest import SPACE

from nolabs_engine import revisions
from nolabs_engine.engines import g2p_fallback


def test_decode_drops_special_ids_and_maps_the_rest():
    phonemes = {0: "_", 1: "_", 2: "_", 3: "_", 4: "k", 5: "ˈO", 6: "k", 7: "ə"}
    assert g2p_fallback.decode([1, 4, 5, 3, 6, 7, 2, 0, 0], phonemes) == "kˈOkə"
    assert g2p_fallback.decode([1, 2], phonemes) == ""


def test_generation_length_matches_misaki_default():
    # misaki main relies on transformers' default max_length; the apps' greedy loop must stop at the same place.
    assert g2p_fallback.MAX_LENGTH == 20


def test_every_pinned_model_is_preloaded_at_the_same_commit():
    readme = (SPACE / "README.md").read_text(encoding="utf-8")
    preload = dict(re.findall(r"^  - (\S+) \S+ ([0-9a-f]{40})$", readme, flags=re.M))
    pinned = {revisions.KOKORO_REPO: revisions.KOKORO_COMMIT, revisions.MING_REPO: revisions.MING_COMMIT,
              **dict(revisions.G2P_REPOS.values())}
    assert preload == pinned
    files = dict(re.findall(r"^  - (\S+) (\S+) [0-9a-f]{40}$", readme, flags=re.M))
    for repo, _ in revisions.G2P_REPOS.values():
        assert files[repo].split(",") == list(revisions.G2P_FILES)


def test_health_reports_the_g2p_commits():
    assert revisions.MODEL_REVS["kokoro_g2p"] == (
        "PeterReid/graphemes_to_phonemes_en_us@a5631b285d18d59483c32c0c3379cb9fac924f4b,"
        "PeterReid/graphemes_to_phonemes_en_gb@d8357d5067fa26a5c34134d6bbcf4bbf000c0ac8")
