#!/bin/bash
# Run on big. WT is this pass's worktree (default ~/pass-wt), TAG names its scratch under /tmp (default pass).
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
# usage: cargo.sh <ref> <cmd...> : check out <ref> in $WT, run the command there
source ~/.cargo/env 2>/dev/null
export CARGO_TARGET_DIR=$HOME/.cargo-target-shared/global
cd $WT && git checkout -q -f --detach "$1" && shift && exec "$@"
