# Fleet: parallel workers in herdr

One manager and several workers work a cluster of related issues at once. The manager is one omp session (here, on mac_air). A worker is an omp agent on `ollama-cloud/deepseek-v4.1-flash:1017`, one per issue, one per host slot, opened in a herdr tab so the owner can watch it. The manager briefs the workers, reviews and merges their branches on big, pushes through the gate, publishes, and closes the issues. Workers never touch main and never comment on issues.

## Commands

```sh
python3 tools/loop/fleet/brief.py <issue> --branch fleet/<n>-<slug> --worktree /tmp/dark-incorporated/bevy_dark_factory-fleet-<n> --host <label> [--extra extra.md] --out /tmp/fleet-mgr/brief-<n>.md
tools/loop/fleet/launch.sh <host|local> <issue> <slug> <base-ref> <brief-file>
tools/loop/fleet/status.sh            # FLEET_MACHINES="local big bmb" by default
```

`brief.py` renders `brief.md` from an issue and refuses (exit 1, nothing written) when `gh` fails or a placeholder survives. `launch.sh` makes a worktree `/tmp/dark-incorporated/bevy_dark_factory-fleet-<issue>` on branch `fleet/<issue>-<slug>` on the host, copies the brief to `/tmp/fleet/brief-<issue>.md`, adds a tab `w<issue>` to the herdr workspace `fleet` (created if missing), starts omp there and prompts it to follow the brief. It never reuses or removes an existing worktree. A worker's finish is a `/tmp/fleet/result-<issue>.md` file whose last line is `FLEET-RESULT <issue> DONE|PARTIAL|BLOCKED <sha>`; `status.sh` prints those.

## Watching

Run `herdr`, choose the machine `big` or `bmb` (or the local session), then the workspace `fleet`: one tab per issue. From a script, `herdr [--machine <label>] agent read w<n> --source recent-unwrapped --lines 120`, and `agent wait w<n>` to block until it settles.

## Merge and gate

Workers push only `fleet/*` branches. `tools/loop/hooks/pre-push` gates `refs/heads/main` alone, so branch pushes are free. The manager merges branches into `fleet/integration` in a worktree on big, in dependency order, reviews each branch's diff first (no edited tests but new files and `mod` lines in `all.rs`; nothing under the protected paths), then pushes `HEAD:main` from big, where the gate is green. mac_air's own gate fails the terminal tests of `bevy_cauldron` (environment, identical on clean main). On big the shared `CARGO_TARGET_DIR` reuses test binaries built under another worktree path, so touch every `crates/**/*.rs` before the push.

## Hosts

- big: reachable by saved herdr profile; its login shell is fish, which is why `launch.sh` pipes a script into `bash -s`.
- bmb: needs `OLLAMA_API_KEY` in `~/.omp/agent/.env` (mode 600), or `omp models` there lists no DeepSeek. bmb's omp is on zsh's PATH, not bash's, so the pane is started with `exec zsh -l`.
- big_mac: excluded. Measured 2026-11: no internet, curl to ollama.com, github.com and 1.1.1.1 fails with "Can't assign requested address".
