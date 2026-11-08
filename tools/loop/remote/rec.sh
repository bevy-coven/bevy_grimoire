#!/bin/bash
# Run on big. WT is this pass's worktree (default ~/pass-wt), TAG names its scratch under /tmp (default pass).
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
# usage: rec.sh <storyboard-name> [width]: record one storyboard on a fresh editor, render mp4 + sheet
source ~/.cargo/env 2>/dev/null
NAME=$1; W=${2:-900}
export DISPLAY=:89
pgrep -f "Xvfb :89" >/dev/null || (Xvfb :89 -screen 0 1280x720x24 >/tmp/${TAG}_xvfb89.log 2>&1 &)
sleep 2
OUT=/tmp/${TAG}_rec_$NAME; rm -rf $OUT; mkdir -p $OUT/home $OUT/proj
cp -r $WT/assets $OUT/proj/assets
BIN=/tmp/${TAG}_new/${TAG}_cauldron; MCP=/tmp/${TAG}_new/${TAG}_cauldron_mcp; PORT=15767
(cd $OUT/proj && HOME=$OUT/home XDG_CONFIG_HOME=$OUT/home/config BEVY_BRP_PORT=$PORT exec $BIN . bedroom > $OUT/editor.log 2>&1 < /dev/null) &
EPID=$!
for i in $(seq 1 60); do curl -s -X POST http://127.0.0.1:$PORT -d '{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}' | grep -q Bevy && break; sleep 1; done
sleep 80
cd $WT && BEVY_GRIMOIRE_MCP=$MCP timeout 400 python3 tools/demo/record.py tools/demo/storyboards/$NAME.json --port $PORT --project $OUT/proj 2>&1 | tail -2
EV=$OUT/proj/target/loop/demo/$NAME/events.json
python3 tools/demo/render.py $EV --out /tmp/${TAG}_$NAME.mp4 --width $W --sheet $OUT/sheet.png 2>&1 | tail -1
kill $EPID 2>/dev/null; sleep 2; kill -9 $EPID 2>/dev/null
ls $OUT
