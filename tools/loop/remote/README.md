# Building a pass on big from a machine with little disk

A pass on mac_air (about 9 GB free) edits locally and builds, tests, records and gates on big (`ssh big`). These
scripts are what passes 6 and 7 wrote by hand each time (about eight of them, and a fish-quoting mistake or two),
saved so a pass starts with them. They run **on big**, under bash.

Big's login shell is **fish**: never write bash syntax (`for`, `$(...)`, `VAR=...`) in an `ssh big '...'` argument. Copy a
script over and run it: `scp tools/loop/remote/*.sh big:/tmp/ && ssh big 'bash /tmp/cargo.sh <ref> ...'`. `pkill -f
<pattern>` over ssh kills the ssh command too: `clean.sh` finds pids and skips its own.

One pass, in order:

1. `ssh big 'cd ~/bevy_emerge_loop && git worktree add --detach ~/pN-wt <sha>'`, then give every script `WT` and `TAG` for the
   pass: `ssh big env WT=/home/<user>/pN-wt TAG=pN bash /tmp/build.sh` (`env` is the form fish accepts).
2. Push a branch from the Mac (`git push big:bevy_emerge_loop HEAD:refs/heads/pN-a`), then `cargo.sh pN-a cargo clippy ...`
   or `cargo.sh pN-a cargo nextest run -p bevy_cauldron ...` (it checks the ref out in `$WT` on the shared target).
3. `build.sh` builds the editor and the MCP server to `/tmp/${TAG}_new/` and `/tmp/${TAG}_agent/`.
4. `rec.sh <storyboard>` records one storyboard on a fresh editor (display :89, port 15767) and leaves `/tmp/${TAG}_<name>.mp4`
   and a contact sheet in `/tmp/${TAG}_rec_<name>/sheet.png`; look at the sheet. `retake.sh "<gif names>"` is
   `tools/demo/retake.sh` on display :88, port 15764 (empty: all eight). `editor_png.sh` retakes `docs/media/editor.png`
   (display :87, port 15765). The sibling loop on big uses other displays and ports: pick yours and say so in the log.
5. `scp` the results back and run `media.py <sha8> <dir of gifs> [editor.png]` on the Mac: it rewrites the *Captured* and
   frames columns in `docs/media/README.md` and copies the files.
6. `push.sh <branch>` merges `origin/main` and the branch in the loop clone and pushes `main` through the gate (the
   clone must be clean). Tag a release from the clone afterwards.
7. `clean.sh` stops this pass's displays and editors, removes the worktree and `/tmp/${TAG}_*`, and prints the
   branches to list for the owner (branches are never deleted).
