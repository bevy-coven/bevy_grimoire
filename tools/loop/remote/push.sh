#!/bin/bash
# Run on big. WT is this pass's worktree (default ~/pass-wt), TAG names its scratch under /tmp (default pass).
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
# usage: push.sh <branch> : merge origin/main and the branch in the loop clone, push main through the gate
source ~/.cargo/env 2>/dev/null
export CARGO_TARGET_DIR=$HOME/.cargo-target-shared/global
cd ~/bevy_emerge_loop || exit 1
[ -z "$(git status --short)" ] || { echo "loop clone not clean"; exit 1; }
git fetch -q origin && git merge -q --ff-only origin/main && git merge -q --no-edit "$1" && git push origin main 2>&1 | tail -12
