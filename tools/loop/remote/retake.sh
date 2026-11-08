#!/bin/bash
# Run on big. WT is this pass's worktree (default ~/pass-wt), TAG names its scratch under /tmp (default pass).
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
# usage: retake.sh "<names>" : retake standing demos with the binaries in /tmp/${TAG}_new
cd $WT && git log -1 --format=%h
mkdir -p /tmp/${TAG}_bins && cp /tmp/${TAG}_new/${TAG}_cauldron /tmp/${TAG}_bins/${TAG}_cauldron && cp /tmp/${TAG}_new/${TAG}_cauldron_mcp /tmp/${TAG}_bins/${TAG}_cauldron_mcp
export DISPLAY=:88
pgrep -f "Xvfb :88" >/dev/null || (Xvfb :88 -screen 0 1280x720x24 >/tmp/${TAG}_xvfb.log 2>&1 &)
sleep 2
rm -rf /tmp/${TAG}_retake && mkdir -p /tmp/${TAG}_retake
REPO=$WT EDITOR_BIN=/tmp/${TAG}_bins/${TAG}_cauldron MCP_BIN=/tmp/${TAG}_bins/${TAG}_cauldron_mcp WORK=/tmp/${TAG}_retake PORT=15764 ONLY="$1" bash tools/demo/retake.sh
cat /tmp/${TAG}_retake/summary.txt
