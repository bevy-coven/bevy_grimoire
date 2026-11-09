#!/bin/bash
# waitres.sh <host|local> <issue>: returns when the worker wrote its FLEET-RESULT, or its herdr pane stopped changing for 6 polls (3 min) without one.
# herdr's agent_status reads "idle" while omp is mid-turn, so the pane's revision counter (it moves with omp's spinner) is the liveness signal.
host=$1 n=$2; still=0; last=""
if [ "$host" = local ]; then H=(herdr); else H=(herdr --machine "$host"); fi
while :; do
  if [ "$host" = local ]; then r=$(grep -h '^FLEET-RESULT' /tmp/fleet/result-$n.md 2>/dev/null | tail -1); else r=$(ssh "$host" "grep -h '^FLEET-RESULT' /tmp/fleet/result-$n.md 2>/dev/null | tail -1" 2>/dev/null); fi
  [ -n "$r" ] && { echo "RESULT: $r"; exit 0; }
  rev=$("${H[@]}" agent get w$n 2>/dev/null | python3 -c 'import json,sys;print(json.load(sys.stdin)["result"]["agent"]["revision"])' 2>/dev/null)
  if [ -n "$rev" ] && [ "$rev" = "$last" ]; then still=$((still+1)); else still=0; fi
  last=$rev
  [ $still -ge 6 ] && { echo "STILL-NO-RESULT w$n (pane revision $rev unchanged for 3 min)"; exit 0; }
  sleep 30
done
