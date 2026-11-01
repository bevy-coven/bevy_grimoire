# Running a Wiggum loop: a new one, or one that already exists

`tools/loop/README-wiggum.md` is how to *read* a loop that is already running. This file is how to
**start one on a machine that does not have it yet**, and the facts that cost a cycle to learn. Read it
before touching a unit or a plist.

## The four pieces

| Piece | Lives in | Owns |
|---|---|---|
| driver | a clone's `tools/loop/wiggum.sh` | *how* a pass runs: one fresh `omp -p`, never mid-edit, refuse a bad tree |
| command | `<repo>/.omp/commands/<name>.md` | *what* a pass does, with that repo's conventions |
| supervisor | `wiggum@<name>.service` (systemd) or `com.ladvien.wiggum-<name>.plist` (launchd) | restart, boot persistence, per-loop display |
| state | `~/.local/state/wiggum/<repo>/` | `driver.log`, `passes/pass-NNNN-*.log`, counters |

`omp` is on PATH only in a login shell; **every** supervisor must set `PATH` explicitly or every pass
dies `rc=127` while the job still reports `active`.

## Start a new loop, in order

1. **A dedicated clone, never a working tree.** `git clone` the repo as `<repo>_loop` (or a clone already
   used by that loop) and require `git status --short` empty at `main` before starting.
2. **Install the driver, and hide it from `git status`.** `tools/loop/` is deliberately untracked in the
   clones. The driver aborts a pass in a tree with >40 modified files, but `dirty=1` (its own directory)
   is still noise a pass will try to "fix" — add `/tools/loop/` to `.git/info/exclude`.
3. **Prove the command resolves** before writing any unit. From the clone root:

   ```sh
   omp --model ollama-cloud/deepseek-v4.1-flash:1017 --thinking low \
     -p "Reply with exactly one token: CMD-OK-<name> if a slash command named <command> exists"
   ```

4. **Write the supervisor** (templates below) and verify it: `systemd-analyze --user verify` must print
   nothing; `plutil -lint` must say `OK`.
5. **Build the gate's prerequisite.** A gate that is not built fails every pass for a reason that is not
   in the code — for `bevy_grimoire` that is
   `cargo build -p bevy_cauldron -p bevy_cauldron_mcp` in the *same* `CARGO_TARGET_DIR` the unit sets,
   because `accept.py` asks `cargo metadata` where the binary is and exits if it is absent.
6. **Start, then read the log for `pass 1 START`.** `command=` and `model=` in that line are the proof
   the right thing is running. `rc=127` is PATH or a missing binary, never a model failure.

## Put a build where there is room, not where the checkout happens to be

**Never pick a compile host by habit.** Ask Home Cloud's OTEL first:

```sh
python3 tools/loop/otel_host_pick.py            # every host, ranked, with the exclusion reason
python3 tools/loop/otel_host_pick.py --pick     # one ssh destination
python3 tools/loop/otel_host_pick.py --arch arm64 --need-cores 6 --min-mem-gb 8
python3 tools/loop/otel_host_pick.py --json     # eligible + all, for another script
```

It is also installed on both hosts as **`otel-host-pick`** (`~/.local/bin`), which is what a pass in
*another* repo should use — `bevy_lights` and `bevy_society` do not contain this file, and a decision
about where to compile is not a property of one repo. The copy in this repo is the source of truth;
re-install it after changing it:

```sh
cat tools/loop/otel_host_pick.py | ssh big_mac 'cat > $HOME/.local/bin/otel-host-pick && chmod 755 $HOME/.local/bin/otel-host-pick'
```

Dashboard: `http://192.168.1.104:3000/d/default-home-dashboard/home` (Grafana 13.2.4, one Prometheus
datasource). The script reads Prometheus directly at `http://192.168.1.104:9090` — the Grafana proxy
works too (`/api/datasources/proxy/uid/prometheus/…`) but adds a hop for a tool that runs in a pass.

**Rank by idle cores, never by raw load.** `free_cores = cores × (1 − busy)`, with `busy` from
`rate(node_cpu_seconds_total{mode="idle"}[5m])`. Raw `load1` always favours the biggest machine: on
2026-10-26 `big` looked busiest (load 25) yet was a worse choice than `bmb`, which had 12.9 idle cores
against big's 4.

**It exits non-zero when nothing qualifies**, and prints why each host was excluded. Treat that as an
answer — a compile sent to a host with no free RAM fails after the linker, hours in. The observed split on
2026-10-26:

| host | cores | cpu% | free cores | note |
|---|---|---|---|---|
| `bmb` | 14 | 7.6 | 12.9 | M4 Pro; the right target for a heavy build |
| `Cs-MacBook-Pro` | 10 | 96.3 | 0.4 | **saturated** — four loops and a build share it |
| `big` | 24 | 83.2 | 4.0 | 2.8 GB available, swap already deep |
| `four`, `one`, `two`, `three` | 4 | <10 | ~3.7 | cheap, roomy, but small |
| `mac_air` | 10 | 32.9 | 6.7 | 7.4 GB disk free — too little for a Bevy `target/` |

Two measurement traps the script already handles, and that a hand-written query will hit: the macOS
exporter publishes **lowercase** memory names (`node_memory_free_bytes`, `node_memory_inactive_bytes`,
`node_memory_total_bytes`) and has no `MemAvailable`, so a Linux-only query silently excludes every Mac;
and `node_filesystem_avail_bytes` must be filtered to the real data mount, because a macOS
`AppTranslocation`/`nullfs` stub reports 191 GB on a host whose `/` has 22 GB.

## A killed pass must not strand its work

A pass commits at its **last** step, behind implement, gate and benchmark. So a pass killed at the
`WIGGUM_PASS_TIMEOUT` ceiling, or by the OOM killer, has usually done the work and never reaches its own
commit. Measured 2026-10-26: society accumulated **15** uncommitted files and lights **8** across killed
passes, none of it on a branch — and the clone was left **ahead of origin and behind it**, so the next
pass's `--ff-only` could never recover it (lights sat 1 ahead, 2 behind).

So the driver saves it to **a snapshot ref, never a commit on a branch**:

- `snapshot()` writes the tree with a private `GIT_INDEX_FILE` to
  `refs/wiggum/pass-N-{checkpoint,final}-<ts>`, parented on `HEAD`, then pushes **that ref**. The
  index, `HEAD` and the branch are untouched, so the pass's own `git status` is unchanged and this
  cannot move a branch anybody builds from;
- the private index is the load-bearing detail. `git stash create` moves the real index, which leaves
  the pass's `git status --short` empty mid-work and breaks the loop command's own rule that the tree
  must be clean before its commit step;
- a ref under `refs/wiggum/` is **not a branch**: `git branch` and `git log` do not show it, so a pass
  cannot mistake it for its own work. Recover one with `git -C <clone> log --all --oneline
  --grep='WIP(pass'` and then `git show <ref>`;
- a plain push only — never a force, never a rebase.

**Why not commit it to the branch, which is what this used to do.** Measured the same day: committing
the checkpoint to the pass's branch put **ungated work on `origin/main` as the branch tip** in all four
shared repos (lights `35c7824`, society `89d9563` — a mid-pass checkpoint —, elements `911ca5b`). Each
loop command builds a `loop/iter-N` branch from `main` and merges back **only after its gate**, so a
commit written straight onto `main` bypasses the gate, and Sanctuary path-deps `bevy_grimoire_core`. A
safety net that skips the evaluator is worse than losing the last ten minutes.

Verified on a throwaway repo: after a checkpoint + final snapshot, `HEAD` was still `main`,
`origin/main` still had its original single commit, the dirty untracked file was still untracked (the
index was not moved), and both refs were on origin and recoverable.

**It is not a substitute for the pass committing.** A snapshot has not been through the gate, but unlike
a commit it is not on the branch either — so every loop command now tells its pass to commit
incrementally (research note, then implementation, then docs and log) rather than once at the end, and
that the only thing which puts work on `main` is its own commit step.

**A snapshot saves the work; it does not clear the clone.** The tree stays dirty on whatever branch the pass was on, and the next pass used to refuse it. The driver now parks that tree on a branch before each pass (see the facts list below, "A tree a pass leaves behind is parked"), so a clone starts every pass on `main`.

**Refs accumulate and are never auto-pruned.** One ref and one commit per checkpoint. Past ~50 in a
clone, prune by hand — never from the driver, because an automatic delete of the only copy of a killed
pass's tree is the failure this design exists to prevent:

