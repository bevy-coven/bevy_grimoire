#!/bin/bash
# Run on big. WT is this pass's worktree (default ~/pass-wt), TAG names its scratch under /tmp (default pass).
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
# editor.png, by the recipe in docs/media/README.md
source ~/.cargo/env 2>/dev/null
WT=$WT
OUT=/tmp/${TAG}_png
rm -rf $OUT && mkdir -p $OUT/home $OUT/proj
cp -r $WT/assets $OUT/proj/assets
export DISPLAY=:87
pgrep -f "Xvfb :87" >/dev/null || (Xvfb :87 -screen 0 1280x720x24 >/tmp/${TAG}_xvfb87.log 2>&1 &)
sleep 2
( cd $OUT/proj && HOME=$OUT/home XDG_DATA_HOME=$OUT/home/data XDG_CONFIG_HOME=$OUT/home/config BEVY_BRP_PORT=15765 exec /tmp/${TAG}_new/${TAG}_cauldron . bedroom > $OUT/editor.log 2>&1 < /dev/null ) &
EPID=$!
for i in $(seq 1 60); do curl -s -X POST http://127.0.0.1:15765 -d '{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}' | grep -q Bevy && break; sleep 1; done
sleep 100
python3 - <<PY
import sys, time
sys.path.insert(0, "$WT/tools/demo")
import record
c = record.McpClient("/tmp/${TAG}_new/${TAG}_cauldron_mcp", "$OUT/proj", 15765, "editor-png")
c.json_call("grimoire_frame_view", {"target": {"rect": [-2.6, -2.1, 2.6, 2.1]}})
time.sleep(1.5)
c.json_call("grimoire_apply_operation", {"operation": "placement_remove", "parameters": {"id": "bed"}})
time.sleep(1.0)
c.json_call("grimoire_apply_operation", {"operation": "placement_set_yaw", "parameters": {"id": "sofa", "yaw": 90}})
time.sleep(3.0)
print("operations applied")
PY
import -window root $OUT/full.png
python3 -c "from PIL import Image; im=Image.open('$OUT/full.png'); print(im.size); im.crop((96,40,96+1088,40+640)).save('$OUT/editor.png')"
ls -la $OUT/editor.png
kill $EPID 2>/dev/null; sleep 2; kill -9 $EPID 2>/dev/null
