#!/bin/bash
# Run on big. WT is this pass's worktree (default ~/pass-wt), TAG names its scratch under /tmp (default pass).
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
source ~/.cargo/env 2>/dev/null
export CARGO_TARGET_DIR=$HOME/.cargo-target-shared/global
cd $WT && git log -1 --format=%h
cargo build -p bevy_cauldron -p bevy_cauldron_mcp 2>&1 | grep -E "^error|Finished" | head -3
mkdir -p /tmp/${TAG}_new /tmp/${TAG}_agent
cp $CARGO_TARGET_DIR/debug/bevy_cauldron /tmp/${TAG}_new/${TAG}_cauldron && cp $CARGO_TARGET_DIR/debug/bevy_cauldron_mcp /tmp/${TAG}_new/${TAG}_cauldron_mcp
cp $CARGO_TARGET_DIR/debug/bevy_cauldron /tmp/${TAG}_agent/bevy_cauldron && cp $CARGO_TARGET_DIR/debug/bevy_cauldron_mcp /tmp/${TAG}_agent/bevy_cauldron_mcp
ls /tmp/${TAG}_new