```sh
git -C <clone> for-each-ref --format='%(refname)' refs/wiggum | head -n -50 | xargs -r -n1 git update-ref -d
```

## A pass will not start on a full disk

`WIGGUM_MIN_FREE_GB` (default 150) is checked at the top of every pass. Measured 2026-10-26: the society
tree alone was growing ~1 GB/min and `/home` was ~10 hours from full, which breaks all four loops and
`hs-distill` at once. Below the threshold the driver first prunes only what cargo regenerates —
`target/debug/deps/*.tmp*` older than 20 minutes (43 GB measured, abandoned linker files, none open) and
`target/debug/incremental/` — then refuses the pass and returns non-zero if that was not enough. Never
`deps/` generally, never `target/`: the deletion is limited to files a live process cannot own and to a
directory cargo rebuilds.

## A pass reads the filed issues before it picks what to build

Each loop prompt carries a "Filed issues come before new features" step immediately before the step that ranks candidates, because the four repos had 15 open issues between them on 2026-10-27 (lights 3, society 4, elements 7, grimoire 1) and no prompt had mentioned one. A bug issue is the pass's item ahead of any enhancement, an enhancement issue is ranked with the pass's own candidates and its number goes in the rank list, a fix says `Fixes #N` in the commit and the report, and a release issue is recorded and skipped because publishing is the owner's call. big has an authenticated `gh`; big_mac has none, so elements and grimoire read `https://api.github.com/repos/Ladvien/<repo>/issues?state=open`, which allows 60 requests an hour per IP and answers 403 or 429 beyond that. The prompt therefore says a 403 or 429 is logged as "could not be read" and never as "no issues", because a pass that records silence it never looked at has hidden the list from every later pass.

The same ship step carries "README, demos and docs stay true", for a matching reason: the 2026-10-27 audit found a false line in each of the four READMEs, and society's and grimoire's prompts had never told a pass to touch the README or the clips at all.

## Steer a loop: its settings, its provider, its keys and its focus

All four are files in the loop's own repo or one command, so the start line stays `WIGGUM_REPO=<repo> WIGGUM_COMMAND=<command> wiggum.sh start` and nobody has to remember flags. A pass never edits any of them: they are in each loop command's protected list.

|To change|Edit|Takes effect|
|---|---|---|
|the focus|the body below the `---` line of the repo's focus file (`docs/loop/focus.md` here, `docs/loop/emerge/focus.md` in Sanctuary); commit and push|the next pass; no restart|
|driver settings (`WIGGUM_MODEL`, `WIGGUM_PARK`, `WIGGUM_SNAPSHOT`, ...)|`<repo>/.omp/wiggum/<command>.env`, `NAME=VALUE` lines|the next driver start: `wiggum.sh drain --wait`, then `start`|
|omp's own settings for the loop's passes only|`<repo>/.omp/wiggum/<command>.omp.yml`, passed to omp as `--config`|the next pass|
|which Ollama keys omp may rotate through|`python3 tools/loop/omp_keys.py ollama-cloud <command>` on that host, with `list`, `probe`, `keep` or `restore`|the next model call|

