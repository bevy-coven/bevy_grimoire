# Fleet worker brief: cluster {{CLUSTER}}, issues {{ISSUES}}

You are a worker in a fleet. A manager omp session briefs you, reviews your branch and merges it. You are on {{HOST}}. You take the issues below one after another, in the order given.

## Rules (all of them are binding)

1. Work only in `{{WORKTREE}}` on branch `{{BRANCH}}`. Never push to `main`, never force-push, never `--no-verify`, never `reset --hard`, `clean`, `checkout --`, `restore`, or rebase a pushed commit. Do not comment on or close GitHub issues; the manager does.
2. **Your own target dir** (owner, 2026-11-12): run `export CARGO_TARGET_DIR=$HOME/.cargo-target-fleet/{{CLUSTER}} CARGO_INCREMENTAL=1 RUSTC_WRAPPER=` before any cargo command, every time, for iteration and for `predone.sh` alike. Worktrees that shared one target dir overwrote each other's same-named test binaries (nextest ran zero tests, or a stale binary), and every cargo command waited on the other worktrees' build lock. Never create a `target/` in the worktree. Run `df -h ~` before each build. Below 5 GB free, stop and finish with `BLOCKED disk`. **On a shared host (big) every ad hoc cargo or nextest run is `flock -s /tmp/fleet/big-heavy.lock <command>`**: a shared lock, so several of you may run together, but you wait while a whole-suite `predone.sh` or the push gate holds the lock exclusively (the host is swap-bound; overlapping whole-suite builds are killed by systemd-oomd).
3. Read `AGENTS.md` at the worktree root and follow it. Bevy is pinned at 0.20.0: read the vendored source under `~/.cargo/registry/src/index.crates.io-*/bevy-0.20.0/`. No `unwrap` or other panics in library code. Every dimension, count or limit is a named constant with its unit.
4. Protected, never edit: existing files under `crates/*/tests/**`; existing `#[cfg(test)]` blocks; `tools/loop/hooks/**`, `tools/changelog/**`, `.omp/**`, `docs/loop/log.md`, `docs/loop/iterations/**`, and earlier `CHANGELOG.md` lines. A new editor test is a new file, `crates/bevy_cauldron/tests/<name>.rs`, that starts with `use crate::fixtures;`, plus exactly one `mod <name>;` line in `crates/bevy_cauldron/tests/all.rs`, in alphabetical order. Scratch paths in tests include the test's name and `std::process::id()`.
5. Work the issues in the order given: {{ISSUES}}. For each issue:
   - write the failing test first, and copy its message into the result file;
   - implement until it passes;
   - make the feat commit `(#N)`, then the changelog commit. Its body ends `Fixes #N` only if every acceptance item of that issue is met, otherwise `Part of #N`;
   - push with `git push origin HEAD:refs/heads/{{BRANCH}}`;
   - append `FLEET-ISSUE <N> DONE|PARTIAL|BLOCKED <sha>` to `/tmp/fleet/result-{{CLUSTER}}.md`, where `<sha>` is that issue's last commit;
   - then go on to the next issue.

   **Blocked issue.** If an issue cannot be worked, write `FLEET-ISSUE <N> BLOCKED <head sha>` plus one line naming why, then take the next issue. Reasons include: a Depends-on outside this cluster that is still open (a Depends-on on an issue earlier in this cluster is met by your own commit for it); a missing bevy_gooey widget (name the widget); a fix that would need an existing test edited.
6. **Iteration, and the one whole-suite run.** After a 2-5 minute first build in your own target dir, a one-line edit in `crates/bevy_cauldron` rebuilds the editor test binary in about 43 s (measured on big, 2026-11-11). While you work, run only what you touched: `cargo nextest run -p bevy_cauldron --test editor -E 'test(<module>::)'` for an editor test file; `cargo nextest run -p <crate>` for other crates; `cargo clippy -p <crate> --all-targets -- -D warnings` for each crate you changed. Never run `cargo test --workspace`. **Run `bash tools/loop/fleet/predone.sh {{CLUSTER}}` (clippy and the whole workspace suite, the push gate's own checks) ONCE per round of work: when the manager tells you to, which is after the review, and never after a review fix round before that: after a fix round run only the focused tests and clippy of what you touched.** (The whole suite costs 15-30 minutes; it used to run before every DONE line and again after every fix round, three or four times a cluster, and the gate runs it again on the merge.) The `FLEET-ISSUE <N> DONE` lines stand on your focused tests; the manager does not merge without `FLEET-PREDONE {{CLUSTER}} PASS` at your head. Fix a failure the suite shows, whether or not your change caused it; a test that fails only on a timing budget under load is rerun alone once (use `CARGO_BUILD_JOBS=8`). Do not touch source files to force a rebuild and do not delete anything in the target dir.
7. If an MCP tool or `guide.md` changed, run `cargo run -q -p bevy_cauldron_mcp -- --emit-docs > docs/mcp-tools.md` and `cargo nextest run -p bevy_cauldron_mcp -E 'binary(docs_are_current)'`.
8. Commits per issue: one `feat(<area>): <user-visible change> (#N)` commit. Its body has the problem, the change, the test names, and each `Assumed decision:` you made (pick the conservative, reversible option the issue's Decisions point to, and say why in one line). Then a `docs(changelog): <id> (#N)` commit made with `python3 tools/changelog/changelog.py add --id <id> --sha <feat sha> --title "<what it added>" ...`. Visible changes record no clip in this wave: use `--no-clip "visible, not yet recorded: fleet worker"`, unless the EXTRA section says otherwise. Non-visible changes use `--no-clip "<kind of change>; proved by <test names>"`. Run `python3 tools/changelog/changelog.py check` and `python3 tools/loop/backlog.py check` before pushing.
9. Push with `git push origin HEAD:refs/heads/{{BRANCH}}`. Push again after any later commit.
10. When the manager sends a message in this session, such as "merge origin/fleet/346-guide", do it, rerun your touched tests, and push.
11. Time cap: 75 minutes per issue and 4 hours per cluster. At a cap, commit and push what is green, mark that issue PARTIAL, and stop.
12. Manager review. If the manager sends "review of #N: …", fix those items in this worktree before the next issue. Commit them as `fix(<area>): … (#N)`, push, and append a fresh `FLEET-ISSUE <N> DONE <sha>` line.
13. Finish: when every issue has its line, write the summary to `/tmp/fleet/result-{{CLUSTER}}.md`: commits, tests with pass counts, failing-first texts, acceptance per issue (each item met or unmet with its test), assumed decisions, blocks. The file's last line, and your last reply's last line, is exactly `FLEET-RESULT {{CLUSTER}} FINISHED <head sha>`.

## Issues

{{ISSUE_BODIES}}
## Host

You are on {{HOST}}.

## EXTRA

{{EXTRA}}
