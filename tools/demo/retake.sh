#!/bin/bash
# Re-records the eight standing demos on a fresh editor each, until each passes tools/demo/check.py.
#
#   DISPLAY=:96 EDITOR_BIN=/tmp/x/bevy_cauldron MCP_BIN=/tmp/x/bevy_cauldron_mcp WORK=/tmp/x \
#     ONLY="hero build-a-map" tools/demo/retake.sh   # ONLY unset: all eight (GIF names, no .gif)
#
# Writes WORK/<name>.gif and WORK/<name>_sheet_<take>.png (look at the sheet: a passing check is
# necessary, not sufficient) and WORK/summary.txt, ending with ALLDONE. It copies the repository's
# assets into WORK/proj for every take, so a take never starts from the last one's edits.
#
# Each take kills every editor it can find by its command line first. A stray editor keeps the BRP
# port, and the next take then records the old one's state (measured 2026-11-07: nineteen editors
# were alive after a recording that took the shell's `$!` of a `nohup` wrapper as the editor's pid).
# The editor is started with `exec`, so `$!` is the editor itself.
set -u
exec </dev/null
REPO=${REPO:-$(git rev-parse --show-toplevel)}
: "${DISPLAY:?set DISPLAY to a private Xvfb, never :0}"
: "${EDITOR_BIN:?path to bevy_cauldron}" "${MCP_BIN:?path to bevy_cauldron_mcp}" "${WORK:?scratch directory}"
PORT=${PORT:-15710}
mkdir -p "$WORK/home"
SPECS=("hero hero 600" "agent_drives agent-drives 600" "agent_removes_then_undo agent-removes-then-undo 600"
       "build_a_map build-a-map 760" "frame_the_map frame-the-map 520" "move_the_camera move-the-camera 600"
       "place_and_undo place-and-undo 760" "review_labels review-labels 900")
kill_editors() { for p in $(pgrep -f "$EDITOR_BIN \\. bedroom"); do kill "$p"; done; sleep 3; }
: > "$WORK/summary.txt"
for spec in "${SPECS[@]}"; do
  set -- $spec
  if [ $# -ge 1 ] && [ -n "${ONLY:-}" ] && ! echo " $ONLY " | grep -q " $2 "; then continue; fi
  echo "== $2" >> "$WORK/summary.txt"
  ok=0
  for take in 1 2 3 4; do
    kill_editors
    rm -rf "$WORK/proj" && mkdir -p "$WORK/proj" && cp -r "$REPO/assets" "$WORK/proj/assets"
    (cd "$WORK/proj" && HOME="$WORK/home" BEVY_BRP_PORT=$PORT exec "$EDITOR_BIN" . bedroom > "$WORK/editor.log" 2>&1 < /dev/null) &
    for i in $(seq 1 60); do curl -s -X POST "http://127.0.0.1:$PORT" -d '{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}' | grep -q Bevy && break; sleep 1; done
    sleep 45   # the palette's thumbnails bake for about 40 s after the editor opens
    ( cd "$REPO" && BEVY_GRIMOIRE_MCP="$MCP_BIN" timeout 400 python3 tools/demo/record.py "tools/demo/storyboards/$1.json" --port $PORT --project "$WORK/proj" ) > "$WORK/rec_$2.log" 2>&1 \
      || { echo "take $take: record failed: $(tail -1 "$WORK/rec_$2.log")" >> "$WORK/summary.txt"; continue; }
    ev="$WORK/proj/target/loop/demo/$1/events.json"
    ( cd "$REPO" && python3 tools/demo/render.py "$ev" --out "$WORK/$2.gif" --width "$3" --sheet "$WORK/$2_sheet_$take.png" > /dev/null 2>&1 \
      && python3 tools/demo/check.py "$ev" "$WORK/$2.gif" ) > "$WORK/check_$2.txt" 2>&1
    if grep -q 'RESULT: PASS' "$WORK/check_$2.txt"; then echo "take $take: PASS" >> "$WORK/summary.txt"; ok=1; break; fi
    echo "take $take: FAIL $(grep FAIL "$WORK/check_$2.txt" | head -2 | tr '\n' ' ')" >> "$WORK/summary.txt"
  done
  [ $ok = 1 ] || echo "NOT RECORDED: $2" >> "$WORK/summary.txt"
done
kill_editors
echo ALLDONE >> "$WORK/summary.txt"