`WIGGUM_REPO=<repo> WIGGUM_COMMAND=<command> wiggum.sh settings` prints what the driver will use: each line of the `.env` and whether the environment overrides it, every line it refuses (a refused line makes `start` and `one` exit 2), and the overlay or `omp overlay: none`. A checkout that people and other agents work in (Sanctuary's) carries `WIGGUM_PARK=0`, `WIGGUM_CHECKPOINT=0` and `WIGGUM_SNAPSHOT=0`: measured 2026-10-28, without the last one the driver still wrote and pushed a final `refs/wiggum/` snapshot of other agents' unfinished files.

**Ollama first, Anthropic when Ollama is out.** `WIGGUM_MODEL=ollama-cloud/gpt-oss:20b` in the `.env` and this overlay:

```yaml
retry:
  fallbackChains:
    "ollama-cloud/*":
      - anthropic/claude-sonnet-5-5
```

Measured 2026-10-28 with both Ollama accounts at their weekly limit: omp tried the `Ladvien` key (429), switched to the `cthomasbrittain` key (429), then answered from `anthropic/claude-sonnet-5-5`; the same overlay through the driver (`wiggum.sh one` on a scratch repo) did the same. Keep the overlay per loop: the same chain in `~/.omp/agent/config.yml` would move the owner's interactive sessions and subagents too.

**One stored key per account.** On a usage-limit error omp first switches to a sibling stored key of the same provider, and Ollama's quota is per account, so extra keys of an exhausted account are extra failures before the fallback. On 2026-10-28 the laptop held nine `ollama-cloud` keys for two accounts plus a dead one; `omp_keys.py ollama-cloud keep <digest> <digest>` left one per account on each host (laptop and big_mac `fef2f5a0` and `6bce445a`, big `5341594b` and `6bce445a`). It names keys by the first 8 hex of their SHA-256 and never prints one; `keep` disables softly and prints the `restore` that undoes it; `export <digest> | ssh <host> python3 omp_keys.py ollama-cloud import` copies a key to a host that lacks one (`probe` says which account a key belongs to: a 429 names it).

## Facts that cost a cycle

- **A loop cannot simply be moved to the idle machine, because the disk is the binding constraint, not
  the cores.** Measured 2026-10-26: `bevy_society`'s `target/` is **239 GB** and `bevy_lights`' is 41 GB.
  `bmb` — the host the picker ranks first on idle cores — has **23 GB** free, and `mac_air` 14 GB. There
  is no room to build either on a Mac, so re-homing them would trade an OOM for a full disk. `big` has
  804 GB free and 22 idle cores, which is why the fix there is a memory bound, not a new host. Run
  `du -sh <clone>/target` before proposing a move; a Bevy workspace's target is larger than most
  machines' free space.
- **Two loops that compile do not fit on `big`.** Measured 2026-10-26: `wiggum@society` died
  `Result: oom-kill` at a 6.9 GB peak with 8.7 GB of swap while `hs-distill` held 9.9 GB resident and
  the host showed 1 GB available. `CARGO_BUILD_JOBS` is now 4 there, and one loop is held until the
  other's pass finishes. A loop that OOMs is worse than a loop that waits: systemd counts the kill as a
  restart, and `StartLimitBurst=3` can retire the unit entirely.
- **`MemorySwapMax` is what oomd kills on, and 2 G was not enough.** Measured 2026-10-27, twice in an
  hour: `wiggum@lights` died `oom-kill` at exactly `6G memory peak, 2G memory swap peak` — both caps hit
  together — and three restarts then retired the unit with `Failed with result 'start-limit-hit'`
  (`StartLimitBurst=3` in `[Unit]`). `oomctl` names the rule: *Swap Used Limit 90 %* against the
  **monitored ancestor** `/user.slice/user-1000.slice`, whose `memory.swap.current` was 9.4 GB of
  31.2 GB (30 %) before a loop spilled into swap and pushed the slice's ratio over. `MemorySwapMax=0` on
  both big units stopped a unit adding to that ratio, and it was enough while the loops were only
  refusing dirty trees. **It was not enough once lights ran real builds:** oomd killed `wiggum@lights`
  three times in 40 minutes (17:24, 17:54, 18:02), each at a 6 GB peak, while society, with four build
  jobs, was killed zero times. Big's swap was 74 % full by then and almost none of it was the loops'
  (vLLM 5.8 GB, `hs-mcp` 4.9 GB, qdrant 2.5 GB; the llama-swap cgroup holds 7.8 GB), so oomd reached its
  90 % line on other services' memory and took the largest eligible cgroup, a lights build. Two changes
  followed, and only one is more than a mitigation: `StartLimitBurst=8` on both units, because three oomd
  kills are not a spin and had retired a healthy loop; and `CARGO_BUILD_JOBS=2` on lights, which is a guess
  at lowering its peak, with kills per hour as the test. The real lever is the machine's swap, which is not
  the loops' to free. `MemoryHigh`/`MemoryMax` stay: the rule is to bound swap, never to remove the memory
  cap. Check a unit's real position with `cat .../memory.swap.current` against `memory.swap.max`, not with
  `MemoryPeak` alone: society sat at 95 % of its swap cap while its peak looked unremarkable.
- **A `target/` for a Bevy workspace is tens of GB, so disk is a real eligibility check.** `mac_air` has
  10 cores and 7.6 GB of disk, which makes it useless for a compile however idle it is;
  `--min-disk-gb 40` on the picker is the honest default. Reclaiming `~/target/sanctuary/release`
  (6.3 GB, 28 h untouched) took it to 14 GB free — deleting a *profile* directory is safe where deleting
  a whole `target/` is not: the dependency cache the next build needs is in the other profile.
- **Check `WIGGUM_MAX` is actually in the installed plist/unit**, not only in the committed one. The
  grimoire agent ran `max=0` (forever) because the key was absent, and nothing in a pass log says
  "unbounded" — `driver START … max=0` is the only tell.
- **`launchctl kickstart -k` and `bootout` both kill the pass in flight** (`last exit = -15`). If a pass
  was killed, remove the driver's `stop` file and zero `consecutive-failures` before restarting, then
  `bootout` + `bootstrap` in strict sequence: kickstarting a job that launchd throttled (`state = spawn
  scheduled`, `active count = 0`) does nothing until `ThrottleInterval``ThrottleInterval` (600 s) elapses.
- **macOS has no `timeout(1)`** (and no coreutils: `timeout --foreground 5 echo hi` → 127). A driver that
  wraps a pass in `timeout` cannot run there. `tools/loop/wiggum.sh` since commit `338c5d3` implements the
  ceiling as a pid watchdog, so the same script runs on both hosts — **do not copy an older driver**.
- **One Xvfb per loop, and never `:0`.** `bevy_lights`, `bevy_society` and `bevy_grimoire` open real
  windows. Two units whose `ExecStartPre` reaps `Xvfb :99` reap *each other's* server. Each unit owns its
  own display (`:97`, `:98`, …) and matches its own exact command line. Check `pgrep -a Xvfb` first — a
  display already held by something not yours means pick another number, not kill it.
- **`StartLimit*` are `[Unit]` keys.** systemd ignores them in `[Service]` with only a warning.
- **`StandardOutput=journal`, not `append:`.** An `append:` path fails the whole start
  (`status=209/STDOUT`) when its directory is missing, and `LogsDirectory=` does not create it early
  enough.
- **`WIGGUM_MAX`, not a small cap.** The driver exits **0** at the cap, so `Restart=on-failure` /
  `KeepAlive.Crashed` will not restart it — a small number silently ends the loop.
- **`TimeoutStopSec`/`ExitTimeOut` > `WIGGUM_PASS_TIMEOUT`**, or a stop cuts a pass mid-edit.
- **`CARGO_TARGET_DIR` outside the repo, one per loop.** Workspace crates at one version share an artifact
  hash across checkouts, so a shared dir makes cargo call one tree's units `Fresh` and silently run the
  other tree's code (measured 2026-10-21). A clone's own `target/` is fine; `~/.cargo-target-shared/global`
  never is.
- **launchd does not expand `~` or `$HOME`.** The committed plists use a `/Users/USER` placeholder that the
  install step substitutes; installing an unsubstituted plist yields a job that looks `running` and does
  nothing.
- **Every way of stopping a loop ends the pass in flight, except `wiggum.sh drain`.** That covers `launchctl kickstart -k`, `bootout`, `systemctl --user stop` and the driver's own stop file, which sends the pass `TERM` and then `KILL` five seconds later (`df7324e`, 2026-10-26). The stop file is the tidy way (the driver exits cleanly and snapshots the tree), not the gentle one. Measured 2026-10-27, on the word of stale comments in the driver, both launchd plists, the unit template and `README-wiggum.md` that all said a stop "finishes the pass in flight": society's pass 20 (78 minutes in, 7 uncommitted files) and elements' pass 22 (48 minutes in, 11 files) ended `rc=143` nine seconds after a stop file was written to restart both onto a new driver. Nothing was lost, because the final snapshot saved each tree and the new park step committed it to a branch (`WIP(pass N, parked)`) and pushed it, but the passes' momentum was, and the next pass starts from `main`. Those comments now say what the code does, and **`drain` now exists**: `wiggum.sh drain --wait` writes a file the driver reads only between passes, so the pass in flight runs to its own end (`rc=0`, its commit and push included) and the driver then exits cleanly, which is how to restart onto a new driver or prompt for free. It is proven by 23 checks on macOS and Linux (an idle driver exits within seconds; a pass drained mid-flight ends `rc=0` with its work landed while a plain `stop` ends it `rc=143`; a stale drain file does not stop a fresh driver; a capped `--wait` gives up and leaves the file). **A driver started before `drain` existed never reads the file**, so for those the only no-cost restart is to wait for the gap between passes (`driver.log` showing `pass N END` or `idle (…): next pass in …s`) and write the stop file then, which cuts nothing because no pass exists; `systemctl --user restart` is no better than a stop, since KillMode=control-group sends SIGTERM to the pass too. After a drain or a stop, reset `consecutive-failures` before restarting; the driver clears a leftover `stop` or `drain` file itself when it starts.
- **A `TimeoutStopSec` that is longer than a pass turns an unrelated cgroup leak into a 7-hour
  `deactivating`.** Measured 2026-10-26: stopping `wiggum@lights` hung in `deactivating` for its full
  `TimeoutStopSec=25200` because a `home-still_qdrant_1` container (`rootlessport`, `conmon`,
  `entrypoint.sh`) had been parented **into `wiggum@lights.service`'s cgroup** at container start — a
  live CI runner job owned it, so killing it was not an option either. `ActiveState=deactivating`,
  `SubState=final-sigterm` and `cat <cg>/cgroup.procs` showing PIDs you never started is the signature.
  Writing those PIDs to another cgroup's `cgroup.procs` fails **`EBUSY`** while the target is a cgroup
  systemd manages for a service (no internal processes allowed), so the way out is a **transient scope**,
  which is a leaf that accepts them:

  ```sh
  systemd-run --user --scope --unit=wiggum-rescue-$(date +%s) sleep 7200 &
  # then, for each pid in the stuck cgroup:
  echo <pid> > /sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/app.slice/<scope>.scope/cgroup.procs
  ```

  The unit then finishes its stop and can be started. Root cause is elsewhere (a `podman run` from a
  service inherits that service's cgroup, so the container should be given its own `--cgroup-parent` or an
  `ExecStart` under a transient unit); treat the rescue as per-incident, not a fix.
- **The driver refuses a tree with >40 modified files**, so it will not run a pass in an rsync scratch tree.
- **An uncommitted edit to a loop's own prompt is parked on a branch, and the pass runs the old prompt.** Measured 2026-10-27: a rule added to `bevy_lights`' skill file in the live clone and left uncommitted was found by 50 consecutive passes (iterations 61 to 110), each of which saw a tracked file it had not changed, stopped as its own rule says, built nothing, and logged the stop as a commit. The same happened once on 2026-10-26. The driver now parks a leftover tree before each pass (next facts), so the loop no longer stalls, but the failure has changed shape and not gone: the edit is committed to a `wiggum/parked-pass-N-…` branch, the clone returns to `main`, and the pass runs the **committed** prompt, so the edit has no effect until it is on `main`. After editing a command or skill in a clone, stop the loop first (`wiggum.sh stop`), then commit it at once by explicit path (`git commit -- <path>`) and push it, unless the clone already holds the loop's own unpushed commit: a plain `git push origin main` would publish that too, and publishing a pass's work is the pass's decision. A pass's own `git add -A` will otherwise sweep the edit into its commit (society's `688af75` carried one), which hides what changed.
- **A loop whose model has no credit looks healthy.** Measured 2026-10-27: ollama-cloud answered `402 Insufficient credits` 109 times on big while both units were `active`, `consecutive-failures` was 0 and no unit had failed; every pass log held the 11 bytes `Working...` and nothing else. The tell is `grep -c 402 ~/.omp/logs/omp.<date>.<pid>.log` against a pass log that never grows. Switching `WIGGUM_MODEL` is not enough: omp's auxiliary roles in `~/.omp/agent/config.yml` (`modelRoles`, notably `smol`) each name their own provider, and `smol` was still on a second dry one, so skill-description compression and job-widget calls kept returning 402 after the main model answered. Check both, then confirm with a pass whose log grows.
- **A GIF whose frames all carry a 0-centisecond delay is a still.** Measured 2026-10-27: two of `bevy_elements`' README GIFs (35 frames each) had every Graphic Control Extension delay at 0, so a viewer that honours the delay shows one frame under a caption that promises motion. Count the delays by scanning for the `21 f9 04` block header and reading the two bytes after its packed-flags byte; rewriting those two bytes (8 cs, 12.5 fps, was used) leaves the file size and every pixel unchanged, so it needs no GPU recapture.
- **A pass can commit 131 MB in one commit.** Measured 2026-10-27 in society's `688af75`: ten `docs/loop/runs/2026-10-27-03/evidence/…/series.csv` files of 67,200 lines each (about 14 MB apiece), 69 files and 682,564 insertions, on a repository whose `.git` was already 253 MB. Earlier runs commit files of the same name (`36c4d8c`, `9938002`), so it is the loop's practice and not an accident. It was pushed on 2026-10-27 at the owner's instruction (185 MB of pack, 11 s); each such commit is permanent once pushed, and whether evidence of that size should keep going into history is still open.
- **A pass waiting on a detached gate looks exactly like a hung one, and only its session file tells them apart.** Measured 2026-10-27 on society's pass 19, 4 h 45 m old: 0.5% CPU, no compiler among its children, no repository change for 83 minutes and the omp debug log silent for 27 minutes, which are every usual sign of a hang. It was inside a `wait` on a `setsid nohup` gate whose `cargo test` had passed (321 tests, 0 failed) and whose second step wrote its output file that same minute. The signal is `~/.omp/agent/sessions/-<repo>/<start>_<id>.jsonl`: its mtime is the last time the pass did anything, and its final entries name the tool it is inside, so read `tail -n 3` of it before calling a pass dead.
- **A detached gate is not reached by the driver's kill, and the cgroup is what ends it.** `setsid nohup` takes a process out of the pass's process group but not out of the unit's cgroup, and the driver signals only the pass's own pid (`kill -TERM "$pass_pid"`), so by reading the code a ceiling kill leaves the gate running with the cargo target-dir lock and several GB against the next pass inside a 7 GB, no-swap cgroup (inferred from the code, not observed). What was observed: stopping the driver through its stop file made the unit inactive and systemd tore the whole cgroup down, with 0 orphan processes afterwards, and the restarted unit's pass 20 was editing code within two minutes.
- **Society's gate does not fit a 6-hour pass that spends its first four hours on research.** Measured 2026-10-27: after the commit at 15:01 the pass launched `cargo test` (47 minutes, 321 tests across 80 suites) and then `cargo test --no-default-features` (50 minutes in, 70 of 80 suites done), with clippy and rustdoc still queued. That is over two hours, started an hour after the commit, and it was still on the second of four steps when the pass was stopped three minutes before its ceiling. Either the gate or the ceiling has to give; it is recorded here, not decided. **Where the time goes**, from the pass's own gate logs: the 47 minutes of `cargo test` were test running (the build was cached, 1 s), and two suites are 78% of it, `label_hysteresis.rs` at 19.8 minutes and `reciprocity.rs` at 17.0, each a 24-person village stepped through thousands of ticks, one test per thread; the other 78 suites take about ten minutes together. `--no-default-features` is 21 min 53 s of compile and link (78 test binaries built against the full Bevy umbrella, `CARGO_BUILD_JOBS=3` on a 24-core machine) plus 29.5 minutes of tests, 82% of them in the same two suites. `cargo test` runs binaries one after another, so with parallel binaries the floor would be the longest single test, at most 19.8 minutes, not the sum. The society crate itself compiles at opt-level 0 (only the dependencies are at 3); whether raising it shortens those two suites is **not established**: the one measurement, on a laptop with a load average of 30, ran `label_hysteresis` at opt-level 3 in 1,487 s, no faster than big's opt-level-0 run, and is too confounded to conclude from. The pass's clippy step had also failed on `neg_multiply` in `tests/reputation_patient.rs` when it was cut off.
- **A loop with nothing it may do costs as much as one that is working, so the driver backs off.** Measured 2026-10-27: lights ran 50 passes of about 20 s that each refused a stranded tree, and grimoire ran 25 passes of about 26 s whose only output was a log line, 19 of them pushed to `main` as "a pass that shipped nothing" commits, each pass a full model call every 90 seconds. The driver now treats `rc=0` in under `WIGGUM_IDLE_UNDER` (default 120 s) as idle and doubles the sleep for each consecutive idle pass up to `WIGGUM_IDLE_MAX` (default 3600 s), so 60 s becomes 2, 4, 8, 16 and 32 minutes and then an hour, which still notices a new issue within the hour; a longer or failed pass resets it and `WIGGUM_IDLE_MAX=0` turns it off. Proven against a fake `omp` on a throwaway repo: it doubles and caps, a 12 s pass resets the streak, failures are not counted as idle and keep the five-failure stop, and a stop lands in under a second in the middle of a 60 s backoff. A unit picks it up only when its driver restarts.
- **A baseline worktree holding `main` blocks the way back to it.** Measured 2026-10-27: lights' clone was stranded on `loop/iter-115` with an ungated tree, and `git checkout main` failed with `'main' is already used by worktree at '/home/ladvien/base_l115'`, a comparison checkout a pass had made. The skill says a pass starts on `main` with no tracked changes, so a clone left elsewhere stops every later pass. The driver now detaches a *clean* holder itself when it returns a clone to `main`, and refuses the pass, with the holder's path in the log, when the holder has changes in it. The lights skill now tells a pass to make its baseline with `git worktree add --detach` and to remove it when done. Removing the seven old ones freed about 2 GB: the other 37 GB under `~/l13`, `~/l15` and `~/l20` was in sibling `*-target` build directories, which `git worktree remove` does not touch and which are not worktrees; they were deleted on 2026-10-28 with the owner's approval, and every loop's `build-loop.md` now ends with a "Clean up when you are done" section so that a pass removes the pair itself (next fact).
- **A tree a pass leaves behind is parked on a branch, and the clone returns to `main`, before the next pass.** Measured 2026-10-27: lights logged 61 passes that each refused a tree they had not made and stopped in about 20 s (50 for an uncommitted prompt edit, 11 for a stranded `loop/iter-115` holding 412 lines of ungated shader work), and both were cleared by hand the same way. `park_stranded_tree` in `wiggum.sh` does it at the start of every pass: dirty on `main` cuts a `wiggum/parked-pass-N-<time>` branch from HEAD and commits there; dirty on another branch commits on that branch; a detached HEAD whose commit no branch holds gets a branch cut for it; then the branch is pushed and the clone checks out `main`. N is the pass that left the work. It **never writes to or pushes `main`**, because committing a leftover onto a branch somebody builds from put ungated work on `origin/main` in all four repos on 2026-10-26. It refuses the pass, and says why in the log, in the middle of a merge, rebase, revert or cherry-pick, when `main` is held by a worktree with changes, and (as before) above 40 changed files; each counts as a failure, so five in a row stop the loop. The commit is titled `WIP(pass N, parked)`, which is what every loop command tells a pass to search for (`git log --all --oneline --grep='WIP(pass'`), so a pass looking for a killed pass's work finds it as it finds a snapshot. The pass starts clean on `main` and is not told, so since 2026-10-28 rule 3 of each prompt's "saved even if you are killed" section tells it to list `wiggum/parked-*` once, after triaging the filed issues and before it picks an item, and to adopt the newest parked tree if it is for the item it would pick: as a first draft that was never built or gated, brought into its own branch and put through the real gate, never merged as a branch and never deleted. It is also in `driver.log` (`pass N pre: parked 3 file(s) on wiggum/parked-pass-N-…`), on origin, and in `git branch --list 'wiggum/parked-*'`. `WIGGUM_PARK=0` turns it off; `WIGGUM_BRANCH` names the branch to return to when origin has no `HEAD`. It leans on the rule that nothing but the driver writes to a loop's clone, so a person who edits one between passes has the edit parked, not lost. Pushing a parked branch triggers no CI in any of the four repos, whose workflows run on `main` and pull requests only. Tested with 57 checks across 18 cases, on macOS (bash 3.2) and on Linux (bash 5); the tests found a data-loss bug before it shipped: `git branch --contains HEAD` lists the detached HEAD itself, so a commit left on a detached HEAD looked held, the clone moved to `main`, and the commit lost its only name. `git for-each-ref --contains` names real refs only.
- **A parked branch or a snapshot that adds more than `WIGGUM_PUSH_MAX_MB` (default 10) of new data is committed and kept local.** Measured 2026-10-27: the park step pushed an 82 MB branch to society's **public** repository, six raw `series.csv` of about 14 MB each swept up by `git add -A` before the new `.gitignore` reached that clone (the cap was 100 MB then, and the six totalled 84), and the older snapshot push had already put the same blobs into 51 hidden `refs/wiggum/*` refs (passes 14 to 20); a 129 MB branch of nine more CSVs, `loop/wip-2026-10-26`, had been pushed at the owner's instruction without anyone opening it. The owner had ruled raw run output out of git an hour earlier. Both push paths now share one guard, measured on what the commit adds beyond its first parent (not on file sizes, so content already in history is free); a legitimate leftover is small, and elements' 11 files were 0.45 MB. The owner approved deleting the two branches and the 51 refs from origin: `refs/wiggum` there went from 75 to 24 and no blob over 5 MB is reachable from any side-channel ref, but GitHub keeps unreachable objects until it garbage-collects, which cannot be forced. Everything stayed on big. The other three loop repos hold 3 to 6 MB of side-channel data each. Before pushing anything a loop made, open it: `git diff --numstat <base> <branch>` with sizes takes a second.
- **`StartLimitBurst` counts operator starts, and a loop that oomd keeps killing is retired by it.** Measured 2026-10-27: `wiggum@lights` was killed by systemd-oomd six times between 17:54 and 19:00, and then sat `failed` with `Result=start-limit-hit` (restart counter 7 against `StartLimitBurst=8` in 24 hours, with the two manual `systemctl start`s from a restart session counted), so the loop was down and nothing in its own log said so. `systemctl --user is-active wiggum@<name>` printing `failed` is the tell, and a watcher waiting for a `pass N END` line that never comes is the other. `systemctl --user reset-failed wiggum@<name>` clears it, then `start`. The limit exists to stop a unit that spins; the defect was the kills.
- **`CARGO_BUILD_JOBS` does not bound everything a pass compiles, and `/tmp` here is RAM.** Measured 2026-10-27: after `CARGO_BUILD_JOBS=2` was set on lights, oomd killed it four more times, each at a 6 GB peak with 583 to 612 processes. A sampler caught the unit at 88 processes and 6 GB: 20 each of `rustc`, `cc`, `collect2` and `rust-lld` under one `cargo` and one `rustdoc`. Cargo's own jobs are 2, so those 20 links are not cargo's; the likely owner is rustdoc's doctest threads (one per core by default, 24 here), each compiling and linking its own binary, which would also explain the 252 leftover `/tmp/rustdoctest*` directories (1.3 GB) found from killed runs, because `/tmp` is a 16 GB tmpfs and its files are RAM. The unit now sets `RUST_TEST_THREADS=4` and a `TMPDIR` on disk (`~/.cache/wiggum-tmp/lights`, swept of anything older than two hours at each start); the linker's `cc*.cdtor.c` and `*.res` temp files were seen landing there. **There were no kills in the 10.6 hours after that restart (19:32 to 06:10), where the interval between kills had been 6 to 25 minutes, but that is not evidence for the fix:** every pass in that window was a 0.5 to 6 minute "nothing shipped" pass that never reached the gate or a doctest run, and the second sampler, set to log above 30 processes, finished with no samples because nothing ran that long. The hypothesis is untested; the first sampler also did not record parents. The test is kills per hour over passes that build and test: `journalctl --user -u wiggum@lights | grep 'oomd killed'`. Separately, swap on big fell from 53% to 24% over the hours in which `/tmp` was cleared on 2026-10-28 (234 `rustdoctest*` directories and other scratch, about 5.3 GB of tmpfs), but hs-mcp, which had been holding several GB and was mostly swapped, was also restarted in that window, and either would do it; the cause was not isolated.
- **A running driver has its command name in memory, so a rename is staged: add the new file, switch the loop, and only then remove the old one.** On 2026-10-28 lights and elements, each a thin `<repo>-loop.md` that told a pass to run a skill, became one `build-loop.md` like society and grimoire. Deleting the old file under a driver started with `WIGGUM_COMMAND=lights-loop` would have pointed every pass at a command that no longer exists (what omp does with an unknown slash command was not tested, and a failed pass counts toward the five-failure stop). The order used: push the new file; change `WIGGUM_COMMAND` in the unit or plist; restart in the gap between passes (the last pass line in `driver.log` is `END` or `idle`); read `pass N START command=/build-loop` in `driver.log` and the first message in that pass's session file; then `git rm` the old command and skill. A launchd plist is read when the job is loaded, so elements was switched with `launchctl bootout` and `bootstrap`, not `launchctl start`. Elements' pass 36 ran 2 h 54 m, so its switch waited from 06:32 to 08:03 and cut nothing: the old driver exited on its stop file in the gap and pass 37 started on `/build-loop` three seconds after the new one did. The new command must also render under omp's Handlebars, where one `{{` breaks every pass: probe each with `render_probe.sh` first. A user-level `~/.omp/commands/build-loop.md` on big (a copy of this repo's prompt from 2026-10-26) would now be what a clone *without* its own `build-loop.md` runs, which is a reason to keep the name only where a repo has the file.
- **A container a pass starts belongs to the loop's unit, and stopping the unit stops it.** Measured 2026-10-28: lights pass 179 (02:49) ran `hs restart` because distill was down ("as the skill says"); it restarted four services and distill was still down 60 seconds later, because distill is a system unit that `hs restart` cannot start without sudo. By inference (stopping the unit restarted Qdrant, and its monitors were found in the unit's cgroup afterwards), that pass put Qdrant's `conmon`, `rootlessport` and `rootlessport-child` inside `wiggum@lights.service`'s cgroup. Stopping the unit at 06:25:32 to switch its command killed them; podman's restart policy brought Qdrant back (its own log shows it starting at 06:26:12, so about 40 s without it) and the respawned monitors joined the dying cgroup, so the unit sat in `deactivating (stop-sigterm)` until `TimeoutStopSec` (300 s), which would have SIGKILLed `rootlessport`, Qdrant's port forwarder. They were moved into a transient `hs-qdrant-monitor.service` (`sleep infinity`) by writing their pids to its `cgroup.procs` at 06:29:56, about 35 s before the kill, and the unit finished stopping. A `systemctl --user start` issued behind a stuck stop blocks until the stop ends and stays queued if the client is killed, which is how the unit came up late on the new command. The tell is `systemctl --user status wiggum@<name>` listing `conmon` or `rootlessport` in its CGroup. The lights command now says `systemd-run --user --scope --collect hs restart`; that was checked with a harmless scoped process (it and its children land in a `run-*.scope`), and `hs restart` itself was not run.
- **`Restart=on-failure` does not restart a service that was sent SIGTERM, and a loop that cannot research idles instead of failing.** Measured 2026-10-28: home-still's `hs-serve-distill.service` (a system unit) exited with `ExecMainStatus=15` at 19:03:49 on 2026-10-27, which `on-failure` counts as clean, so distill and semantic search were down about 11 hours; lights logged "Research blocked: distill and qdrant refuse connections" and shipped nothing. The unit cannot be started without sudo, so `hs-serve-distill-user.service` (transient, `Restart=always`) runs `hs serve distill --port 7434` for now; the hand-back is `systemctl --user stop hs-serve-distill-user && sudo systemctl start hs-serve-distill`. Three services in this stack now share the shape (the driver at `WIGGUM_MAX`, launchd `KeepAlive` `Crashed`, this): a clean or signalled exit is not a failure to the supervisor, so something has to notice that nothing is running.
- **A pass deletes only what it created, because the owner's rule says an agent never deletes build caches or target directories it did not create, and a loop brief is not permission.** The first version of every `build-loop.md`'s "Clean up when you are done" section also had a pass remove earlier passes' clean, merged, idle worktrees and `*-target` directories; the owner was asked and chose own-mess-only. So a pass removes its own baseline pair and `/tmp` scratch and names each path in the log, and **lists** earlier passes' leftovers (path, size, clean or not, on `origin/main` or not, idle for two hours or not) for a person to approve. That keeps the cost of a killed pass visible rather than automatic: `git worktree list`, plus the `*-target` beside each worktree, is the inventory, which is why a baseline is a detached worktree paired with a `<path>-target`. If the leftovers should be cleared without a person, the way that respects the rule is a step in the operator-owned driver (like the park step), with the owner's explicit approval of its policy.
- **Retired loops leave their `target/` behind, and `du` overstates what deleting it frees.** Measured 2026-10-28: big's `/home` was 91% full (364 GB free) and a read-only inventory found about 600 GB of stale build output by `du`: a 376 GB `target/` in `~/bevy_debugger_mcp` (last written 2026-10-26, clone clean, nothing unpushed), a 141 GB `target/` in an old `~/bevy_elements` clone, and about 90 GB in copies of repos: ten `bevy_elements_iNN` and `_base` directories (about 71 GB; most are plain copies of the tree with no `.git`, one with a broken worktree link, left by elements passes from before that loop moved to big_mac), an 18 GB `~/bevy_lights` that held little but a `target/`, and two small ones. The owner approved nine named paths (the 376 GB target and 79 GB of the copies, 455 GB by `du`); `df` read 650 GB free straight afterwards, so **286 GB was freed, not 455**. The filesystem is btrfs with zstd compression and async discard, which could account for the gap, and nothing was done to isolate it. Not deleted, and not proposed: the 141 GB `target/` in `~/bevy_elements` (the clone has four uncommitted source files edited at 21:19 to 21:24 on 2026-10-27 and a release build from 23:35, made by something that is not an elements pass, since none of the recent passes touch big) and about 13 GB of copies from that evening. Judge a cleanup by `df` before and after, never by the `du` of what was removed. This is what the cleanup section in each `build-loop.md` is for: a baseline that is a detached worktree plus a `<path>-target` is something `git worktree list` can find, where a copy of the tree is not.
- **`wiggum.sh start` printed "started." on a Mac where nothing started, and four more things stand between a committed loop command and its first pass.** Found 2026-10-28 reviewing Sanctuary's `emerge-loop` (`WIGGUM_REPO=/Users/ladvien/sanctuary WIGGUM_COMMAND=emerge-loop`), which was written and never started. (1) `setsid` is util-linux and macOS has none, so the launch failed inside the background subshell, its error went to `driver.log`, and the script still printed "started."; `start` now falls back to `perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV'`, waits up to 10 s for the pidfile, and otherwise prints `FAILED to start` with the log tail and exits 1. That is tested by `tools/loop/test_wiggum_start.sh` (12 checks, run on macOS with bash 3.2); the park suite (57 checks) and the drain suite (23) in the same directory still pass. (2) The compiled-in default model was ollama-cloud, which answered 402 and 429; a start line that names no model would have run every pass against it, so the default is now `claude-sonnet-5-5`. (3) A checkout that people and other agents work in at the same time (Sanctuary's had 34 uncommitted files, six of them untracked `.blend`s, and notes from three agents in its coordination chat that day) must run with `WIGGUM_PARK=0 WIGGUM_CHECKPOINT=0` and a raised `WIGGUM_MAX_DIRTY` (new, default 40): with the defaults the park step commits everyone's unfinished files to a branch and returns the checkout to `main` under them, the checkpoint snapshots them into hidden refs, and the dirty guard aborts every pass above 40 files. A dedicated clone avoids all three, but that command hard-codes the live checkout's path and the agents' coordination file `AGENTS_CHAT.md` is gitignored. (4) The free-space gate: a pass is refused under `WIGGUM_MIN_FREE_GB` (default 150) after the driver prunes what cargo regenerates. The laptop had 11 GB free of 461 GB (98% used; `~/.cargo-target-shared/global` alone is 114 GB) and big_mac 74 GB, so on either the loop would abort every pass and stop itself after five consecutive failures. (5) One loop per repo: the process marker is `wiggum-$USER-<repo name>`, so two loops with different commands on one repo (Sanctuary's `build-loop` and `emerge-loop`) refuse to run together; `WIGGUM_STATE` can be set apart, the marker cannot.
- **Society's gate was CPU-bound by rayon, not by memory or core count, and folding the test binaries plus nextest took it from over two hours to about one.** Measured 2026-10-28 (PR #36 on `bevy-coven/bevy_society`; the table is in its `docs/loop/test-gate.md`): 74 of 81 test binaries became one, `tests/it`, and the gate is `RAYON_NUM_THREADS=1 cargo nextest run --profile gate`. Default-features tests went from 47.1 to 24.6 minutes and `--no-default-features` tests from 29.5 to 19.2, with clippy, fmt, doctests and rustdoc clean. A cold full sequence took 1 h 25 min; a warm one is estimated at 55 to 60 minutes, which was not measured. The first nextest run had used 7 h 11 min of CPU in 40 minutes of wall (load average above 150 on a 24-core machine shared with the loops): `bevy_stigmergy` runs a rayon pass every tick and rayon starts one worker per core, which cost 494 CPU-seconds against 104 for the same test with one thread, and was no slower. The first diagnosis was wrong: a one-thread Bevy task pool changed nothing, because a Linux thread inherits its creator's name and rayon creates its pool from inside a Bevy compute thread, so its 24 workers showed up as "Compute Task Pool" while `ComputeTaskPool::thread_num()` said 1. What found it was listing the process's threads after three ticks, not asking the pool how wide it was. Memory was never the constraint (eight tests held 197 MB in total), and a process starved of CPU on a loaded box shows almost no ticks per thread, so sample CPU time over a whole run. nextest 0.9.150's wrapper and setup scripts are still experimental, so the variable is on the command line. Lights, grimoire and elements have no rayon in their dependency graphs, so none of this applies to them.

## Templates

### systemd (`~/.config/systemd/user/wiggum@<name>.service`)

```ini
[Unit]
Description=<repo> Wiggum loop (one fresh <command> pass at a time)
StartLimitIntervalSec=86400
StartLimitBurst=8
Documentation=file:%h/<clone>/.omp/commands/<command>.md
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=%h/<clone>
ExecStartPre=-/usr/bin/pkill -x -f "Xvfb :97 -screen 0 1920x1080x24 -nolisten tcp -ac"
ExecStartPre=/bin/sh -c 'sleep 1; /usr/bin/Xvfb :97 -screen 0 1920x1080x24 -nolisten tcp -ac & sleep 2; test -S /tmp/.X11-unix/X97'
ExecStopPost=-/usr/bin/pkill -x -f "Xvfb :97 -screen 0 1920x1080x24 -nolisten tcp -ac"
ExecStart=%h/<clone>/tools/loop/wiggum.sh __driver
Environment=PATH=%h/.local/bin:%h/.cargo/bin:/usr/local/bin:/usr/bin:/bin
Environment=WIGGUM_REPO=%h/<clone>
Environment=WIGGUM_COMMAND=<command>
Environment=WIGGUM_MODEL=ollama-cloud/deepseek-v4.1-flash:1017
Environment=WIGGUM_THINK=medium
Environment=WIGGUM_SLEEP=60
Environment=WIGGUM_MAX=400
Environment=WIGGUM_PASS_TIMEOUT=21600
Environment=CARGO_BUILD_JOBS=6
Environment=DISPLAY=:97
Nice=19
IOSchedulingClass=idle
# **Bounded so a pass is throttled, not shot, and so oomd cannot pick it on swap.** oomd kills on a
# 90 % swap-used limit against the monitored ancestor `/user.slice/<uid>.slice`; a unit that can swap
# feeds that ratio, and at 2G both big loops were OOM-killed at their cap. `0` makes the unit
# contribute no swap at all. `MemoryHigh` reclaims under pressure and `MemoryMax` bounds the cgroup —
# keep both; the rule is to bound swap, never to remove the memory cap. `ManagedOOMPreference` belongs
# on the *slice* (`app-wiggum.slice`), not here: a unit-level setting is ignored because oomd monitors
# the ancestor.
MemoryHigh=6G
MemoryMax=7G
MemorySwapMax=0
KillSignal=SIGTERM
TimeoutStopSec=25200
Restart=on-failure
RestartSec=120
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
```

```sh
systemctl --user daemon-reload
systemd-analyze --user verify ~/.config/systemd/user/wiggum@<name>.service
systemctl --user enable --now wiggum@<name>.service
```

`loginctl enable-linger` must be on, or a user unit dies with the last session.

### launchd (`~/Library/LaunchAgents/com.ladvien.wiggum-<name>.plist`)

The committed pair in `tools/loop/` is the reference: `com.ladvien.wiggum-emerge.plist` (two-hour pass,
`CARGO_TARGET_DIR`) and `com.ladvien.wiggum-elements.plist` (six-hour pass). Substitute `/Users/USER`
before installing, and keep `KeepAlive` as `Crashed` only so `stop` is observable:

```sh
sed -i '' "s|/Users/USER|$HOME|g" com.ladvien.wiggum-<name>.plist
plutil -lint com.ladvien.wiggum-<name>.plist
cp com.ladvien.wiggum-<name>.plist ~/Library/LaunchAgents/
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.ladvien.wiggum-<name>.plist
launchctl kickstart -k gui/$(id -u)/com.ladvien.wiggum-<name>
```

`RunAtLoad` is `false` deliberately: installing an agent must not start a pass while a human is setting
things up.

## What the loops are, today

**Roster as of 2026-10-31 (supersedes the table below wherever they differ).** The owner's rule: **one loop per repo** ("we only want one item being worked on per loop, per repo; the multiple agents was just because we were trying to burn tokens"). bevy_grimoire's extra workers w2 to w8 finished the pass in flight and stopped (supervisors disabled, plists renamed `*.plist.retired`); `bevy_emerge_loop` is bevy_grimoire's one loop. Every loop runs Claude Sonnet 5.5 (`anthropic/claude-sonnet-5-5`, owner 2026-10-30 16:15: "switch them all to Sonnet five point five by Anthropic") at `WIGGUM_THINK=xhigh`, `WIGGUM_SLEEP=15`, `WIGGUM_IDLE_MAX=300`; the 4 p.m. burn ended when the weekly quota reset.

| host | loops |
|---|---|
| big | fairy, gooey, lights, society: `wiggum@<name>`, Xvfb, `oom_score_adj` 500 so a kernel OOM kill takes a loop's build and not HomeStill |
| big_mac | grimoire, elements, carnage, speech_bubbles, isomesh, stigmergy: `com.ladvien.wiggum-<name>` |
| bmb | orca: `com.ladvien.wiggum-orca`, `~/bevy_orca_loop`, `~/target/orca-loop`, floor 10 GB |
| laptop | flora: `com.ladvien.wiggum-flora`, `~/bevy_flora_loop`, `~/target/flora-loop` |
| sib | none: unavailable for days from 2026-10-30; gooey, orca, stigmergy and w8 left it (disable it in Herdr, above) |

**Moving a loop between hosts** (done five times on 2026-10-30/31): (1) let the pass finish, then stop and keep the unit down: `~/.local/share/wiggum/retire_at_boundary.sh <instance> <repo> <driver>` on Linux, `mac_boundary.sh retire <label>` on a Mac (`reload` for a plist edit; both wait with `drain --wait`); a host that is already gone is recovered from its `refs/wiggum/pass-N-checkpoint-*` refs instead, which the driver pushes every 10 minutes (put the newest on a `wiggum/parked-pass-N-<time>` branch and push it, the loop prompt looks for those first). (2) **Push the clone's local branch**: a pass's `loop/iter-N` branch can exist only in the clone, and the new host starts from GitHub (stigmergy's `loop/iter-3` and elements' `loop/iter-104` would have been lost). (3) Clone the repo on the new host, copy the driver to `~/.local/share/wiggum/wiggum.sh`, write the unit or plist from a sibling loop's (change repo, target dir, display, label, log dir), and rename a retired worker's target dir to `<repo>-loop` instead of building cold (`mv`, never delete without asking). (4) Remove a leftover `~/.local/state/wiggum/<clone>/drain` file, which makes the driver stop at once.

**What stopped loops overnight on 2026-10-30/31**, so it is looked for first: a disk below the loop's floor (`pass N ABORTED: 13GB free < 25GB after pruning`, five in a row stop the driver: big_mac was at 11 GB, bmb at 1.7 GB, because a moved loop's target grows 5 to 15 GB in a few hours); an elements pass that ran its 6-hour limit twice on a GPU sweep test; and big's RAM-backed `/tmp` filling to 16 GB with finished passes' scratch. `df -h` on every Mac and `df -h /tmp` on big come before any other diagnosis.

**bevy_grimoire runs as parallel workers** (owner, 2026-10-29): one clone per worker, all on `build-loop`, coordinated by `worker:<name>` issue labels (`build-loop.md`, "Parallel workers"). A worker's name is its clone's directory, unique across machines. GitHub Actions runs once a day; workers never wait for it.

| worker / loop | host | clone | supervisor | target dir | jobs | display | floor |
|---|---|---|---|---|---|---|---|
| `bevy_emerge_loop` (also owns non-issue work) | big_mac | `~/bevy_emerge_loop` | `com.ladvien.wiggum-emerge` | `~/target/emerge-loop` | 4 | native Metal | 25 GB |
| `bevy_emerge_w2` (records big's unrecorded clips) | bmb, from 2026-10-30 00:11 (moved off big_mac, whose 10 cores ran three loops: w2's gates and pushes timed out three passes running). Its unpushed commits came over as a git bundle | `~/bevy_emerge_w2` | `com.ladvien.wiggum-emerge-w2` (gui/501) | `~/target/emerge-w2`, an APFS clone of w7's | 4 | native Metal | 20 GB |
| `bevy_emerge_w3` (the GPU: #109) | big | `~/bevy_emerge_w3` | `wiggum@emerge-w3.service`, `MemoryMax=10G`, no swap | `~/target/emerge-w3` | 6 | Xvfb `:96` | 150 GB |
| `bevy_emerge_w4` | laptop | `~/bevy_emerge_w4` | `com.ladvien.wiggum-emerge-w4` | `~/target/emerge-w4`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only` | 4 | native Metal | 20 GB, above Sanctuary's 15 so this worker stops first |
| `bevy_emerge_w5` | big | `~/bevy_emerge_w5` | `wiggum@emerge-w5.service`, `MemoryHigh=6G`, `MemoryMax=8G`, no swap | `~/target/emerge-w5`, started as a btrfs reflink copy of w3's (`cp -a --reflink=always`), so no cold build | 4 | Xvfb `:94` | 150 GB |
| `bevy_emerge_w6` | bmb (account `rebekahbrittain`) | `~/bevy_emerge_w6` | `com.ladvien.wiggum-emerge-w6` (gui/501) | `~/target/emerge-w6`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only` | 8 of 14, leaving room for golem's FLUX jobs | native Metal | 20 GB |
| `bevy_emerge_w7` | bmb, beside w6 (48 GB of RAM, so memory has room) | `~/bevy_emerge_w7` | `com.ladvien.wiggum-emerge-w7` (gui/501) | `~/target/emerge-w7`, an APFS clone of w6's (`cp -c -R`), so no cold build | 6 | native Metal | 20 GB |
| `bevy_emerge_w8` | sib (CachyOS, 16 cores, 15 GB, RTX 3060), from 2026-10-30 | `~/bevy_emerge_w8` | `wiggum@emerge-w8.service`, `MemoryHigh=7G`, `MemoryMax=8G`, 2 GB swap, `RUST_TEST_THREADS=4` | `~/target/emerge-w8` | 6 | Xvfb `:92` | |
| Sanctuary `emerge-loop` | laptop | `~/sanctuary` (shared checkout) | `wiggum.sh start` | `~/.cargo-target-shared/global` | — | native | 15 GB |
| lights, society | big | `~/bevy_lights_loop`, `~/bevy_society_loop` | `wiggum@lights`, `wiggum@society` (society: `RAYON_NUM_THREADS=1` drop-in), `MemoryMax=7G`, no swap | | 2, 3 | Xvfb `:97`, `:98` | |
| fairy | big | `~/bevy_fairy_loop` | `wiggum@fairy`, `MemoryMax=8G`, no swap; driver is a copy at `~/.local/share/wiggum/wiggum.sh` (bevy_fairy has none in its tree) | `~/target/fairy-loop` | 3 | Xvfb `:95` | |
| elements | big_mac | `~/bevy_elements` | `com.ladvien.wiggum-elements` | | 6 | native Metal | |
| gooey | sib, from 2026-10-30 (planned for big, moved before its first pass: big was at load 73 on 24 cores with swap full) | `~/bevy_gooey_loop` | `wiggum@gooey`, `MemoryHigh=5G`, `MemoryMax=7G` (`memory.conf` drop-in after an OOM kill at 6G), no swap, Opus 5.5; driver is a copy at `~/.local/share/wiggum/wiggum.sh` | `~/target/gooey-loop` | 6 | Xvfb `:93` | |
| stigmergy | sib, from 2026-10-30 (owner: "add a wiggum loop to bevy_stigmergy"; its first item is #6, the rayon stall Sanctuary's perf loop filed) | `~/bevy_stigmergy_loop` | `wiggum@stigmergy`, `MemoryHigh=3G`, `MemoryMax=4G`, 1 GB swap, Opus 5.5; the same driver copy | `~/target/stigmergy-loop` | 4 | Xvfb `:91` | |
| orca, flora | big, from 2026-10-30 (owner: use the day's tokens) | `~/bevy_orca_loop`, `~/bevy_flora_loop` | `wiggum@orca` (`MemoryMax=4G`), `wiggum@flora` (`MemoryMax=7G`), no swap; the shared driver copy | `~/target/bevy_orca-loop`, `~/target/bevy_flora-loop` | 3 each | Xvfb `:89`, `:87` | |
| speech_bubbles, carnage, isomesh | big_mac, from 2026-10-30 (carnage learns its look from horror-film reference stills, owner's direction in its prompt) | `~/bevy_speech_bubbles_loop`, `~/bevy_carnage_loop`, `~/isomesh_loop` | `com.ladvien.wiggum-speech_bubbles`, `-carnage`, `-isomesh`; the shared driver copy at `~/.local/share/wiggum/wiggum.sh` | `~/target/<repo>-loop` | 3, 3, 4 | native Metal | 20 GB |

**sib** joined on 2026-10-30 (owner: "move some of the compilations to SIB"). It had cargo, rustup 1.98.1, Xvfb and linger already; it needed `omp` copied from big (the same x86-64 Linux binary) with big's `~/.omp/agent` config, `mcp.json` and `home-still-bridge.py`; **its own `omp login anthropic`** (a copied `agent.db` login worked for two hours, then big refreshed the shared OAuth token and every sib pass failed with "No API key found for anthropic": never copy a login between machines, log each one in); `gh auth login --with-token` with the owner's token plus `gh auth setup-git`, so clones are `https://` (its ssh key is not on GitHub); the `git-lfs` release binary in `~/.local/bin` (no sudo there); and an `hs cloud enroll` of its own (invite from `two`). Its login shell is fish, and an `ssh` inside a `ssh host 'bash -s' <<EOF` script reads the rest of the script as its stdin: give nested calls `-n`.

