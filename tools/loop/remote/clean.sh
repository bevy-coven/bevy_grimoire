#!/bin/bash
# Run on big at the end of a pass: stop what this pass started and remove its worktree and scratch.
# Only this pass's: processes whose command line names /tmp/${TAG}_ or the pass's own Xvfb displays, and plain
# `git worktree remove` (never --force). It never touches another pass's worktree or the loop clone.
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
me=$$
for pat in 'Xvfb :89' 'Xvfb :88' 'Xvfb :87' "/tmp/${TAG}_agent/bevy_cauldron" "/tmp/${TAG}_bins/${TAG}_cauldron" "/tmp/${TAG}_new/${TAG}_cauldron"; do
  for p in $(pgrep -f "$pat"); do
    [ "$p" != "$me" ] && [ "$p" != "$PPID" ] && { echo "kill $p: $(ps -o args= -p "$p" | cut -c1-70)"; kill "$p"; }
  done
done
sleep 2
cd "$WT" && git status --short | head -5
cd ~/bevy_emerge_loop && git worktree remove "$WT" && echo "removed $WT"
rm -rf /tmp/${TAG}_*
git branch --list "${TAG}-*" | tr -d ' ' | tr '\n' ' '; echo " <- branches to list for the owner, not delete"
