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
#
# One whole-suite run at a time per host: big is swap-bound (its /tmp is a 16 GB tmpfs that lives in swap), and two
# overlapping workspace builds, or one beside the push gate, are killed by systemd-oomd or the kernel (2026-11-11: the
# gate was killed seven times in a row). The lock is /tmp/fleet/big-heavy.lock; the gate runner holds it too.
set -uo pipefail
cluster=${1:?usage: predone.sh <cluster>}
if [ -z "${FLEET_HEAVY_LOCKED:-}" ] && command -v flock >/dev/null 2>&1; then
  mkdir -p /tmp/fleet
  FLEET_HEAVY_LOCKED=1 exec flock /tmp/fleet/big-heavy.lock bash "$0" "$@"
fi
# **A target dir per cluster** (owner, 2026-11-12), the same one the worker iterates in (brief rule 2): worktrees that share one
# target dir overwrite each other's same-named test binaries (nextest then ran zero tests, or a stale binary), the only cure was
# touching every source file, which forced a full workspace rebuild (~10 min, 10-12 GB) and cost the incremental cache.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/.cargo-target-fleet/$cluster}
# The same build mode as the worker's own iteration (brief rule 2), so the incremental cache is reused, not rebuilt.
export CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-1}
export RUSTC_WRAPPER=${RUSTC_WRAPPER-}
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