**Every loop runs flat out until further notice** (owner, 2026-10-30 11:10: "use all of our tokens by 4 p.m. Central"): Opus 5.5 at `WIGGUM_THINK=xhigh`, `WIGGUM_SLEEP=15`, `WIGGUM_IDLE_MAX=300`. On Linux it is one template drop-in, `~/.config/systemd/user/wiggum@.service.d/zz-burn.conf` (named `zz-` so it wins over per-unit `model.conf`); on the Macs it is each plist's `EnvironmentVariables`. A change reaches a running loop at its next pass boundary through `~/.local/share/wiggum/restart_at_boundary.sh <instance> <repo> <driver>` (Linux: `drain --wait`, then `systemctl --user restart`) or `reload_at_boundary.sh <label> <repo> <driver>` (macOS: `drain --wait`, then `bootout`, `bootstrap`, `kickstart`, because launchd re-reads a plist only on bootstrap). Keep those helpers in `~/.local/share/wiggum`, never `/tmp`: bmb's reboot at 09:19 wiped one from `/tmp`, and its workers came back on the old model.

**Herdr shows every loop on every machine in one window** (owner, 2026-10-30). Each loop host runs a headless Herdr server, session `loops` (`herdr-loops.service` on Linux, `com.ladvien.herdr-loops` on macOS), with one tab per **live** loop (2026-10-31: it was every loop that ran there in the last 12 hours, so a retired worker or a loop moved to another host kept a dead tab for half a day). A loop is live when the pid on its driver log's last `driver START` line is still a wiggum driver, or its log changed in the last 10 minutes (the gap before a supervisor restarts a driver). The tab runs `wiggum-pane.sh <loop> <driver.log>`, which runs `wiggum-chat.py`: **the pass's live chat** (owner: "expose the actual chat in each of those"). omp writes each pass's session as JSONL while it works (`~/.omp/agent/sessions/-<clone>/<time>_<id>.jsonl`, one line per thinking block, tool call, tool result and reply), so the view follows the clone's newest session, prints it as a readable chat (thinking in grey, each tool call with its intent and command in cyan, the first lines of each result, replies in bold), moves to the next session when the next pass starts, and prints each pass's START and END from the driver log. It reports the loop's state to Herdr (`working` while a pass runs, `idle` between passes, `blocked` on a failed or timed-out pass), and the session's title and latest tool intent as pane metadata. A sync job (`herdr-loops-sync.service` timer on Linux, `com.ladvien.herdr-loops-sync` on macOS, every 5 minutes) adds a tab for any new live loop and closes the tab of any that is not (never when nothing is live, because closing a server's last tab loses its workspace); `herdr-loops-sync.sh --fresh` rebuilds every tab after a change to the scripts. On the laptop, `herdr` opens the client: the sidebar holds the saved machines `big`, `sib` (disabled with `herdr machine disable <id>` while it is away; `enable` brings it back), `bmb`, `big_mac` and `laptop` (the laptop's own `loops` session, reached over ssh to itself), and `herdr --machine <name> agent list` or `pane read <pane> --source recent` answers from a script. On the Macs the server runs from `~/.cargo/bin/herdr` (a link to Homebrew's), because a non-login ssh shell's PATH holds `~/.cargo/bin` but not `/opt/homebrew/bin`, and a server started from a different path than the one ssh finds is treated as a foreign install. The scripts are in `docs/loop/herdr/` (`herdr-linux-setup.sh`, `herdr-mac-setup.sh` install a host; `herdr-mac-fix.sh` puts a Mac's server back under launchd). Nothing in a loop's supervisor changed: the view only reads driver logs and session files, so watching cannot steer or stop a pass.

