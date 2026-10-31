#!/bin/bash
# **The Wiggum loop: run `build-loop` forever, one fresh context per pass.**
#
# Dumb on purpose. It starts an `omp` process with the command, waits for it to exit, sleeps a
# little, and starts another. It holds no state, makes no decisions, and never inspects the result —
# everything a pass learns and everything it must not forget lives in the repository, so a pass that
# dies mid-thought is repaired by the next one rather than by this script.
#
# **Why not a smart driver?** Because the smart driver is a second agent with its own context, its
# own bugs, and a second opinion about when the work is done. The research is unambiguous that the
# evaluator must not be part of the loop; a driver that decides "that pass looked good, skip ahead"
# is exactly that. This one only counts.
#
# Usage, on big:
#   ~/wiggum.sh start          # start in the background, detached from the shell
#   ~/wiggum.sh status         # is it running, how many passes, what is the last log line
#   ~/wiggum.sh stop           # end the pass in flight (TERM, then KILL after 5 s) and exit; its tree is parked
#   ~/wiggum.sh drain [--wait] # let the pass in flight FINISH, then exit; --wait blocks until it has. Cuts nothing
#   ~/wiggum.sh tail           # follow the driver's own log
#   ~/wiggum.sh one            # run exactly one pass in the foreground, for a smoke test
#   ~/wiggum.sh guard          # exit 1 if a pass is running — call before reset/checkout/clean
#   ~/wiggum.sh settings       # what the repo's own settings file sets and what the environment overrides, before anything runs
#
# Environment, all optional:
#   WIGGUM_MODEL   default claude-sonnet-5-5, which every loop has run since 2026-10-27 (ollama-cloud answered 402 and 429)
#   WIGGUM_REPO    default $HOME/bevy_emerge — any git clone
#   WIGGUM_COMMAND slash command a pass runs, default build-loop
#   WIGGUM_STATE   where the driver keeps its logs, default ~/.local/state/wiggum/<repo>
#   WIGGUM_SLEEP   seconds between passes, default 60
#   WIGGUM_MAX     stop after this many passes, default 0 (forever)
#   WIGGUM_THINK   thinking level, default medium
#   WIGGUM_PASS_TIMEOUT  hard ceiling on one pass in seconds, default 4500 (75 min)
#   WIGGUM_CHECKPOINT    seconds between mid-pass snapshots, default 600 (0 disables)
#   WIGGUM_MIN_FREE_GB   refuse to start a pass with less free space, default 150
#   WIGGUM_IDLE_UNDER    a pass that ends rc=0 in under this many seconds did no work, default 120
#   WIGGUM_IDLE_MAX      longest sleep after repeated idle passes, default 3600 (0 disables backoff)
#   WIGGUM_PARK          0 turns off parking a leftover tree on a branch before each pass, default 1
#   WIGGUM_BRANCH        the branch a clone returns to, default origin's HEAD, else main or master
#   WIGGUM_PUSH_MAX_MB   a snapshot or parked branch adding more than this many MB of new data stays local, default 10
#   WIGGUM_MAX_DIRTY     most modified files a tree may have when a pass starts, default 40; a checkout people work in needs more
#   WIGGUM_SNAPSHOT      0 writes no refs/wiggum snapshot, mid-pass or final (a checkout other people work in), default 1
#
# A repo may carry its own settings in `<repo>/.omp/wiggum/<command>.env`: NAME=VALUE lines for the WIGGUM_ knobs above
# (not REPO, COMMAND or STATE), `#` comments in column 1. The environment still wins over the file. A line the driver
# cannot use stops it with exit 2, because a safety setting dropped without a word puts its default back. `settings` shows it.
# `<repo>/.omp/wiggum/<command>.omp.yml`, when present, is passed to omp as `--config` for this loop's passes only (an omp
# settings overlay: model fallback chains, retry budget).

set -uo pipefail

REPO="${WIGGUM_REPO:-$HOME/bevy_emerge}"
# **The command this repo's passes run: `build-loop`, in every loop repo.** The owner's rule since 2026-10-28
# is that a loop's prompt is `.omp/commands/build-loop.md` wherever it lives, so there is one name to know.
# This driver is about *how* a loop runs (one fresh pass, stop between passes, never mid-edit), not about
# what a pass does; set `WIGGUM_COMMAND` only for a repo that has no such file (then `bevy-loop`, the generic one).
COMMAND="${WIGGUM_COMMAND:-build-loop}"

# **A repo carries its own settings, in `.omp/wiggum/<command>.env`.** Found 2026-10-28 reading the code for
# Sanctuary's grimoire loop, whose start line carried no flags: the first pass would have run `git add -A` over
# the 29 files other agents were editing in that checkout, committed them to `wiggum/parked-pass-0-…` and
# checked `main` out under them, all before the free-space gate got to refuse the pass. A checkout people work
# in needs `WIGGUM_PARK=0`, `WIGGUM_CHECKPOINT=0` and a raised `WIGGUM_MAX_DIRTY` on every start, and a start
# line is a poor place to keep them: whoever types it has to remember. The repo that needs them says so in a
# file this driver reads. **The environment wins**, so one run can still override one setting. **A line the
# driver cannot use stops it** (`start`, `one` and `__driver` exit 2): the likeliest typo, `WIGGUM_PARK = 0`,
# would otherwise be dropped without a word and put the default back. Only the knobs in the header may be set
# (not REPO, COMMAND or STATE: the file is found through them), and values are plain words, so a file can
# never run anything. It is read once, when the driver starts, so a pass, which can edit the tree, cannot
# change the settings of the loop that is running it.
SETTINGS_FILE="$REPO/.omp/wiggum/$COMMAND.env"
OMP_OVERLAY="$REPO/.omp/wiggum/$COMMAND.omp.yml"
SETTINGS_KNOBS=" WIGGUM_MODEL WIGGUM_SLEEP WIGGUM_MAX WIGGUM_THINK WIGGUM_PASS_TIMEOUT WIGGUM_CHECKPOINT WIGGUM_MIN_FREE_GB WIGGUM_IDLE_UNDER WIGGUM_IDLE_MAX WIGGUM_PARK WIGGUM_BRANCH WIGGUM_PUSH_MAX_MB WIGGUM_MAX_DIRTY WIGGUM_NO_FETCH WIGGUM_SNAPSHOT "
SETTINGS_NL=$'\n'
SETTINGS_ACCEPTED=""   # NAME=VALUE, one per line, in file order
SETTINGS_REJECTED=""   # one line each: where, and why

reject_setting() { SETTINGS_REJECTED="$SETTINGS_REJECTED$SETTINGS_NL$1"; }

load_settings() {
  [ -f "$SETTINGS_FILE" ] || return 0
  local line name value n=0
  while IFS= read -r line || [ -n "$line" ]; do
    n=$((n + 1))
    case "$line" in '' | '#'*) continue ;; esac
    name=${line%%=*}
    value=${line#*=}
    if [ "$name" = "$line" ]; then reject_setting "line $n: not NAME=VALUE: $line"; continue; fi
    case "$SETTINGS_KNOBS" in
      *" $name "*) ;;
      *) reject_setting "line $n: '$name' is not a setting a file may carry"; continue ;;
    esac
    case "$value" in
      '' | *[!A-Za-z0-9_./:@%+-]*) reject_setting "line $n: $name has an empty value or characters outside A-Za-z0-9_./:@%+-"; continue ;;
    esac
    case "$SETTINGS_ACCEPTED" in
      *"$SETTINGS_NL$name="*) reject_setting "line $n: $name is set twice"; continue ;;
    esac
    SETTINGS_ACCEPTED="$SETTINGS_ACCEPTED$SETTINGS_NL$name=$value"
    eval "[ -n \"\${$name+x}\" ]" || export "$name=$value"
  done <"$SETTINGS_FILE"
}

