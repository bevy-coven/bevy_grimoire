#!/bin/bash
# usage: test.sh <ref> <file holding the nextest filterset> [clippy]: check <ref> out in $WT and run the editor's tests that match; with a
# third argument, clippy over every target first. The filterset is read from a file (`scp` it over): big's login shell is fish,
# which eats the quotes and parentheses of an argument sent over ssh.
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
source ~/.cargo/env 2>/dev/null
export CARGO_TARGET_DIR=$HOME/.cargo-target-shared/global
FILTER=$(cat "$2")
cd $WT && git checkout -q -f --detach "$1" && git log -1 --format=%h || exit 1
if [ -n "$3" ]; then
  cargo clippy -p bevy_cauldron --all-targets -- -D warnings 2>&1 | grep -E "^(warning|error)|-->|Finished" | head -12
fi
cargo nextest run -p bevy_cauldron --no-fail-fast -E "$FILTER" 2>&1 | grep -E "^\s+(FAIL|SIGABRT)|Summary|panicked|^error|assert" | cut -c1-400 | head -30