Every bevy_grimoire worker and Sanctuary's grimoire loop run Claude Opus 5.5 (`anthropic/claude-opus-5-5`, owner, 2026-10-30: "we need to burn all of the tokens"), set as `WIGGUM_MODEL` in each worker's supervisor (the plist, or a `model.conf` drop-in for `wiggum@emerge-w3` and `-w5` on big) and in Sanctuary's `.omp/wiggum/emerge-loop.env`. The family loops ran Claude Sonnet 5.5 until 2026-10-30 11:10; since then every loop runs Opus at xhigh (above). **bmb** joined on 2026-10-29 (owner: "Use bmb. It's what it's there for."): it needed `brew install gh node`, `gh auth login --with-token` with the owner's token, an `hs cloud enroll` of its own (a copied `cloud-token` would rotate under the other machine), the home-still bridge copied to `~/.local/bin` with `mcp.json` pointing at this account's home, and 80 GB of stale build caches removed (`~/target/*`, `~/bevy_elements_i5*/target`); leave `~/llm` and `~/.cache/huggingface`, which hold the models golem's image jobs use. The machines not used: **one, two, three, four** are Raspberry Pi 4/5 boards (4 cores, 3–15 GB, no `omp`). Adding a worker: a fresh clone named `bevy_grimoire_w<N>`, its own target dir, `git -C <clone> config core.hooksPath tools/loop/hooks` (the push gate), a supervisor copied from a row above, a display no other loop uses, and the row here. Before its first pass, build in its own target dir with step 5's `cargo build -p bevy_cauldron -p bevy_cauldron_mcp` and `cargo test --workspace --no-run`, with the supervisor's own `CARGO_PROFILE_*` settings (a different profile rebuilds everything): on 2026-10-29 a cold build took w4's whole first two-hour pass and w2's first two passes. On btrfs (big), start the target as a `cp -a --reflink=always` copy of another worker's instead.

