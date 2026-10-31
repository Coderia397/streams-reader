#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Renders the player harness fixtures once: ~30 Kokoro (af_heart) segments of public-domain text, cut with the
Kokoro segment rules (sentence packs of 120-320 chars, headings on their own), downloaded as WebM/Opus into
harness/fixtures/ with manifest.json. Skips the work when the manifest already exists (pass --force to redo).

Render takes the segment texts directly, so no document is created on the server. The passphrase is read from
~/.config/nolabs/app.env and never printed.
"""

import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

BASE = "https://listen.streams.watch"
VOICE = "kokoro:af_heart"
HERE = Path(__file__).resolve().parent
OUT = HERE / "fixtures"
MIN_CHARS, MAX_CHARS = 120, 320

# Lewis Carroll, Alice's Adventures in Wonderland (1865), public domain. Headings start with "#".
TEXT = """# Chapter I. Down the Rabbit-Hole

Alice was beginning to get very tired of sitting by her sister on the bank, and of having nothing to do: once or twice she had peeped into the book her sister was reading, but it had no pictures or conversations in it, "and what is the use of a book," thought Alice "without pictures or conversations?"

So she was considering in her own mind (as well as she could, for the hot day made her feel very sleepy and stupid), whether the pleasure of making a daisy-chain would be worth the trouble of getting up and picking the daisies, when suddenly a White Rabbit with pink eyes ran close by her.

There was nothing so very remarkable in that; nor did Alice think it so very much out of the way to hear the Rabbit say to itself, "Oh dear! Oh dear! I shall be late!" But when the Rabbit actually took a watch out of its waistcoat-pocket, and looked at it, and then hurried on, Alice started to her feet. Burning with curiosity, she ran across the field after it, and fortunately was just in time to see it pop down a large rabbit-hole under the hedge.

In another moment down went Alice after it, never once considering how in the world she was to get out again. The rabbit-hole went straight on like a tunnel for some way, and then dipped suddenly down, so suddenly that Alice had not a moment to think about stopping herself before she found herself falling down a very deep well.

Either the well was very deep, or she fell very slowly, for she had plenty of time as she went down to look about her and to wonder what was going to happen next. First, she tried to look down and make out what she was coming to, but it was too dark to see anything; then she looked at the sides of the well, and noticed that they were filled with cupboards and book-shelves.

"Well!" thought Alice to herself, "after such a fall as this, I shall think nothing of tumbling down stairs! How brave they'll all think me at home!" Down, down, down. Would the fall never come to an end? "I wonder how many miles I've fallen by this time?" she said aloud.

Down, down, down. There was nothing else to do, so Alice soon began talking again. "Dinah'll miss me very much to-night, I should think!" (Dinah was the cat.) "I hope they'll remember her saucer of milk at tea-time. Dinah my dear! I wish you were down here with me! There are no mice in the air, I'm afraid, but you might catch a bat, and that's very like a mouse, you know."

Suddenly, thump! thump! down she came upon a heap of sticks and dry leaves, and the fall was over. Alice was not a bit hurt, and she jumped up on to her feet in a moment: she looked up, but it was all dark overhead; before her was another long passage, and the White Rabbit was still in sight, hurrying down it.

There were doors all round the hall, but they were all locked; and when Alice had been all the way down one side and up the other, trying every door, she walked sadly down the middle, wondering how she was ever to get out again. Suddenly she came upon a little three-legged table, all made of solid glass; there was nothing on it except a tiny golden key.

Alice opened the door and found that it led into a small passage, not much larger than a rat-hole: she knelt down and looked along the passage into the loveliest garden you ever saw. How she longed to get out of that dark hall, and wander about among those beds of bright flowers and those cool fountains, but she could not even get her head through the doorway.

There seemed to be no use in waiting by the little door, so she went back to the table, half hoping she might find another key on it. This time she found a little bottle on it, and round the neck of the bottle was a paper label, with the words "DRINK ME," beautifully printed on it in large letters.

# Chapter II. The Pool of Tears

"Curiouser and curiouser!" cried Alice (she was so much surprised, that for the moment she quite forgot how to speak good English); "now I'm opening out like the largest telescope that ever was! Good-bye, feet!" For when she looked down at her feet, they seemed to be almost out of sight, they were getting so far off.

Just then her head struck against the roof of the hall: in fact she was now more than nine feet high, and she at once took up the little golden key and hurried off to the garden door. Poor Alice! It was as much as she could do, lying down on one side, to look through into the garden with one eye; but to get through was more hopeless than ever.

She sat down and began to cry again. "You ought to be ashamed of yourself," said Alice, "a great girl like you, to go on crying in this way! Stop this moment, I tell you!" But she went on all the same, shedding gallons of tears, until there was a large pool all round her, about four inches deep and reaching half down the hall.

After a time she heard a little pattering of feet in the distance, and she hastily dried her eyes to see what was coming. It was the White Rabbit returning, splendidly dressed, with a pair of white kid gloves in one hand and a large fan in the other.

# Chapter III. A Caucus-Race and a Long Tale

