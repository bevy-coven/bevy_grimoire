# Fleet: parallel workers in herdr

One manager and several workers work a cluster of related issues at once. The manager is one omp session (here, on mac_air). A worker is an omp agent on `ollama-cloud/deepseek-v4.1-flash:1017`, one per cluster of three or four related issues, one per host, opened in a herdr tab so the owner can watch it. The manager briefs the workers, reviews and merges their branches on big, pushes through the gate, publishes, and closes the issues. Workers never touch main and never comment on issues.

## Commands

```sh
python3 tools/loop/fleet/brief.py <issue>... --cluster <slug> --branch fleet/<slug> --worktree /tmp/dark-incorporated/bevy_dark_factory-fleet-<slug> --host <label> [--extra extra.md] --out /tmp/fleet-mgr/brief-<slug>.md
tools/loop/fleet/launch.sh <host|local> <cluster> <base-ref> <brief-file>
tools/loop/fleet/status.sh            # FLEET_MACHINES="local big bmb" by default
tools/loop/fleet/watch.sh             # blocks for the next FLEET-ISSUE / FLEET-RESULT / STALL line of /tmp/fleet-mgr/roster.txt
```

`brief.py` renders `brief.md` from a cluster of issues (each issue's number, title and body, in the order given) and refuses (exit 1, nothing written) when `gh` fails or a placeholder survives. `launch.sh` makes a worktree `/tmp/dark-incorporated/bevy_dark_factory-fleet-<cluster>` on branch `fleet/<cluster>` on the host, copies the brief to `/tmp/fleet/brief-<cluster>.md`, adds a tab `w<cluster>` to the herdr workspace `fleet` (created if missing), starts omp there and prompts it to follow the brief. It never reuses or removes an existing worktree. A worker reports in `/tmp/fleet/result-<cluster>.md`: one `FLEET-ISSUE <n> DONE|PARTIAL|BLOCKED <sha>` line as each issue is pushed, and last `FLEET-RESULT <cluster> FINISHED <sha>`; `status.sh` prints those. `watch.sh` reads `/tmp/fleet-mgr/roster.txt` (`<host|local> <cluster>` per line), blocks until a line not yet in `/tmp/fleet-mgr/seen.txt` appears and prints it, or prints `STALL <host> <cluster>` when a worker without a result has had an unchanged pane revision for 3 minutes (`herdr agent wait` and `agent_status` read idle while omp is mid-turn, so neither can supervise). A clone that fetches only `main` needs `git fetch origin "+refs/heads/fleet/*:refs/remotes/origin/fleet/*"` before it sees a fleet branch; `launch.sh` does that itself.

One worker per machine (owner, 2026-11-09): two workers on one host fight over the shared target's lock and the cores, so a host takes its next cluster only when its current worker has written `FLEET-RESULT`. `launch.sh` refuses a busy host. A worker stopped by hand gets `FLEET-RESULT <cluster> STOPPED <head sha>` appended by the manager.

## Watching

The Herdr app lists `hc` terminals (herdr-core), not the herdr-CLI sessions the workers run in, so `launch.sh` also starts an `hc` terminal `w<cluster>` under the manager's that attaches to the worker's pane (`ssh -t <host> herdr --session loops agent attach w<cluster> --takeover` for a remote host). A host whose own `hc` core has its registry writer stopped (it panicked on a full disk) cannot host them, so the viewer is always a local terminal. 

From the herdr CLI:

Run `herdr`, choose the machine `big` or `bmb` (or the local session), then the workspace `fleet`: one tab per cluster. From a script, `herdr [--machine <label>] agent read w<cluster> --source recent-unwrapped --lines 120`, and `agent wait w<cluster>` to block until it settles.

## Merge and gate

Workers push only `fleet/*` branches. `tools/loop/hooks/pre-push` gates `refs/heads/main` alone, so branch pushes are free. The manager merges branches into `fleet/integration` in a worktree on big, in dependency order, reviews each branch's diff first (no edited tests but new files and `mod` lines in `all.rs`; nothing under the protected paths), then pushes `HEAD:main` from big, where the gate is green. mac_air's own gate fails the terminal tests of `bevy_cauldron` (environment, identical on clean main). On big the shared `CARGO_TARGET_DIR` reuses test binaries built under another worktree path, so touch every `crates/**/*.rs` before the push.

## Hosts

- big: reachable by saved herdr profile; its login shell is fish, which is why `launch.sh` pipes a script into `bash -s`.
- bmb: needs `OLLAMA_API_KEY` in `~/.omp/agent/.env` (mode 600), or `omp models` there lists no DeepSeek. bmb's omp is on zsh's PATH, not bash's, so the pane is started with `exec zsh -l`.
- big_mac: excluded. Measured 2026-11: no internet, curl to ollama.com, github.com and 1.1.1.1 fails with "Can't assign requested address".