bmb also needs `brew install git-lfs` and `git lfs install` (found 2026-10-29 starting w7: without it a clone's checkout fails at the first LFS file, and `changelog.py check` refuses on a machine with no `git-lfs`).

**Every loop on big has three template drop-ins** in `~/.config/systemd/user/wiggum@.service.d/` (2026-10-29). The first two were added because big has 31 GB of RAM for HomeStill and five or more loops:

- `oom.conf`, `OOMPolicy=continue`: with the default (`stop`), one OOM-killed linker ended the whole pass (w5's passes 5 and 6).
- `debug.conf`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`: a Linux link reads every crate's DWARF, and under memory pressure fairy's test link re-read 148 GB from disk over 56 minutes. Changing it rebuilds every dependency once per target dir.
- `incremental.conf`, `CARGO_INCREMENTAL=1`, and the same variable in every Mac loop's plist: the workspace profile turns incremental off to save disk, so each edit recompiled all of `bevy_cauldron`. Under load on big_mac that compile took over 20 minutes. Every host now has 50 GB or more free, so the disk this costs is affordable. The env variable overrides the profile for this machine's loops only.

## Verify a loop is real, not just active

1. supervisor: `systemctl --user is-active wiggum@<name>` → `active`; `launchctl print gui/$(id -u)/…` →
   `state = running` with a pid.
2. the pass is the right one: `grep "pass 1 START" ~/.local/state/wiggum/<repo>/driver.log` →
   `command=/<command> ... dirty=0`.
3. nothing is driving a human's tree: `git -C <human checkout> status --short` unchanged.
4. **a pass reached its own gate once.** This is the only proof the loop can do anything; watch one pass
   to completion and read its own verdict, not just the exit code.
