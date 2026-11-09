#!/bin/bash
# launch.sh <host> <cluster> <base-ref> <brief-file>
# Opens one omp worker on DeepSeek in herdr, in its own worktree and branch fleet/<cluster>.
# <host> is "local" or a herdr machine label that is also an ssh alias.
# <cluster> is lower-case letters only; the herdr agent is w<cluster>. One worker per machine: a host with a brief
# whose result file has no FLEET-RESULT line is busy, and launch.sh exits 1 before creating anything.
set -euo pipefail
[ $# -eq 4 ] || { echo "usage: launch.sh <host> <cluster> <base-ref> <brief-file>" >&2; exit 2; }
host=$1 cluster=$2 base=$3 brief=$4
[[ $cluster =~ ^[a-z]+$ ]] || { echo "cluster must be lower-case letters only: $cluster" >&2; exit 2; }
MODEL=ollama-cloud/deepseek-v4.1-flash:1017

if [ "$host" = local ]; then
  common=$(git rev-parse --path-format=absolute --git-common-dir)
  clone=${common%/.git}
else
  clone=${FLEET_CLONE:-'$HOME/dark-incorporated/bevy_dark_factory'}
fi
worktree=/tmp/dark-incorporated/bevy_dark_factory-fleet-$cluster
branch=fleet/$cluster

remote() { if [ "$host" = local ]; then bash -s; else ssh "$host" bash -s; fi; }
remote <<SH
set -e
# A non-interactive ssh shell has a bare PATH: git-lfs (checkout filter) lives under homebrew on a Mac.
export PATH=/opt/homebrew/bin:/usr/local/bin:\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH
mkdir -p /tmp/fleet /tmp/dark-incorporated \$HOME/.cargo-target-shared/global
for b in /tmp/fleet/brief-*.md; do
  [ -e "\$b" ] || continue
  x=\${b#/tmp/fleet/brief-}; x=\${x%.md}
  grep -q '^FLEET-RESULT' "/tmp/fleet/result-\$x.md" 2>/dev/null || { echo "host busy: w\$x" >&2; exit 1; }
done
clone=$clone
[ -e "$worktree" ] && { echo "worktree exists: $worktree" >&2; exit 1; }
git -C "\$clone" fetch -q origin
# A clone may fetch only main; fleet branches (a base such as origin/fleet/integration) need their own refspec.
git -C "\$clone" fetch -q origin "+refs/heads/fleet/*:refs/remotes/origin/fleet/*"
git -C "\$clone" worktree add -q -b "$branch" "$worktree" "$base"
SH
if [ "$host" = local ]; then cp "$brief" "/tmp/fleet/brief-$cluster.md"; else scp -q "$brief" "$host:/tmp/fleet/brief-$cluster.md"; fi

if [ "$host" = local ]; then H=(herdr); else H=(herdr --machine "$host"); fi
js() { python3 -c "import json,sys; d=json.load(sys.stdin); $1"; }
hc() { local out; out=$("${H[@]}" "$@") || { echo "herdr $* failed" >&2; exit 1; }; printf '%s' "$out"; }

ws=$(hc workspace list | js 'w=[x for x in d["result"]["workspaces"] if x.get("label")=="fleet"]; print(w[0]["workspace_id"] if w else "")')
if [ -z "$ws" ]; then
  out=$(hc workspace create --cwd "$worktree" --label fleet --no-focus)
  pane=$(printf '%s' "$out" | js 'print(d["result"]["root_pane"]["pane_id"])')
  tab=$(printf '%s' "$out" | js 'print(d["result"]["tab"]["tab_id"])')
  hc tab rename "$tab" "w$cluster" >/dev/null
else
  out=$(hc tab create --workspace "$ws" --cwd "$worktree" --label "w$cluster" --no-focus)
  pane=$(printf '%s' "$out" | js 'print(d["result"]["root_pane"]["pane_id"])')
fi
# Powerlevel10k asks its configuration wizard on a host without a p10k config, which hides the prompt.
hc pane run "$pane" "exec env POWERLEVEL9K_DISABLE_CONFIGURATION_WIZARD=true zsh -l" >/dev/null
sleep 3
hc agent start "w$cluster" --kind omp --pane "$pane" --timeout 120000 -- --model "$MODEL" --thinking high >/dev/null
hc agent prompt "w$cluster" "Read /tmp/fleet/brief-$cluster.md and follow it exactly; it is your whole task." >/dev/null
# Show the worker in the Herdr app too: an hc terminal under the manager's own, attached to the agent's pane.
# The app lists hc terminals, not herdr-CLI sessions. --takeover because herdr's own pane view already holds the attach.
if command -v hc >/dev/null; then
  if [ "$host" = local ]; then attach=(herdr agent attach "w$cluster" --takeover); else attach=(ssh -t "$host" "herdr --session loops agent attach w$cluster --takeover"); fi
  hc run --bg --no-notify --name "w$cluster" -- "${attach[@]}" >/dev/null || echo "warning: no hc viewer for w$cluster" >&2
fi
echo "launched w$cluster on $host pane $pane branch $branch worktree $worktree"
