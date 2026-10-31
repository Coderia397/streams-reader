"""Parity of nolabs BartFallback with misaki HEAD FallbackNetwork (copied logic), plus timing."""
import sys, time
sys.path.insert(0, sys.argv[1])
import torch
from transformers import BartForConditionalGeneration
from nolabs_engine import revisions
from nolabs_engine.engines.g2p_fallback import BartFallback

WORDS = ["Zorblax", "Hermione", "Daenerys", "quokka", "Nguyen", "Szczepanski", "flibbertigibbet", "Kubernetes",
         "Tauri", "Xiaomi", "Ouagadougou", "Llanfair", "antidisestablishmentarianism", "Cthulhu", "Bezos",
         "Kokoro", "misaki", "nolabs", "Pfizer", "Worcestershire"]
for code in "ab":
    repo, sha = revisions.G2P_REPOS[code]
    d = revisions.snapshot(repo, sha, revisions.G2P_FILES)
    ours = BartFallback(d)
    m = BartForConditionalGeneration.from_pretrained(d).eval()
    g2t = {g: i for i, g in enumerate(m.config.grapheme_chars)}
    t2p = {i: p for i, p in enumerate(m.config.phoneme_chars)}
    same = 0
    t = time.perf_counter()
    for w in WORDS:
        a = ours.phonemize(w)
        ids = torch.tensor([[1] + [g2t.get(g, 3) for g in w] + [2]])
        with torch.no_grad():
            out = m.generate(input_ids=ids)
        b = "".join(t2p.get(x, '') for x in out[0].tolist() if x > 3)
        same += a == b
        if code == "a": print(f"{w:32s} {a}")
    print(code, "parity", same, "/", len(WORDS))
    t = time.perf_counter()
    ours.phonemize.cache_clear()
    for w in WORDS: ours.phonemize(w)
    print(code, f"ms/word uncached {1000*(time.perf_counter()-t)/len(WORDS):.1f}")