They were indeed a queer-looking party that assembled on the bank, the birds with draggled feathers, the animals with their fur clinging close to them, and all dripping wet, cross, and uncomfortable. The first question of course was, how to get dry again: they had a consultation about this, and after a few minutes it seemed quite natural to Alice to find herself talking familiarly with them.
"""


def sentences(paragraph: str) -> list[str]:
    parts = re.split(r'(?:(?<=[.!?:;])|(?<=[.!?:;]["\')]))\s+(?=["(A-Z])', paragraph.strip())
    return [p.strip() for p in parts if p.strip()]


def segments() -> list[dict]:
    """Kokoro rules: never split a sentence, pack to 120-320 chars, never cross a block, headings alone."""
    blocks = [b.strip() for b in TEXT.strip().split("\n\n") if b.strip()]
    out: list[dict] = []
    for bi, block in enumerate(blocks):
        heading = block.startswith("#")
        next_heading = bi + 1 < len(blocks) and blocks[bi + 1].startswith("#")
        end_boundary = "section" if heading or next_heading or bi + 1 == len(blocks) else "paragraph"
        if heading:
            out.append({"text": block.lstrip("# ").strip(), "boundary": end_boundary})
            continue
        packs: list[str] = []
        cur = ""
        for s in sentences(block):
            cand = f"{cur} {s}".strip()
            if cur and len(cand) > MAX_CHARS and len(cur) >= MIN_CHARS:
                packs.append(cur)
                cur = s
            else:
                cur = cand
        if cur:
            if packs and len(cur) < MIN_CHARS and len(packs[-1]) + len(cur) + 1 <= MAX_CHARS * 5 // 4:
                packs[-1] = f"{packs[-1]} {cur}"
            else:
                packs.append(cur)
        for i, p in enumerate(packs):
            out.append({"text": p, "boundary": end_boundary if i + 1 == len(packs) else "sentence"})
    return out


class Api:
    def __init__(self) -> None:
        self.cookie = ""

    def call(self, method: str, path: str, body: object | None = None, raw: bool = False):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(BASE + path, data=data, method=method)
        req.add_header("x-nolabs-api", "1")
        req.add_header("Origin", BASE)
        req.add_header("User-Agent", "nolabs-player-harness/1")
        if data is not None:
            req.add_header("Content-Type", "application/json")
        if self.cookie:
            req.add_header("Cookie", self.cookie)
        with urllib.request.urlopen(req, timeout=120) as resp:
            for header in resp.headers.get_all("Set-Cookie") or []:
                if header.startswith("__Host-nolabs_s="):
                    self.cookie = header.split(";", 1)[0]
            payload = resp.read()
        if raw:
            return payload
        return json.loads(payload) if payload else None


def passphrase() -> str:
    env = Path.home() / ".config/nolabs/app.env"
    for line in env.read_text().splitlines():
        if line.startswith("PASSPHRASE="):
            return line.split("=", 1)[1].strip().strip('"').strip("'")
    sys.exit("PASSPHRASE missing from ~/.config/nolabs/app.env")


def main() -> None:
    manifest_path = OUT / "manifest.json"
    if manifest_path.exists() and "--force" not in sys.argv:
        print(f"fixtures present: {manifest_path}")
        return
    segs = segments()
    print(f"{len(segs)} segments, {sum(len(s['text']) for s in segs)} chars")
    api = Api()
    api.call("POST", "/api/session", {"passphrase": passphrase()})
    if not api.cookie:
        sys.exit("login did not return a session cookie")
    items = [{"text": s["text"], "boundary": s["boundary"]} for s in segs]
    resp = api.call("POST", "/api/render", {"voice": VOICE, "items": items, "priority": "interactive"})
    keys = [it["key"] for it in resp["items"]]
    status = {it["key"]: it for it in resp["items"]}
    deadline = time.time() + 900
    while True:
        pending = [k for k in keys if status[k]["state"] != "ready"]
        failed = [k for k in keys if status[k]["state"] == "failed"]
        if failed:
            sys.exit(f"render failed: {[status[k].get('error') for k in failed]}")
        print(f"ready {len(keys) - len(pending)}/{len(keys)}", flush=True)
        if not pending:
            break
        if time.time() > deadline:
            sys.exit("render timed out")
        time.sleep(5)
        for i in range(0, len(pending), 10):
            batch = pending[i : i + 10]
            for it in api.call("GET", "/api/render/status?keys=" + ",".join(batch))["items"]:
                status[it["key"]] = it
    OUT.mkdir(parents=True, exist_ok=True)
    manifest = []
    for i, (seg, key) in enumerate(zip(segs, keys)):
        audio = status[key]["audio"]
        data = api.call("GET", f"/api/audio/{key}", raw=True)
        name = f"{i:02d}.webm"
        (OUT / name).write_bytes(data)
        manifest.append(
            {
                "segment": i,
                "file": name,
                "key": key,
                "duration_ms": audio["duration_ms"],
                "words": audio.get("words", []),
                "ends_section": seg["boundary"] == "section",
                "text": seg["text"],
            }
        )
    try:
        api.call("DELETE", "/api/session")
    except urllib.error.HTTPError:
        pass
    manifest_path.write_text(json.dumps({"voice": VOICE, "segments": manifest}, indent=1))
    total = sum(m["duration_ms"] for m in manifest)
    size = sum((OUT / m["file"]).stat().st_size for m in manifest)
    print(f"wrote {len(manifest)} clips, {total / 1000:.1f} s audio, {size / 1024:.0f} KiB")


if __name__ == "__main__":
    main()
