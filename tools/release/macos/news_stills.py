#!/usr/bin/env python3
"""Prints the still of each entry the What's new page lists, one repo-relative path per line.

The page (launcher::news_page) lists the newest N changelog entries that carry a `*For you: ...*` sentence, each with the
`<img src="docs/changelog/media/<name>.png">` of its clip. build_app.sh copies those stills into the app's Resources, where
launcher::still_image looks for them, so the app carries a handful of images and not every clip's still.

    news_stills.py CHANGELOG.md 10
"""
import re
import sys


def stills(changelog: str, shown: int) -> list[str]:
    out = []
    entries = 0
    for line in changelog.splitlines():
        # The same two tests launcher::news makes: an entry line, and its For you sentence.
        if not line.startswith("- **") or " *For you: " not in line:
            continue
        entries += 1
        if entries > shown:
            break
        match = re.search(r'<img src="([^"]+\.png)"', line)
        if match and match.group(1) not in out:
            out.append(match.group(1))
    return out


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: news_stills.py CHANGELOG.md N")
    with open(sys.argv[1], encoding="utf-8") as f:
        print("\n".join(stills(f.read(), int(sys.argv[2]))))
