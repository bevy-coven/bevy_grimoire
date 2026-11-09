#!/bin/bash
# waitres.sh <host|local> <issue>: returns when the worker wrote its FLEET-RESULT, or sat idle 6 polls (3 min) in a row without one.
host=$1 n=$2; idle=0
if [ "$host" = local ]; then H=(herdr); else H=(herdr --machine "$host"); fi
while :; do
  if [ "$host" = local ]; then r=$(grep -h '^FLEET-RESULT' /tmp/fleet/result-$n.md 2>/dev/null | tail -1); else r=$(ssh "$host" "grep -h '^FLEET-RESULT' /tmp/fleet/result-$n.md 2>/dev/null | tail -1" 2>/dev/null); fi
  [ -n "$r" ] && { echo "RESULT: $r"; exit 0; }
  st=$("${H[@]}" agent get w$n 2>/dev/null | python3 -c 'import json,sys;print(json.load(sys.stdin)["result"]["agent"]["agent_status"])' 2>/dev/null)
  if [ "$st" = idle ] || [ "$st" = blocked ]; then idle=$((idle+1)); else idle=0; fi
  [ "$st" = blocked ] && { echo "BLOCKED-UI w$n"; exit 0; }
  [ $idle -ge 6 ] && { echo "IDLE-NO-RESULT w$n"; exit 0; }
  sleep 30
done
