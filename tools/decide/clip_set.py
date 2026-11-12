#!/usr/bin/env python3
"""Extract the clip check's labelled set from the changelog's own clips (#112).

    python3 tools/decide/clip_set.py --out /tmp/clipset

`crates/bevy_oracle/fixtures/clip_samples.yaml` names every sample; this script makes the images those
samples point at. Four per clip — the still beside the MP4 and three frames spread through it — because
the API takes at most four images and the still is the fourth.

**The set is derived, so it is not committed.** Sixty-one clips × four PNGs is about 10 MB of data that
`extract` remakes in seconds from files the changelog already names; what is committed is the manifest,
which is the part a person edits (a title, a mismatch).

A clip with no still beside it gets one from its first frame, so a clip recorded before stills were
written is still usable. A clip ffprobe cannot read is named and skipped, never silently dropped.
"""
import argparse
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def samples(changelog):
    """Every `**id** `sha` title` line whose clip is in the media directory."""
    out = []
    with open(changelog, encoding="utf-8") as fh:
        for line in fh:
            m = re.search(r"\*\*([a-z0-9\-]+)\*\* `([0-9a-f]{7})` (.+?)(?:<br>|\*No clip|$)", line)
            clip = re.search(r"(docs/changelog/media/[^\"]+\.mp4)", line)
            if m and clip:
                out.append({"id": m.group(1), "title": m.group(3).strip(), "clip": clip.group(1)})
    return out


# **The width a sample is sent at** (#112). Four frames at 960 px are about 2 100 tokens, and a served
# clef-flash has a 2 048-token physical batch: the request is refused before the model sees it. 480 px
# keeps the four-image request inside one batch and still shows the layout a clip's title is about —
# what is being asked is "does this show a bench for stallkeepers", not a pixel comparison.
SAMPLE_W = 480


def extract(clip, dest):
    os.makedirs(dest, exist_ok=True)
    still = clip[:-4] + ".png"
    target = os.path.join(dest, "still.png")
    if os.path.exists(still):
        run(["ffmpeg", "-v", "error", "-y", "-i", still, "-vf", f"scale={SAMPLE_W}:-2", target])
    else:
        run(["ffmpeg", "-v", "error", "-y", "-i", clip, "-vframes", "1",
             "-vf", f"scale={SAMPLE_W}:-2", target])
    probe = run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", clip])
    duration = float(probe.stdout.strip() or 0)
    for i, frac in enumerate((0.25, 0.55, 0.85)):
        run(["ffmpeg", "-v", "error", "-y", "-ss", f"{max(0.0, duration * frac):.2f}", "-i", clip,
             "-vframes", "1", "-vf", f"scale={SAMPLE_W}:-2", os.path.join(dest, f"frame{i}.png")])


def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd[:3])}…: {r.stderr.strip()[:200]}")
    return r


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=os.path.join(REPO, "target/clip_set"), help="where the sample images go")
    ap.add_argument("--manifest", default=os.path.join(REPO, "crates/bevy_oracle/fixtures/clip_samples.yaml"),
                    help="the committed manifest, whose relative dirs are relative to its own directory")
    args = ap.parse_args(argv)
    if subprocess.run(["which", "ffmpeg"], capture_output=True).returncode != 0:
        sys.exit("ffmpeg is not on PATH, so the frames cannot be extracted")

    clips = samples(os.path.join(REPO, "CHANGELOG.md"))
    if not clips:
        sys.exit("no changelog clip lines found: nothing to extract")
    made = 0
    for c in clips:
        extract(os.path.join(REPO, c["clip"]), os.path.join(args.out, c["id"]))
        made += 1
    print(f"{made} clip(s) extracted into {args.out}")
    print(f"the manifest is {args.manifest}; point its `dir` fields at {args.out}/<id> to run the set")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
