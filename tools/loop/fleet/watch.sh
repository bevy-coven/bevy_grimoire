#!/bin/bash
# watch.sh: blocks until a fleet worker reports, then prints one report line and exits 0.
# Reads /tmp/fleet-mgr/roster.txt, one "<host|local> <cluster>" per line (agent w<cluster>, result /tmp/fleet/result-<cluster>.md).
# Every 30 s it reads each worker's FLEET-ISSUE and FLEET-RESULT lines; the first one not yet in /tmp/fleet-mgr/seen.txt is
# printed as "<host> <cluster> <line>", appended to seen.txt, and the script exits. A worker with no FLEET-RESULT whose herdr
# revision has not moved for 6 polls prints "STALL <host> <cluster>" and exits. herdr's agent_status reads idle mid-turn,
# so the revision counter (it moves with omp's spinner) is the liveness signal.
MGR=/tmp/fleet-mgr
ROSTER=$MGR/roster.txt SEEN=$MGR/seen.txt
STALL_POLLS=6 POLL_S=30
touch "$SEEN"
REVS=$MGR/revs  # one file per worker holding "<revision> <unchanged polls>"; macOS bash 3.2 has no associative arrays
mkdir -p "$REVS"
while :; do
  while read -r host cluster; do
    [ -n "$cluster" ] || continue
    if [ "$host" = local ]; then
      lines=$(grep -hE '^(FLEET-ISSUE|FLEET-RESULT)' "/tmp/fleet/result-$cluster.md" 2>/dev/null)
      H=(herdr)
    else
      lines=$(ssh -n "$host" "grep -hE '^(FLEET-ISSUE|FLEET-RESULT)' /tmp/fleet/result-$cluster.md 2>/dev/null" 2>/dev/null)
      H=(herdr --machine "$host")
    fi
    while IFS= read -r line; do
      [ -n "$line" ] || continue
      entry="$host $cluster $line"
      if ! grep -qxF "$entry" "$SEEN"; then
        printf '%s\n' "$entry" | tee -a "$SEEN"
        exit 0
      fi
    done <<<"$lines"
    printf '%s\n' "$lines" | grep -q '^FLEET-RESULT' && continue
    rev=$("${H[@]}" agent get "w$cluster" </dev/null 2>/dev/null | python3 -c 'import json,sys;print(json.load(sys.stdin)["result"]["agent"]["revision"])' 2>/dev/null)
    key="$REVS/$host-$cluster"
    prev="" count=0; [ -f "$key" ] && read -r prev count <"$key"
    if [ -n "$rev" ] && [ "$rev" = "$prev" ]; then count=$(( ${count:-0} + 1 )); else count=0; fi
    printf '%s %s\n' "$rev" "$count" >"$key"
    if [ "$count" -ge "$STALL_POLLS" ]; then
      echo "STALL $host $cluster"
      exit 0
    fi
  done <"$ROSTER"
  sleep "$POLL_S"
done
