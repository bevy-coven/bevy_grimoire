#!/bin/bash
# status.sh: every fleet worker's herdr status and FLEET-RESULT lines on each host in $FLEET_MACHINES.
for host in ${FLEET_MACHINES:-local big bmb}; do
  if [ "$host" = local ]; then H=(herdr); else H=(herdr --machine "$host"); fi
  echo "== $host"
  if out=$("${H[@]}" agent list 2>&1); then
    printf '%s' "$out" | python3 -c '
import json, sys
d = json.load(sys.stdin)
for a in d["result"]["agents"]:
    n = a.get("name") or a.get("agent") or ""
    if n.startswith("w"):
        print("  %-8s %-10s %s" % (n, a.get("agent_status", "?"), a.get("pane_id", "")))'
  else
    echo "$host: unreachable ($(printf '%s' "$out" | head -1))"
  fi
done
for host in ${FLEET_MACHINES:-local big bmb}; do
  echo "== results $host"
  if [ "$host" = local ]; then cat /tmp/fleet/result-*.md 2>/dev/null; else ssh "$host" 'cat /tmp/fleet/result-*.md 2>/dev/null' 2>/dev/null; fi | grep -h '^FLEET-RESULT'
done
true
