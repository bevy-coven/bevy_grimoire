#!/bin/bash
# launch.sh <host> <issue> <slug> <base-ref> <brief-file>
# Opens one omp worker on DeepSeek in herdr, in its own worktree and branch fleet/<issue>-<slug>.
# <host> is "local" or a herdr machine label that is also an ssh alias.
set -euo pipefail
[ $# -eq 5 ] || { echo "usage: launch.sh <host> <issue> <slug> <base-ref> <brief-file>" >&2; exit 2; }
host=$1 issue=$2 slug=$3 base=$4 brief=$5
MODEL=ollama-cloud/deepseek-v4.1-flash:1017

if [ "$host" = local ]; then
  common=$(git rev-parse --path-format=absolute --git-common-dir)
  clone=${common%/.git}
else
  clone=${FLEET_CLONE:-'$HOME/dark-incorporated/bevy_dark_factory'}
fi
worktree=/tmp/dark-incorporated/bevy_dark_factory-fleet-$issue
branch=fleet/$issue-$slug

remote() { if [ "$host" = local ]; then bash -s; else ssh "$host" bash -s; fi; }
remote <<SH
set -e
# A non-interactive ssh shell has a bare PATH: git-lfs (checkout filter) lives under homebrew on a Mac.
export PATH=/opt/homebrew/bin:/usr/local/bin:\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH
mkdir -p /tmp/fleet /tmp/dark-incorporated
clone=$clone
[ -e "$worktree" ] && { echo "worktree exists: $worktree" >&2; exit 1; }
git -C "\$clone" fetch -q origin
git -C "\$clone" worktree add -b "$branch" "$worktree" "$base"
SH
if [ "$host" = local ]; then cp "$brief" "/tmp/fleet/brief-$issue.md"; else scp -q "$brief" "$host:/tmp/fleet/brief-$issue.md"; fi

if [ "$host" = local ]; then H=(herdr); else H=(herdr --machine "$host"); fi
js() { python3 -c "import json,sys; d=json.load(sys.stdin); $1"; }
hc() { local out; out=$("${H[@]}" "$@") || { echo "herdr $* failed" >&2; exit 1; }; printf '%s' "$out"; }

ws=$(hc workspace list | js 'w=[x for x in d["result"]["workspaces"] if x.get("label")=="fleet"]; print(w[0]["workspace_id"] if w else "")')
if [ -z "$ws" ]; then
  out=$(hc workspace create --cwd "$worktree" --label fleet --no-focus)
  pane=$(printf '%s' "$out" | js 'print(d["result"]["root_pane"]["pane_id"])')
  tab=$(printf '%s' "$out" | js 'print(d["result"]["tab"]["tab_id"])')
  hc tab rename "$tab" "w$issue" >/dev/null
else
  out=$(hc tab create --workspace "$ws" --cwd "$worktree" --label "w$issue" --no-focus)
  pane=$(printf '%s' "$out" | js 'print(d["result"]["root_pane"]["pane_id"])')
fi
# Powerlevel10k asks its configuration wizard on a host without a p10k config, which hides the prompt.
hc pane run "$pane" "exec env POWERLEVEL9K_DISABLE_CONFIGURATION_WIZARD=true zsh -l" >/dev/null
sleep 3
hc agent start "w$issue" --kind omp --pane "$pane" --timeout 120000 -- --model "$MODEL" --thinking high >/dev/null
hc agent prompt "w$issue" "Read /tmp/fleet/brief-$issue.md and follow it exactly; it is your whole task." >/dev/null
echo "launched w$issue on $host pane $pane branch $branch worktree $worktree"