print_settings() {
  local rel="${SETTINGS_FILE#"$REPO"/}" pair name value eff
  if [ -f "$OMP_OVERLAY" ]; then
    echo "omp overlay: ${OMP_OVERLAY#"$REPO"/} (passed to omp with --config)"
  else
    echo "omp overlay: none"
  fi
  if [ ! -f "$SETTINGS_FILE" ]; then
    echo "settings file: $rel (none: every setting is the environment's or the default in this script's header)"
    return 0
  fi
  echo "settings file: $rel"
  while IFS= read -r pair; do
    [ -z "$pair" ] && continue
    name=${pair%%=*}; value=${pair#*=}
    eff=$(eval "printf %s \"\${$name-}\"")
    if [ "$eff" = "$value" ]; then
      echo "  $name=$value   (from the file)"
    else
      echo "  $name=$eff   (the environment overrides the file's $value)"
    fi
  done <<EOF
$SETTINGS_ACCEPTED
EOF
  if [ -n "$SETTINGS_REJECTED" ]; then
    echo "REJECTED (the driver will not run with these):"
    printf '%s\n' "$SETTINGS_REJECTED" | sed '/^$/d; s/^/  /'
  fi
  return 0
}

refuse_bad_settings() {
  [ -z "$SETTINGS_REJECTED" ] && return 0
  echo "refusing to run: ${SETTINGS_FILE#"$REPO"/} has lines the driver cannot use, and a safety setting dropped without a word puts its default back:" >&2
  printf '%s\n' "$SETTINGS_REJECTED" | sed '/^$/d; s/^/  /' >&2
  exit 2
}

load_settings

MODEL="${WIGGUM_MODEL:-claude-sonnet-5-5}"
SLEEP_BETWEEN="${WIGGUM_SLEEP:-60}"
MAX_PASSES="${WIGGUM_MAX:-0}"
THINKING="${WIGGUM_THINK:-medium}"
# **A hard ceiling on one pass, in seconds, default 75 minutes.** The command tells a pass its budget is
# 45 minutes and that running long is worse than shipping nothing — but a command is advice, and a model
# that has lost track of time will keep going. Measured: the first real pass ran past an hour with work
# still in flight. This is the enforcement: past the ceiling the pass process is terminated, which is a
# non-zero return, so the driver counts it as a failure (five in a row stops the loop) and the next pass
# finds the tree exactly as the killed pass left it. **Kill only the pass, never with `-f` on a pattern**
# — the shell that launched it would match too.
PASS_TIMEOUT="${WIGGUM_PASS_TIMEOUT:-4500}"
# **How often the driver commits the tree while a pass is still running.** Default 600 s. A pass that
# is killed cannot commit, and a cgroup kill takes the driver with it, so the interval bounds how much
# a death can cost: at most this much work, never the whole pass. Set 0 to disable.
CHECKPOINT_EVERY="${WIGGUM_CHECKPOINT:-600}"

# **A pass that ends cleanly in under two minutes did no work, and the driver backs off from it.**
# Measured 2026-10-27: bevy_lights logged 50 consecutive passes of about 20 s each (iterations 61 to
# 110), every one a refusal to build on a tree it had not made, and bevy_grimoire ran 25 passes of about 26 s
# whose only output was a log line, 19 of them pushed to `main` as commits. Each was a full model call
# every 90 seconds, so a loop with nothing it may do cost the same as one working. A real pass takes
# minutes (the shortest real fix seen, grimoire's issue #83, took 5), so `rc=0` under `IDLE_UNDER` seconds
# is idle, and each consecutive idle pass doubles the sleep up to `IDLE_MAX`. A pass that takes longer,
# or fails, resets it; `WIGGUM_IDLE_MAX=0` turns it off. It does not ask *why* a pass idled: a stranded
# tree, a blocked backlog and a missing tool all look the same from here, and all deserve the same answer.
IDLE_UNDER="${WIGGUM_IDLE_UNDER:-120}"
IDLE_MAX="${WIGGUM_IDLE_MAX:-3600}"

# **State lives outside the repository.** A loop's own bookkeeping must never appear in a pass's
# `git status` — a dirty tree is the first thing a pass is told to fix, and a driver writing logs into the
# tree would make every pass start by diagnosing itself. `target/` is gitignored, but a repo without that
# rule, or a pass that runs `git clean`, would lose the logs. `XDG_STATE_HOME` is the right home.
STATE="${WIGGUM_STATE:-${XDG_STATE_HOME:-$HOME/.local/state}/wiggum/$(basename "$REPO")}"
PIDFILE="$STATE/driver.pid"
COUNTER="$STATE/pass-count"
DRIVER_LOG="$STATE/driver.log"
PASS_LOG_DIR="$STATE/passes"
STOPFILE="$STATE/stop"
DRAINFILE="$STATE/drain"
MARKER="wiggum-$USER-$(basename "$REPO")"

# **The state directory is created only once the repo is known to be a clone.** A driver pointed at a
# scratch tree used to `mkdir` inside it before checking, which is the one write this script must never
# make into somebody else's directory.
ensure_state() { mkdir -p "$STATE" "$PASS_LOG_DIR"; }

log() { printf '%s %s\n' "$(date '+%Y-%m-%dT%H:%M:%S')" "$*" >>"$DRIVER_LOG"; }

running() {
  [ -f "$PIDFILE" ] || return 1
  local pid; pid=$(cat "$PIDFILE" 2>/dev/null) || return 1
  [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

# **A second driver on one checkout, found in the process table.** The pidfile alone cannot answer this
# for the case the unit creates: `ExecStart=… __driver` is the driver, so a stale pidfile or a driver
# started by hand and a unit-owned one both claim the same tree, and two `omp` passes interleaving in one
# checkout is the expensive failure this script exists to prevent.
#
# **Measured 2026-10-26: the marker was in the environment, and `pgrep -f` reads argv.** `env
# WIGGUM_MARKER=… bash "$0" __driver` never puts the marker in the child's command line, so the guard
# matched nothing and two drivers ran together — passes numbered 1, 2, 3 out of order in one
# `driver.log`. The marker is now an argv token on the re-exec, which is where `pgrep -f` looks.
#
# **This driver matches its own argv, and so does every subshell it forks** — bash children inherit the
# command line, and a `$( … )` running this function can still be alive when it asks. So the guard
# counts only a marker process that does **not** descend from this driver.
other_driver_running() {
  local pid p
  for pid in $(pgrep -f -- "$MARKER" 2>/dev/null); do
    # **Walk up from the candidate.** Any marker process that descends from this driver — a `$( … )`
    # subshell, a `pipeline` stage — is this driver, not another one. Bounded by the tree's depth.
    p=$pid
    while [ -n "$p" ] && [ "$p" != 0 ] && [ "$p" != 1 ]; do
      [ "$p" = "$$" ] && { p=""; break; }
      p=$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')
    done
    # `p` is empty when the walk reached this driver, so the candidate is ours and is skipped.
    [ -z "$p" ] && continue
    return 0
  done
  return 1
}

# **How much new data a commit would publish: the blobs it adds beyond its first parent.** The figure the push
# guards compare with `WIGGUM_PUSH_MAX_MB` (default 10). Measured 2026-10-27: the park step pushed a branch of
# 82 MB to a **public** repository, six raw `series.csv` of about 14 MB each swept up by `git add -A`, and the
# older `snapshot()` had already pushed the same blobs to hidden `refs/wiggum/*` refs 51 times: the owner had
# ruled raw run output out of git an hour before. A leftover tree's source is small (elements' 11 files were
# 0.45 MB), so a push over ten megabytes is data or media, never work worth publishing; the commit and the ref
# stay local, and big's own copy is the backup. Measured on the commit's own additions, not on file sizes, so
# content already in history is free and a branch that was pushed before is not charged for it again.
commit_adds_bytes() {
  local c="$1" base=""
  git -C "$REPO" rev-parse -q --verify "$c^" >/dev/null 2>&1 && base="$c^"
  git -C "$REPO" rev-list --objects "$c" ${base:+--not "$base"} 2>/dev/null | awk '{print $1}' \
    | git -C "$REPO" cat-file --batch-check='%(objecttype) %(objectsize)' 2>/dev/null \
    | awk '$1=="blob" {s+=$2} END {print s+0}'
}

# **Save the pass's tree without touching the pass's branch, its index or its HEAD.**
#
# Measured 2026-10-26: committing the tree to the pass's own branch and pushing it put ungated work on
# `origin/main` as the branch tip in all four shared repos (lights `35c7824`, society `89d9563` — a
# mid-pass checkpoint —, elements `911ca5b`). The loop commands build a `loop/iter-N` branch from
# `main` and merge back only after their gate, so a commit written straight onto `main` bypasses the
# gate entirely, and Sanctuary path-deps `bevy_grimoire_core`. A checkpoint must not be a commit on a
# branch anybody builds from.
#
# So the tree is written to **an object under `refs/wiggum/`**, which is not a branch: `git branch`,
# `git log`, and the commands' own "look for a `WIP(pass N)` commit" rule are all unaffected. It is
# still an object git will not garbage-collect, and `git show refs/wiggum/...` recovers the work.
#
# **`GIT_INDEX_FILE` is what keeps the pass's view clean.** `git stash create` would move the real
# index and leave the pass's `git status --short` empty mid-work, which breaks the loop command's own
# rule that the tree must be empty before its commit step; a private index file mutates nothing.
#
# **No force, no rebase, no branch move.** Nothing here can rewrite anything, and `-p HEAD` keeps the
# snapshot reachable from the branch even if the ref is lost. Never fails the pass: every failure path
# returns 0 and says so in the log.
snapshot() {
  [ "${WIGGUM_SNAPSHOT:-1}" = "0" ] && return 0
  local n="$1" tag="$2"            # tag: "checkpoint" | "final"
  local tmpidx; tmpidx=$(mktemp -t wiggum-idx.XXXXXX) || return 0
  rm -f "$tmpidx"
  GIT_INDEX_FILE="$tmpidx" GIT_WORK_TREE="$REPO" git -C "$REPO" add -A 2>/dev/null || { rm -f "$tmpidx"; return 0; }
  local tree; tree=$(GIT_INDEX_FILE="$tmpidx" git -C "$REPO" write-tree 2>/dev/null)
  rm -f "$tmpidx"
  [ -n "$tree" ] || return 0
  # **An empty tree is not a snapshot.** A pass that only touched ignored files would otherwise write
  # a ref whose content is exactly the parent's, once per interval, for hours.
  local parent; parent=$(git -C "$REPO" rev-parse HEAD 2>/dev/null)
  if [ -n "$parent" ] && [ "$tree" = "$(git -C "$REPO" rev-parse "$parent^{tree}" 2>/dev/null)" ]; then
    return 0
  fi
  local ref="refs/wiggum/pass-$n-$tag-$(date '+%Y%m%d-%H%M%S')"
  local msg="WIP(pass $n, $tag): tree saved by the driver, never committed to a branch

Not gated, not on any branch, and not seen by the pass: the index and HEAD are untouched.
Recover with: git -C $REPO show $ref"
  local commit
  commit=$(git -C "$REPO" -c user.email=wiggum@loop.local -c user.name="wiggum loop" \
             commit-tree "$tree" ${parent:+-p "$parent"} -m "$msg" 2>/dev/null)
  [ -n "$commit" ] || return 0
  git -C "$REPO" update-ref "$ref" "$commit" 2>/dev/null || return 0
  local added; added=$(commit_adds_bytes "$commit")
  if [ "${added:-0}" -gt $(( ${WIGGUM_PUSH_MAX_MB:-10} * 1048576 )) ]; then
    log "pass $n $tag SNAPSHOT $ref (LOCAL ONLY — $(( added / 1048576 )) MB of new data is over WIGGUM_PUSH_MAX_MB, not pushed)"
    return 0
  fi
  git -C "$REPO" push -q origin "$ref:$ref" 2>/dev/null \
    && log "pass $n $tag SNAPSHOT $ref (pushed)" \
    || log "pass $n $tag SNAPSHOT $ref (LOCAL ONLY — push failed)"
}

# **The branch a clone returns to.** `WIGGUM_BRANCH` wins; otherwise what origin calls its default, and
# `main` or `master` when origin does not say (a clone made by `git init` and `fetch` has no `origin/HEAD`).
default_branch() {
  local b="${WIGGUM_BRANCH:-}" c
  if [ -z "$b" ]; then
    b=$(git -C "$REPO" symbolic-ref --short -q refs/remotes/origin/HEAD 2>/dev/null || true)
    b="${b#origin/}"
  fi
  if [ -z "$b" ]; then
    for c in main master; do
      if git -C "$REPO" show-ref --verify -q "refs/heads/$c"; then b="$c"; break; fi
    done
  fi
  printf '%s' "$b"
}

# **A tree a pass left behind is parked on a branch, and the clone goes back to `main`, before the next
# pass starts.** Measured 2026-10-27: bevy_lights logged 61 passes that each refused a tree they had not
# made and stopped in about 20 s — 50 for a prompt edit left uncommitted, 11 for a stranded `loop/iter-115`
# holding 412 lines of ungated shader work — and each one was a model call. Both were cleared by hand the
# same way: commit the work to a branch, push the branch, put the clone back on `main`. A baseline
# worktree that had `main` checked out blocked that last step, so it is handled too: a clean holder is
# detached, a holder with changes in it is not touched and the pass is refused.
#
# **It never writes to `main` and never pushes it.** Dirty on `main` cuts a `wiggum/parked-pass-N-…`
# branch from HEAD and commits there; dirty on any other branch commits on that branch; a detached HEAD
# whose commit no branch holds gets a branch cut for it. That is the lesson of `snapshot()`: ungated work
# must not land on a branch anybody builds from. **The commit is titled `WIP(pass N, parked)`, N being the
# pass that left the work**, because that is what every loop command tells a pass to search for
# (`git log --all --grep='WIP(pass'`), so a pass that goes looking for a killed pass's work finds a parked
# tree the way it finds a snapshot. Nothing else announces it: the next pass starts clean on `main`.
#
# **What it will not touch:** a clone in the middle of a merge, rebase, revert or cherry-pick (somebody is
# mid-decision), and more than 40 changed files (a scratch tree; that check comes first). Each of those
# refuses the pass with the reason, which counts as a failure, so five in a row stop the loop. **The
# contract it leans on** is the one `guard` states: nothing but the driver writes to a loop's clone. A
# person who edits the tree between passes has that edit parked, not lost. `WIGGUM_PARK=0` turns it off;
# a leftover that adds more than `WIGGUM_PUSH_MAX_MB` of data is committed but kept off origin.
park_stranded_tree() {
  local n="$1"                        # the pass about to start
  local left=$(( n - 1 ))             # the pass that left the work: the one the branch and commit are named for
  [ "${WIGGUM_PARK:-1}" = "0" ] && return 0
  ensure_state

  local gd st
  gd=$(git -C "$REPO" rev-parse --absolute-git-dir 2>/dev/null) || return 0
  for st in MERGE_HEAD CHERRY_PICK_HEAD REVERT_HEAD rebase-merge rebase-apply; do
    if [ -e "$gd/$st" ]; then
      log "pass $n ABORTED: $REPO is in the middle of $st — finish or abort it by hand"
      echo "ABORTED: $REPO is in the middle of $st — finish or abort it by hand." >&2
      return 1
    fi
  done

  local home_br cur dirty
  home_br=$(default_branch)
  if [ -z "$home_br" ]; then
    log "pre-pass: WARNING no default branch found (set WIGGUM_BRANCH) — nothing parked"
    return 0
  fi
  cur=$(git -C "$REPO" symbolic-ref --short -q HEAD 2>/dev/null || true)     # empty when HEAD is detached
  dirty=$(git -C "$REPO" status --short 2>/dev/null | wc -l | tr -d ' ')
  if [ "$cur" = "$home_br" ] && [ "${dirty:-0}" -eq 0 ]; then
    return 0                                                                  # the ordinary case
  fi

  local was="${cur:-detached at $(git -C "$REPO" rev-parse --short HEAD 2>/dev/null)}"
  local loose=0
  # `git branch --contains` lists the detached HEAD itself as an entry, so its output is never empty;
  # `for-each-ref` names real refs only. Measured by the P5 test: with `branch`, a commit left on a
  # detached HEAD was judged held, the clone moved to `main`, and the commit lost its only name.
  if [ -z "$cur" ] && [ -z "$(git -C "$REPO" for-each-ref --contains HEAD --format='%(refname)' refs/heads refs/remotes refs/tags 2>/dev/null)" ]; then
    loose=1                                                                   # a commit nothing else holds
  fi

  local branch="$cur" parked_sha="" bytes=0 listing
  if [ "${dirty:-0}" -gt 0 ] || [ "$loose" -eq 1 ]; then
    if [ -z "$cur" ] || [ "$cur" = "$home_br" ]; then
      branch="wiggum/parked-pass-$left-$(date '+%Y%m%d-%H%M%S')"
      if ! git -C "$REPO" checkout -q -b "$branch" 2>/dev/null; then
        log "pass $n ABORTED: could not cut $branch to park work on"
        echo "ABORTED: could not cut $branch to park work on." >&2
        return 1
      fi
    fi
    if [ "${dirty:-0}" -gt 0 ]; then
      listing=$(git -C "$REPO" status --short 2>/dev/null | head -20)
      git -C "$REPO" add -A >/dev/null 2>&1
      if ! git -C "$REPO" -c user.name=wiggum -c user.email=wiggum@localhost -c commit.gpgsign=false \
           commit -q --no-verify \
           -m "WIP(pass $left, parked): $dirty file(s) left behind, parked by the driver before pass $n" \
           -m "Ungated: never built or tested. It was on $was when pass $n started, so the driver moved it here and put the clone back on $home_br. Recover it as any killed pass's work: git log --all --grep='WIP(pass'." \
           -m "$listing" >/dev/null 2>&1; then
        log "pass $n ABORTED: could not commit the $dirty leftover file(s) on $branch"
        echo "ABORTED: could not commit the leftover files on $branch." >&2
        return 1
      fi
    fi
    parked_sha=$(git -C "$REPO" rev-parse --short HEAD)
    [ "${dirty:-0}" -gt 0 ] && bytes=$(commit_adds_bytes HEAD)
  fi

  local pushed="n/a" note=""
  if [ -n "$branch" ] && [ "$branch" != "$home_br" ]; then
    if [ "$bytes" -gt $(( ${WIGGUM_PUSH_MAX_MB:-10} * 1048576 )) ]; then
      pushed=no; note=" ($(( bytes / 1048576 )) MB of new data is over WIGGUM_PUSH_MAX_MB, kept local)"
    elif git -C "$REPO" push -q origin "$branch" 2>/dev/null; then
      pushed=yes
    else
      pushed=no; note=" (push failed, kept local)"
    fi
  fi

  local err holder
  if ! err=$(git -C "$REPO" checkout -q "$home_br" 2>&1); then
    holder=$(git -C "$REPO" worktree list --porcelain 2>/dev/null \
      | awk -v b="refs/heads/$home_br" '/^worktree /{p=substr($0,10)} $1=="branch" && $2==b {print p}')
    if [ -n "$holder" ] && [ "$holder" != "$REPO" ] \
       && [ -z "$(git -C "$holder" status --short 2>/dev/null)" ] \
       && git -C "$holder" checkout -q --detach 2>/dev/null; then
      log "pass $n pre: detached $holder, a worktree that had $home_br checked out"
      err=$(git -C "$REPO" checkout -q "$home_br" 2>&1) && err=""
    fi
    if [ -n "$err" ]; then
      log "pass $n ABORTED: could not return to $home_br ($(printf '%s' "$err" | head -1 | cut -c1-140))"
      echo "ABORTED: could not return to $home_br: $err" >&2
      return 1
    fi
  fi

  if [ "${dirty:-0}" -gt 0 ]; then
    log "pass $n pre: parked $dirty file(s) on $branch ($parked_sha), pushed: $pushed$note; clone is back on $home_br"
  elif [ "$loose" -eq 1 ]; then
    log "pass $n pre: kept the detached commit $parked_sha on $branch, pushed: $pushed$note; clone is back on $home_br"
  else
    log "pass $n pre: clone was on $was, not $home_br; back on $home_br (branch $branch pushed: $pushed$note)"
  fi
  return 0
}

# **One pass.** The whole of what this script does to the world.
one_pass() {
  # **Both preconditions come before the first write, and that order is the point.** `one` runs this
  # in the foreground, so a pass that wrote its own counter into somebody's scratch tree before
  # checking would be the script doing the thing it exists to refuse. Measured on big: the first
  # version wrote `pass-count` and `driver.log` into the repo before deciding it was not a clone.
  if [ ! -d "$REPO/.git" ]; then
    echo "ABORTED: $REPO is not a git clone" >&2
    return 1
  fi
  # **Warn before a pass whose command may publish.** The loop command in `bevy_debugger_mcp` says
  # "every iteration ends pushed and **published**", and a publish cannot be undone: crates.io has no
  # unpublish and a yanked version is burned. A driver cannot read a command's intent, so it states what
  # it can see and lets the operator decide. A warning, not a block — only a human knows whether the
  # command in *their* repo publishes.
  local cmd_file="$REPO/.omp/commands/$COMMAND.md"
  if [ -f "$cmd_file" ] && grep -qiE 'cargo publish|ends pushed and published' "$cmd_file"; then
    echo "WARNING: /$COMMAND may publish to crates.io — irreversible, and a loop cannot ask." >&2
  fi

  local dirty; dirty=$(git -C "$REPO" status --short 2>/dev/null | wc -l | tr -d ' ')
  # **A checkout that people and other agents work in at the same time is never under 40 files for long**
  # (Sanctuary's had 34 on 2026-10-28, with six untracked .blend files). Such a loop needs `WIGGUM_MAX_DIRTY`
  # raised, and `WIGGUM_PARK=0` and `WIGGUM_CHECKPOINT=0`: with the defaults the driver would commit everyone's
  # unfinished files to a parked branch and return the checkout to `main` under them.
  local max_dirty="${WIGGUM_MAX_DIRTY:-40}"
  if [ "${dirty:-0}" -gt "$max_dirty" ]; then
    echo "ABORTED: $REPO has $dirty modified files (WIGGUM_MAX_DIRTY=$max_dirty) — a scratch tree, not a clone. Fix it by hand." >&2
    return 1
  fi

  # **A tree the last pass left behind is parked, and the clone goes back to `main`, before this pass
  # starts.** See `park_stranded_tree`. It runs before the fast-forward below because that only happens
  # on a clean tree, which is exactly what parking leaves.
  if ! park_stranded_tree "$(( $(cat "$COUNTER" 2>/dev/null || echo 0) + 1 ))"; then
    return 1
  fi
  dirty=$(git -C "$REPO" status --short 2>/dev/null | wc -l | tr -d ' ')

  # **A clean clone fast-forwards to origin before the pass starts.** (A leftover tree has just been
  # parked above, so it is clean here; with `WIGGUM_PARK=0` a dirty one is left alone and not updated.)
  # Measured 2026-10-26: a human session pushed iterations 11 and 12 to `bevy_lights` while the loop's
  # clone sat 7 commits behind, so the pass would have redesigned a pool the human had already rebuilt
  # — the loop's whole product is a diff, and a diff against stale code is the wrong diff. `--ff-only`
  # and only on a clean tree: a fast-forward is the one history change that cannot lose work, and a
  # leftover from a killed pass is never resolved by merging it (that pass may be mid-thought).
  # A clone that has diverged from origin (local commits) is reported, not merged: choosing a merge
  # strategy is the operator's call, and a rebase is the one thing this driver must never do to a tree
  # a loop is driving.
  local behind=0 ff_from=0 ff_to=""
  if [ "${dirty:-0}" -eq 0 ] && [ "${WIGGUM_NO_FETCH:-0}" != "1" ]; then
    if git -C "$REPO" fetch --quiet origin 2>/dev/null; then
      local upstream; upstream=$(git -C "$REPO" rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null)
      if [ -n "$upstream" ]; then
        local ahead
        ahead=$(git -C "$REPO" rev-list --count "$upstream..HEAD" 2>/dev/null || echo 0)
        behind=$(git -C "$REPO" rev-list --count "HEAD..$upstream" 2>/dev/null || echo 0)
        if [ "${behind:-0}" -gt 0 ] && [ "${ahead:-0}" -eq 0 ]; then
          if git -C "$REPO" merge --ff-only "$upstream" >/dev/null 2>&1; then
            ff_from=$behind
            ff_to=$(git -C "$REPO" rev-parse --short HEAD)
            behind=0
          else
            log "pre-pass: WARNING could not fast-forward $behind commit(s) — pass works on stale code"
          fi
        elif [ "${behind:-0}" -gt 0 ]; then
          log "pre-pass: WARNING $behind behind and $ahead ahead of $upstream — diverged, left as is"
        fi
      fi
    else
      log "pre-pass: WARNING fetch failed — pass works on whatever the clone has"
    fi
  fi

  ensure_state
  local n; n=$(( $(cat "$COUNTER" 2>/dev/null || echo 0) + 1 ))
  echo "$n" >"$COUNTER"
  # **The fast-forward is recorded against the pass it precedes**, so `pass N START … behind=7` and
  # the reason the clone is current are adjacent lines in one log rather than in the previous pass's
  # story. `pre-pass:` lines above this point are deliberately pass-numbered too.
  if [ "${ff_from:-0}" -gt 0 ]; then
    log "pass $n pre: fast-forwarded $ff_from commit(s) to $ff_to"
  fi
  local passlog="$PASS_LOG_DIR/pass-$(printf '%04d' "$n")-$(date '+%Y%m%d-%H%M%S').log"
  # **Never start a pass that cannot finish.** A full `/home` breaks every loop and hs-distill at once,
  # and measured 2026-10-26 the society tree alone grew ~1 GB/min (`target` was 417 GB and climbing;
  # 638 GB free meant roughly ten hours). The pruning below is limited to what cargo regenerates:
  # abandoned linker temp files — measured 43 GB of `.tmp*` in `target/debug/deps`, up to 2.1 GB each
  # and none open — and `incremental/`, which costs one slower rebuild and regrows. Never `deps/`
  # generally, never `target/`: those are the build.
  #
  # **`-mmin +20` is the safety on the delete.** A live linker's temp file is younger than that, so the
  # window can only catch files whose linker is gone.
  local min_free_gb="${WIGGUM_MIN_FREE_GB:-150}"
  # The build directory cargo actually writes: `$CARGO_TARGET_DIR` when set (the Mac's loops build into one shared
  # `~/.cargo-target-shared/global`). Pruning `<repo>/target` there freed nothing and stopped Sanctuary's loop at
  # 14 GB free (2026-10-28).
  local target_dir="${CARGO_TARGET_DIR:-$(git -C "$REPO" rev-parse --show-toplevel 2>/dev/null)/target}"
  local free_gb
  free_gb=$(df -Pk "$REPO" 2>/dev/null | awk 'NR==2 {printf "%d", $4/1048576}')
  if [ -n "$free_gb" ] && [ "$free_gb" -lt "$min_free_gb" ]; then
    find "$target_dir/debug/deps" -maxdepth 1 -name '*.tmp*' -mmin +20 -delete 2>/dev/null
    rm -rf "$target_dir/debug/incremental" 2>/dev/null
    free_gb=$(df -Pk "$REPO" 2>/dev/null | awk 'NR==2 {printf "%d", $4/1048576}')
    log "pass $n prerun: pruned caches, free=${free_gb}GB"
    if [ "${free_gb:-0}" -lt "$min_free_gb" ]; then
      log "pass $n ABORTED: ${free_gb}GB free < ${min_free_gb}GB after pruning — refusing to start"
      echo "ABORTED: only ${free_gb}GB free on $(df -Pk "$REPO" 2>/dev/null | awk 'NR==2{print $6}')." >&2
      return 1
    fi
  fi
  log "pass $n START command=/$COMMAND model=$MODEL dirty=$dirty behind=$behind"
  # **A fresh process per pass is the whole design.** `omp -p` is one-shot: it reads the prompt,
  # does the work, exits. No `--continue`, no `--resume`, no session carried over — the repository is
  # the memory. `--model` is explicit so a pass never inherits an interactive choice.
  # **The ceiling is a poll loop in the driver, not a background watchdog.** `timeout(1)` does not exist
  # on macOS (measured: every pass died `rc=127`), and two watchdog designs each left a process behind —
  # `( sleep N; … ) &` orphaned its `sleep`, and so did `&& {}`, leaving a `sleep 21600` reparented to
  # pid 1 after every pass. A leaked process per pass for days is a leak that matters, and the orphan
  # also made `systemctl --user stop` sit in `deactivating` until it expired.
  #
  # Polling has **no process to leak**: the only child is the pass itself. It also gives `stop` a
  # meaning it did not have — a stop request is noticed within 5 s and the pass is ended politely,
  # instead of the driver waiting out a 6-hour ceiling.
  #
  # **The timeout is marked in a file, not inferred from the exit code.** A pass killed by SIGTERM and
  # a pass that died for its own reasons both exit 143; only the flag can tell them apart.
  local pass_pid rc timed_out=0
  local deadline=$(( $(date +%s) + PASS_TIMEOUT ))
  # **A checkpoint saves the tree while the pass is still running, and it is the only thing that
  # survives a cgroup kill.** Measured 2026-10-26 14:02 — `systemd-oomd killed 391 process(es)` in the
  # lights unit, the driver included, so passes 8 and 10 never reached their own `END` line and the
  # end-of-pass save never ran; society kept 19-20 dirty files across two such deaths. Saving
  # only at pass end assumes the driver outlives the pass, and under memory pressure it does not.
  # Every `WIGGUM_CHECKPOINT` seconds (default 600) the tree is written to a `refs/wiggum/` snapshot,
  # so the worst case is losing the last interval rather than the whole pass — and nothing lands on a
  # branch, so the gate is never bypassed. See `snapshot()`.
  local checkpoint_at=$(( $(date +%s) + CHECKPOINT_EVERY ))
  checkpoint() {
    # Never fails the pass: a checkpoint that cannot save is reported and the pass continues.
    [ -n "$(git -C "$REPO" status --short 2>/dev/null)" ] || return 0
    snapshot "$n" checkpoint
  }
  rm -f "$STATE/pass-timed-out"
  local -a omp_args
  omp_args=(--model "$MODEL" --thinking "$THINKING")
  [ -f "$OMP_OVERLAY" ] && omp_args+=(--config "$OMP_OVERLAY")
  # **Every step of the pass, timed, afterwards.** omp records the session as JSONL; `steps.py` turns it
  # into one row per model turn, tool call, background job and wait in `$STATE/steps.tsv`, so the slow
  # steps of a loop can be found (`steps.py report --state $STATE`). It reads the session, so it needs
  # nothing from the pass and works for a pass that was killed. Owner, 2026-10-31.
  local pass_started_at
  pass_started_at=$(date +%s)
  ( cd "$REPO" && exec omp "${omp_args[@]}" -p "/$COMMAND" ) \
    >"$passlog" 2>&1 &
  pass_pid=$!
  while kill -0 "$pass_pid" 2>/dev/null; do
    sleep 5
    # **`WIGGUM_CHECKPOINT=0` must mean never, not "every 5 s".** `checkpoint_at` is `now + 0` in that
    # case, so the elapsed test below is true on every poll; the interval is therefore gated on the
    # setting too. Cheap refs make a five-second snapshot less destructive than it used to be, but a
    # ref per poll for hours is still the wrong thing to write.
    if [ "$CHECKPOINT_EVERY" -gt 0 ] && [ "$(date +%s)" -ge "$checkpoint_at" ]; then
      checkpoint
      checkpoint_at=$(( $(date +%s) + CHECKPOINT_EVERY ))
    fi
    if [ "$(date +%s)" -ge "$deadline" ]; then
      : >"$STATE/pass-timed-out"
      kill -TERM "$pass_pid" 2>/dev/null
      sleep 10
      kill -KILL "$pass_pid" 2>/dev/null
      break
    fi
    # **A stop request ends the pass politely** — the trap writes the stop file, and this is where it
    # is seen. The pass gets TERM, then KILL if it has not exited in five seconds.
    if [ -f "$STOPFILE" ]; then
      kill -TERM "$pass_pid" 2>/dev/null
      sleep 5
      kill -KILL "$pass_pid" 2>/dev/null
      break
    fi
  done
  wait "$pass_pid" 2>/dev/null
  rc=$?
  if [ -f "$STATE/pass-timed-out" ]; then
    timed_out=1
    rm -f "$STATE/pass-timed-out"
    log "pass $n TIMED OUT after ${PASS_TIMEOUT}s — killed, tree left as found"
  fi
  if [ "$rc" -eq 124 ]; then
    log "pass $n TIMED OUT after ${PASS_TIMEOUT}s — killed, tree left as found"
  fi
  log "pass $n END rc=$rc timed_out=$timed_out log=$passlog"
  # Never fails the pass: a missing tool or an unreadable session is one log line.
  local steps_tool
  steps_tool="$(cd "$(dirname "$0")" && pwd)/steps.py"
  if [ -f "$steps_tool" ]; then
    log "pass $n $(python3 "$steps_tool" record --repo "$REPO" --since "$pass_started_at" --pass "$n" --state "$STATE" 2>&1 | tail -1)"
    # And onto GitHub, on the repo's own `loop-stats` branch (`steps/<clone>.tsv` and `.md`), built with
    # plumbing so the clone's tree, index and `main` are untouched. WIGGUM_STEPS_BRANCH= turns it off.
    if [ -n "${WIGGUM_STEPS_BRANCH-loop-stats}" ]; then
      log "pass $n $(python3 "$steps_tool" publish --repo "$REPO" --state "$STATE" --branch "${WIGGUM_STEPS_BRANCH-loop-stats}" 2>&1 | tail -1)"
    fi
  fi

  # **A tree left dirty is the next pass's first bug**, and the command says so.
  #
  # **But reporting is not enough, and that is measured.** A pass commits at its last step, behind
  # implement/gate/benchmark, so a pass killed at the ceiling or by the OOM killer has usually done the
  # work and *never* reaches its own commit. Measured 2026-10-26: society accumulated 15 uncommitted
  # files and lights 8 across killed passes, none of it on any branch — and worse, a clone left with
  # unpushed commits *diverges* from origin, so the next pass's `--ff-only` can never recover it
  # (lights sat 1 ahead, 2 behind, permanently unable to fast-forward).
  #
  # So the driver saves it **as a `refs/wiggum/` snapshot, and pushes that ref** — never a commit on
  # the branch. Committing it on the branch was measured 2026-10-26 to put ungated work on
  # `origin/main` in all four repos, bypassing the commands' own gate-then-merge. The snapshot is the
  # same safety net (the work is in an object on origin, recoverable with `git show`) with no way to
  # reach a branch anybody builds from. The ref is named for the pass, so the next pass can find it if
  # it wants to; it is deliberately not told to look.
  #
  # **A detached HEAD is saved too, now.** The old branch-targeted push had to refuse there; a ref
  # points at the commit, not the branch, so the work is recoverable either way.
  local after; after=$(git -C "$REPO" status --short 2>/dev/null | wc -l | tr -d ' ')
  if [ "${after:-0}" -gt 0 ]; then
    log "pass $n WARNING: $after uncommitted file(s) left behind"
    if [ "$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null)" = "HEAD" ]; then
      log "pass $n rescue: detached HEAD — saving $after file(s) to a snapshot ref"
    fi
    snapshot "$n" final
  fi
  return $rc
}

driver() {
  # **The clone check comes before any write.** A driver pointed at somebody's rsync scratch must not
  # create a directory in it, and the whole run is pointless there anyway.
  if [ ! -d "$REPO/.git" ]; then
    echo "refusing to run: $REPO is not a git clone" >&2
    exit 2
  fi
  ensure_state
  # **One driver per machine.** Two drivers racing on one checkout produce interleaved edits and
  # neither can trust the other's tree. The marker is in the process table, not only the pidfile,
  # because a pidfile survives a kill -9.
  if other_driver_running; then
    log "refusing to start: another driver is already running"
    echo "another wiggum driver is already running ($MARKER)"
    exit 1
  fi
  echo $$ >"$PIDFILE"
  rm -f "$STOPFILE" "$DRAINFILE"
  # **A stop request ends the pass in flight, within seconds, and then exits.** Since `df7324e`
  # (2026-10-26, "stop finishes in seconds and leaves no process behind") the stop file is acted on inside
  # the pass's poll loop: the pass gets TERM, then KILL five seconds later. systemd's `systemctl --user
  # stop` sends SIGTERM to the whole cgroup (KillMode=control-group), which kills the pass anyway;
  # trapping the signal turns it into the same stop file, so the driver exits cleanly instead of being
  # killed in the middle of its own bookkeeping. What the pass had uncommitted is saved by the final
  # snapshot and parked on a branch at the next start, so the work survives and the pass's momentum does
  # not. **`drain` is the one stop that does not cut a pass.** `wiggum.sh drain` writes `$DRAINFILE`, which
  # is read only BETWEEN passes: at the top of this loop and in the sleep that follows a pass. A pass in
  # flight never looks at it, so it runs to its own end, commit and push included, and the driver then
  # exits cleanly. That is how to restart a loop onto a new driver or prompt for free. (Until `drain`
  # existed this comment said a stop finished the pass, and on 2026-10-27 two passes, 78 and 48 minutes in,
  # were ended by a stop issued on its word.) A driver started before `drain` existed never reads the file.
  trap 'touch "$STOPFILE"; log "driver: SIGTERM received, ending the pass in flight"' TERM INT
  local omp_config=none
  [ -f "$OMP_OVERLAY" ] && omp_config="${OMP_OVERLAY#"$REPO"/}"
  log "driver START pid=$$ model=$MODEL repo=$REPO sleep=${SLEEP_BETWEEN}s max=${MAX_PASSES:-forever} omp_config=$omp_config"
  if [ -f "$SETTINGS_FILE" ]; then log "driver settings from ${SETTINGS_FILE#"$REPO"/}:$(printf ' %s' $SETTINGS_ACCEPTED)"; fi
  local idle_streak=0

  while :; do
    [ -f "$STOPFILE" ] && { log "driver STOP: stop file"; break; }
    [ -f "$DRAINFILE" ] && { log "driver STOP: drain requested, no pass in flight"; break; }
    if [ "$MAX_PASSES" -gt 0 ] && [ "$(cat "$COUNTER" 2>/dev/null || echo 0)" -ge "$MAX_PASSES" ]; then
      log "driver STOP: reached WIGGUM_MAX=$MAX_PASSES"
      break
    fi

    local pass_started; pass_started=$(date +%s)
    one_pass
    rc=$?
    local pass_secs=$(( $(date +%s) - pass_started ))

    # **Five consecutive hard failures stop the loop.** A pass that exits non-zero once is ordinary —
    # a rate limit, a killed process. Five in a row means the machine or the checkout is broken, and
    # a loop that keeps hammering it burns the API budget producing nothing.
    if [ "$rc" -ne 0 ]; then
      FAILS=$(( $(cat "$STATE/consecutive-failures" 2>/dev/null || echo 0) + 1 ))
      echo "$FAILS" >"$STATE/consecutive-failures"
      log "pass failed rc=$rc consecutive=$FAILS"
      if [ "$FAILS" -ge 5 ]; then
        log "driver STOP: 5 consecutive failures — see $DRIVER_LOG and $PASS_LOG_DIR"
        break
      fi
    else
      echo 0 >"$STATE/consecutive-failures"
    fi

    # **The inter-pass sleep must be interruptible, or `stop` waits it out.** Measured 2026-10-26: a
    # driver sitting here ignored SIGTERM for the full 60 s because bash runs a foreground `sleep` as a
    # child and the trap only fires after it returns. A stop that takes a minute looks like a hung unit
    # (`deactivating`) and, with `TimeoutStopSec` short, becomes a killed driver instead of a clean one.
    # Polling in one-second slices makes a stop land within a second.
    # **How long to wait, which is longer after a pass that did nothing.** Doubling from the normal sleep
    # and capped, so 60 s becomes 2, 4, 8, 16 and 32 minutes and then an hour: a new issue or a freed
    # tree is still noticed within the hour, and the steady idle cost is about one pass an hour.
    local sleep_for="$SLEEP_BETWEEN"
    if [ "$rc" -eq 0 ] && [ "$pass_secs" -lt "$IDLE_UNDER" ] && [ "$IDLE_MAX" -gt "$SLEEP_BETWEEN" ]; then
      idle_streak=$(( idle_streak + 1 ))
      local doublings=0
      while [ "$doublings" -lt "$idle_streak" ] && [ "$sleep_for" -lt "$IDLE_MAX" ]; do
        sleep_for=$(( sleep_for * 2 ))
        doublings=$(( doublings + 1 ))
      done
      [ "$sleep_for" -gt "$IDLE_MAX" ] && sleep_for="$IDLE_MAX"
      log "pass $(cat "$COUNTER" 2>/dev/null || echo '?') idle (${pass_secs}s, streak ${idle_streak}): next pass in ${sleep_for}s"
    else
      idle_streak=0
    fi
    local waited=0
    while [ "$waited" -lt "$sleep_for" ]; do
      { [ -f "$STOPFILE" ] || [ -f "$DRAINFILE" ]; } && break
      sleep 1
      waited=$(( waited + 1 ))
    done
  done
  rm -f "$PIDFILE"
  log "driver EXIT after $(cat "$COUNTER" 2>/dev/null || echo 0) pass(es)"
}

case "${1:-}" in
  start)
    if running; then echo "already running (pid $(cat "$PIDFILE"))"; exit 0; fi
    refuse_bad_settings
    # **The state directory first, or the redirect below fails.** `>>"$DRIVER_LOG"` needs the directory
    # to exist, and the clone guard lives in `one_pass` — so a fresh repo's first `start` died on
    # `No such file or directory` before it ever checked the tree. Measured on bevy_debugger_mcp.
    if [ ! -d "$REPO/.git" ]; then
      echo "refusing to start: $REPO is not a git clone" >&2
      exit 2
    fi
    ensure_state
    # **A session leader that exists on this machine, and a launch that is checked.** `setsid` is util-linux
    # and macOS has none, so on a Mac the command below failed inside the background subshell, its error went
    # to the driver log, and this script still printed "started." (found 2026-10-28, starting the Sanctuary
    # loop on the owner's laptop). perl is on every Mac and `POSIX::setsid` is the same system call. After the
    # launch the pidfile is awaited, so the message says what happened instead of what was meant to.
    # **setsid + nohup, so an ssh drop cannot kill the loop.** This is the same reason the loop's
    # own cargo jobs are launched that way.
    # **A plain re-exec of this same script**, not a serialised copy of its functions: the script is
    # the program, and a driver rebuilt from `declare -f` is a second copy that can drift from it.
    # The environment carries every setting across.
    if command -v setsid >/dev/null 2>&1; then
      leader=(setsid)
    else
      leader=(perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' --)
    fi
    WIGGUM_MARKER="$MARKER" "${leader[@]}" nohup env \
      WIGGUM_MODEL="$MODEL" WIGGUM_REPO="$REPO" WIGGUM_SLEEP="$SLEEP_BETWEEN" \
      WIGGUM_MAX="$MAX_PASSES" WIGGUM_THINK="$THINKING" \
      WIGGUM_COMMAND="$COMMAND" WIGGUM_STATE="$STATE" \
      bash "$0" __driver "$MARKER" >>"$DRIVER_LOG" 2>&1 < /dev/null &
    up=0
    for _ in 1 2 3 4 5 6 7 8 9 10; do
      sleep 1
      if running; then up=1; break; fi
    done
    if [ "$up" = 1 ]; then
      echo "started (pid $(cat "$PIDFILE")). status: $0 status   /   follow: $0 tail"
      if [ -f "$SETTINGS_FILE" ] || [ -f "$OMP_OVERLAY" ]; then print_settings; fi
    else
      echo "FAILED to start: no driver came up within 10 s. The last lines of $DRIVER_LOG:" >&2
      tail -n 8 "$DRIVER_LOG" >&2
      exit 1
    fi
    ;;
  __driver)
    refuse_bad_settings
    # **The marker is an argv token, because that is the only place `pgrep -f` reads.** The environment
    # form this used to use was measured on 2026-10-26 to match nothing at all, so two drivers shared
    # one checkout. It stays in the environment as well so an operator can see which loop they are
    # looking at, but `other_driver_running` reads the argument, which arrives even when the unit starts
    # this branch directly with no `start` case in front of it.
    export WIGGUM_MARKER="${WIGGUM_MARKER:-$MARKER}"
    # **`systemd` starts this branch with no marker argument.** `ExecStart=… __driver` is the unit's whole
    # command line, so the guard would see no marker on a unit-owned driver and let a hand-started one
    # share its checkout. One `exec` puts the marker in the argv the process finally runs with, which is
    # what `pgrep -f` reads; the condition makes it happen once, never in a loop.
    if [ "${2:-}" != "$MARKER" ]; then
      exec bash "$0" __driver "$MARKER"
    fi
    driver
    ;;
  one)
    # **A smoke test that cannot lie about the model.** One pass, foreground, its output on screen.
    # **It is still a pass in this checkout**, so it must not run beside a driver that is mid-pass: the
    # same interleaving, and `one` is how an operator checks the loop while it is running. Refused rather
    # than queued, because a smoke test that has to wait is one nobody trusts the timing of.
    refuse_bad_settings
    if other_driver_running; then
      echo "refusing to run a pass: a wiggum driver is working this checkout ($MARKER)" >&2
      echo "use '$0 status' to see it, or '$0 stop' to end it (the pass in flight is terminated)" >&2
      exit 1
    fi
    one_pass
    ;;
  settings)
    # **What this loop will do, before it does it.** A repo's settings file is what makes a plain `start` safe on
    # a checkout people work in (see the top of this script), so an operator needs to read it as the driver will:
    # each line the file sets, whether the environment overrides it, and every line it rejects (exit 1).
    print_settings
    [ -z "$SETTINGS_REJECTED" ] || exit 1
    ;;
  status)
    if running; then echo "RUNNING pid $(cat "$PIDFILE")"; else echo "not running"; fi
    [ -f "$DRAINFILE" ] && echo "DRAINING: the pass in flight finishes, then the driver exits (a driver older than 'drain' ignores this)"
    echo "passes completed: $(cat "$COUNTER" 2>/dev/null || echo 0)"
    echo "consecutive failures: $(cat "$STATE/consecutive-failures" 2>/dev/null || echo 0)"
    echo "repo: $REPO   command: /$COMMAND   model: $MODEL"
    echo "state: $STATE"
    echo "--- last 5 driver lines ---"
    tail -5 "$DRIVER_LOG" 2>/dev/null || echo "(no log yet)"
    ;;
  guard)
    # **Call this before touching a repo a loop is driving.** A `git reset --hard` (mine, on
    # 2026-10-26) ran while a pass was mid-edit and silently destroyed its uncommitted source change;
    # only the untracked test file survived, because `reset --hard` leaves untracked files alone. The
    # pass then had no idea its work had gone. Exit 0 means safe to touch, 1 means a pass is running.
    if running || pgrep -f "wiggum-$USER-$(basename "$REPO")" >/dev/null 2>&1; then
      echo "A LOOP IS DRIVING $REPO — do not reset, checkout, clean or rebase it." >&2
      echo "Let the pass finish and stop the loop: $0 drain --wait   (or $0 stop, which ends the pass now)" >&2
      exit 1
    fi
    if [ -n "$(git -C "$REPO" status --short 2>/dev/null)" ]; then
      echo "No driver, but $REPO has uncommitted work — it may be a dead pass's." >&2
      git -C "$REPO" status --short | head -10 >&2
      exit 1
    fi
    echo "safe: no loop is driving $REPO"
    ;;
  stop)
    touch "$STOPFILE"
    echo "stop requested: the pass in flight is ended (TERM, then KILL after 5 s) and the driver exits. Its tree is snapshotted and parked at the next start."
    running && echo "pid $(cat "$PIDFILE") is still working"
    ;;
  drain)
    # **The stop that does not cut a pass.** Writes a file the driver reads only between passes, so the pass
    # in flight finishes (its own commit, push and log included) and the driver then exits cleanly. `--wait`
    # blocks until it has, so `wiggum.sh drain --wait && systemctl --user start wiggum@x` is a restart onto a
    # new driver or prompt that costs nothing. A driver that predates this command never reads the file, so
    # the wait is capped (`WIGGUM_DRAIN_WAIT`, default 8 hours) and leaves the file in place when it gives up.
    if ! running; then echo "no driver is running for $REPO"; exit 0; fi
    ensure_state
    touch "$DRAINFILE"
    echo "drain requested: the pass in flight (if any) finishes, then the driver exits. Nothing is cut."
    if [ "${2:-}" = "--wait" ]; then
      dpid=$(cat "$PIDFILE"); t0=$(date +%s); cap="${WIGGUM_DRAIN_WAIT:-28800}"
      while kill -0 "$dpid" 2>/dev/null; do
        if [ $(( $(date +%s) - t0 )) -gt "$cap" ]; then
          echo "still running after ${cap}s; the drain file is left in place" >&2
          exit 1
        fi
        sleep 2
      done
      echo "driver exited cleanly after $(( $(date +%s) - t0 )) s"
    fi
    ;;
  tail)
    tail -f "$DRIVER_LOG"
    ;;
  *)
    sed -n '2,29p' "$0" | sed 's/^# \{0,1\}//'
    ;;
esac
