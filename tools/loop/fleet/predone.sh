#!/usr/bin/env bash
# One command a fleet worker runs before it writes a FLEET-ISSUE DONE line: the push gate's own checks, from the
# worker's worktree, with the result recorded in the result file. A worker that only ever ran its own tests left the
# fast static protected tests (no engine in a source file, every resource classified, ...) to fail at the gate:
# 13 of the 22 tests that refused pushes in the 2026-11-09..11 fleet run (docs/loop/iterations/2026-11-11-fleet-timing.md).
#
#   bash tools/loop/fleet/predone.sh <cluster>
#
# Appends `FLEET-PREDONE <cluster> PASS|FAIL <sha>` to /tmp/fleet/result-<cluster>.md and exits with the suite's code.
# The timing tests (a UI frame budget, a slow-load spinner) are wall-clock and fail on a loaded host; a FAIL naming only
# those is rerun alone once before it counts.
set -uo pipefail
cluster=${1:?usage: predone.sh <cluster>}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/.cargo-target-shared/global}
result=/tmp/fleet/result-$cluster.md
log=/tmp/fleet/predone-$cluster.log
sha=$(git rev-parse --short=9 HEAD)
{
  cargo clippy --workspace --all-targets -q -- -D warnings &&
    cargo nextest run --workspace --no-fail-fast -E 'not test(terminal)'
} >"$log" 2>&1
rc=$?
grep -E 'FAIL \[|Summary|^error' "$log" | head -40
if [ $rc -eq 0 ]; then verdict=PASS; else verdict=FAIL; fi
echo "FLEET-PREDONE $cluster $verdict $sha" >>"$result"
exit $rc
