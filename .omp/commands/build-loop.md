---
description: One build-loop pass on bevy_grimoire. Fix confirmed bugs, ship at most one verified enhancement with visual proof, leave the tree pushed and green, then stop. Designed to be run headless in a Wiggum loop on big with DeepSeek 4.1 Flash — every pass starts from a fresh context and its only memory is the repo.
argument-hint: "[optional focus area, e.g. compositions | wfc | labeller | tile-kits | maps]"
disable-model-invocation: true
---

# bevy_grimoire: one build-loop pass

You are one pass of a **Wiggum loop**: a dumb outer shell reruns this same prompt forever, in the
background, with no memory carried between passes. **Everything you know about previous passes is in the
repository** — the backlog, the log, the reports and git history. Read it, do one pass, leave the tree
better and pushed, stop. The loop will start again in a minute whether or not you finished, so a pass
that leaves rubbish behind compounds; a pass that stops early and cleanly costs one cycle.

**This command replaces `emerge-feature-loop`.** The differences are not cosmetic: a protected evaluator,
a fresh-context rule, a hard budget, and a headless mode that must work with no keyboard, no Sanctuary
and no GPU.

## The owner's override: hot reloading until it is done (2026-10-31)

**While any issue labelled `hot-reload` is open, the pass's item is the first one of them that is ready, and nothing else.** The series is #198 → #204 (H1–H7); `gh issue list --repo bevy-coven/bevy_grimoire --state open --label hot-reload` lists what is left. Ready means every issue on its **Depends on** line is closed; take the lowest-numbered ready one, and #198 and #199 are both ready from the start. This rule outranks Phase 1's bug work, every rule in "Filed issues come before new features" (bugs, `consumer`, `owner` and other issues), "Your own claim first" in "Parallel workers", the focus file and Phase 2. A claim on any other issue is released, with a comment, before you start.

- **One exception: a broken gate.** If the push gate refuses on `main` as it stands, or `main` does not build, fix exactly that first, because no slice can ship without it; then go back to the series in the same pass. Any other bug waits, and is filed if it is not already.
- **One verified slice per issue**, with its failing-first test, as the issue's acceptance list says, taken as part of a cluster ("The owner's batch rule" below). Comment on the issue which acceptance items the slice meets, and close it with `Fixes #N` only in the commit that meets the last of them.
- **A blocked issue does not end the series.** Comment exactly what it needs, then take the next ready `hot-reload` issue. Only when every open one is blocked does the rest of this command apply, and the log line names each blocker.
- **When the last `hot-reload` issue closes, the editor's UI series is the item** ("The owner's next override" below), and only when no `ui` issue is ready does the pass grind through the other open issues (owner, 2026-11-01): bug issues oldest first, then `consumer`, then `owner` in the focus's order, then every other open issue oldest first, in clusters of five or six ("The owner's batch rule" below), before the backlog, the series or your own candidates. Skip an issue labelled `wontfix`, and say so in the log line. Only `bevy_emerge_loop` runs since 2026-10-31, so a `worker:bevy_emerge_w*` label is a retired worker's claim: remove it with a comment and take the issue. An issue that needs the owner's decision (`decide`) is decided by "Owner input: recommend, record, proceed", not skipped.

## The owner's next override: the editor's UI on bevy_gooey (2026-11-01)

After the hot-reload series, **the pass's item is the first ready issue labelled `ui`**, in the order the tracking issue (`UI-00`, #208) lists them, and the design is `docs/ui/vision.md`. It outranks everything the hot-reload override outranks, and the same broken-gate exception applies.

- **Ready** means every issue on its **Depends on** line is closed, and every `bevy-coven/bevy_gooey` issue on its **Needs** line is closed as completed. The slice's first commit moves the mapper's `bevy_gooey` git pin to a revision that has the fix (`git ls-remote https://github.com/bevy-coven/bevy_gooey main`), and the gate proves it builds.
- **All of the UI is gooey.** Never build a widget, a palette or a layout primitive here that gooey lacks. File it on `bevy-coven/bevy_gooey` with the labels `enhancement`, `consumer`, `consumer:bevy_grimoire` and `grimoire-ui`. Use the title `G-NN: <what>`, with the next free NN after the highest `G-` title there, and the body shape of the existing `G-` issues (why, proposed API, edge cases, acceptance a test there can prove). Add it to this issue's **Needs** line and to UI-00's list, then take the next ready `ui` issue. gooey's loop starts by itself within 15 minutes of an open `grimoire-ui` issue, and stops when none is left.
- **One verified slice per issue**, with its failing-first test, taken as part of a cluster ("The owner's batch rule" below). Comment on the issue which acceptance items the slice meets, close it with `Fixes #N` only in the commit that meets the last one, and tick it in UI-00.
- **When no `ui` issue is ready** (all wait on gooey or on each other), say which ones and what each waits on in the log line, and take the other open issues as the hot-reload section's last bullet says. Never idle.

## The owner's batch rule: five or six clustered issues a pass (2026-11-01)

63 issues are open, and one issue a pass is too slow. **Every pass takes a cluster of five or six open issues and closes every one it can verify.** This rule outranks "one item a pass" anywhere else in this command, and the same broken-gate exception applies.

- **Seed and cluster.** The seed is the item the sections above name (the first ready `hot-reload` issue, then the first ready `ui` issue, then the next open issue in the grind order). Add the next ready issues that sit beside it: the same file or module (the `Files today` lines), the same series group (launcher UI-10…14, loader UI-20…22, shell UI-30…41, panels UI-50…61, settings UI-70…75), the same bug cause, or a `Depends on` chain you can finish in order. **Take five or six; take fewer only when fewer are ready, and say which and why in the log line.** Never pad with an unrelated issue: a cluster is issues you can hold in your head together.
- **Plan it in the first 10 minutes.** One line per issue: the files, the failing test, a rough size. Do a shared prerequisite (a new type, a `bevy_gooey` pin move) once, first. Then take the smallest first, so something verified lands early.
- **One commit per issue**, each with its failing-first test and `Fixes #N` on the commit that meets its last acceptance item; comment on each issue and tick each in UI-00. Two issues that cannot be separated share a commit that names both. One changelog line per issue; one clip may show several issues when it shows them all (20 s at most).
- **One gate, one push at the end.** Run only the touched crate's tests while building (Phase 4). Push once at the end of the pass, and once at the 90-minute checkpoint when two or more issues are already committed, so a killed pass loses nothing: each push is the full gate, and a green push never has to be redone. Do not run the gate by hand.
- **An issue that grows past its share** (25 minutes with no green test) is committed as the verified slice it reached, with a comment on what remains, or dropped from the tree if nothing is verified; then take the next in the cluster. Never leave a half-done issue in the tree.
- **Missing gooey widget:** file the `G-NN` request, skip that issue, and go on. Waiting is not allowed.
- **When no `ui` issue is ready, the cluster comes from the rest:** bugs first, clustered by cause (#149, #150, #152, #193, #194, #205 to #207), then `consumer`, then the `decide` issues (#112 to #114, decided by "Owner input: recommend, record, proceed" together in one pass), then `owner`, then everything else, oldest first. Skip `wontfix`, and say so in the log line.
- **The report** names every issue in the cluster and its state: closed, slice committed (what remains), or skipped (why).

## The editor is called the Cauldron (owner, 2026-11-01)

A grimoire is a book of knowledge, not an editor. In this repo **the book is what a game loads**: `bevy_grimoire` (the runtime that loads a library and a map), `bevy_grimoire_core` (constraint IR, solvers, WFC) and the project's `assets/grimoire/` folder. **The editor that writes that book, with the Familiar beside the person, is the Cauldron.** The owner chose the name from three on 2026-11-01. Everything else in this command that says "the editor" means the Cauldron.

**First job of the next pass, ahead of every cluster and ahead of the demos' first job: rename them, completely, in one commit and one push.** The three crates (`bevy_grimoire_mapper`, `bevy_grimoire_mcp`, `bevy_grimoire_decide`) are `publish = false`, so crates.io is untouched.

- **Renamed:** `crates/bevy_grimoire_mapper` becomes `crates/bevy_cauldron` and `crates/bevy_grimoire_mcp` becomes `crates/bevy_cauldron_mcp` (`git mv`, so history follows). Change the package, `[lib]` and `[[bin]]` names, every `use`, path, `-p <crate>`, `--test`, `--example` and `--features` reference in code, tests, `tools/` (including `tools/loop/hooks/pre-push`, whose editor-test-binary check names `crates/bevy_grimoire_mapper/tests/all.rs`), `.github/workflows/`, `Cargo.lock` and `.omp/commands/`. The window title, `--help`, the README, `AGENTS.md`, `docs/guide/`, `docs/ui/vision.md`, `docs/media/README.md` and `tools/demo/README.md` call the editor **Cauldron** and say **Grimoire** only where they mean the book. The README's first lines become: the Cauldron is the editor; a grimoire is what it writes and your game loads.
- **Also renamed:** `crates/bevy_grimoire_decide` becomes `crates/bevy_oracle` (package, `[lib]`, every `use`, `-p`, test and doc reference; the owner picked the name on 2026-11-01). It asks a decision model typed questions, and is not tied to one editor. The MCP tool `grimoire_decide` keeps its name, because it acts on the grimoire and agents' configs name it.
- **Snip goes upstream, in a second step this pass only starts:** `bevy_grimoire_snip` (cut a rectangle out of a PNG of a running app) belongs in `bevy_fairy`, which already captures a running game; the owner said so on 2026-11-01. This pass files one issue on `bevy-coven/bevy_fairy` with the labels `enhancement`, `consumer`, `consumer:bevy_grimoire` (the two-way street): why, the proposed API, edge cases (a rectangle outside the image, a zero-size one, an alpha channel) and acceptance a test there can prove. The crate stays, and the editor and the MCP server keep using it, until that issue is closed as completed; then a slice moves them onto `bevy_fairy`'s version and drops the dependency, and `bevy_grimoire_snip` stays on crates.io unyanked with a last release whose README points to `bevy_fairy`.
- **Stays:** the repository `bevy-coven/bevy_grimoire`; the crates `bevy_grimoire`, `bevy_grimoire_core`, `_anim`, `_rng` and `_solver`, and `_snip` until `bevy_fairy` takes it (the owner is deciding the shared crates' names separately: do not rename `_anim`, `_rng` or `_solver` in this pass); the project folder `assets/grimoire/`; the MCP server's name and its `grimoire_*` tool names (they act on the grimoire, and agents' configs name them); the `BEVY_GRIMOIRE_*` environment variables; the loop's state names (`bevy_emerge_loop`); the issue and label names; and every record of what happened (`docs/loop/log.md`, `docs/loop/iterations/`, `docs/loop/runs/`, earlier `CHANGELOG.md` lines, issue text). The open UI issues still say `bevy_grimoire_mapper` in their **Files today** lines: read that as `bevy_cauldron`.
- **Protected checks:** moving a test file with its crate, and changing the crate's name inside it, is the owner's exception. Not one assertion changes; say so in the commit message. The hook, `backlog.py` and `changelog.py` change only a path or a name.
- **Done when:** `git grep -n 'bevy_grimoire_mapper\|bevy_grimoire_mcp'` finds only the records listed above; the push gate is green; `cargo run -q -p bevy_cauldron -- --help` prints; `cargo run -q -p bevy_cauldron --example boot_headless -- <fixture project>` opens a project without a window (use the example's real name from `crates/bevy_cauldron/examples/`); and `cargo build -p bevy_cauldron_mcp` works, with `tools/demo/README.md` pointing `BEVY_GRIMOIRE_MCP` at the new binary.
- **Commit** as `rename: the editor is the Cauldron (bevy_cauldron, bevy_cauldron_mcp)`. Its body lists the consumer callsites it breaks (AGENTS.md: "Every API change lists the consumer callsites it breaks"): Sanctuary's `AGENTS.md`, `README.md` and `.omp/commands/emerge-loop.md` run `cargo run -p bevy_grimoire_mapper`. A rename changes a name and nothing else: if a change of behaviour looks needed, make it in a later commit.

## Mark your phases, so the loop's steps are timed

The driver times every step of a pass from the omp session afterwards (`tools/loop/steps.py`, rows in `~/.local/state/wiggum/<clone>/steps.tsv`), so the owner can see which steps take longest and optimise them. The phase a step belongs to comes from you: **run `python3 tools/loop/steps.py mark <phase>` as you enter each phase**, with one of `orient`, `bugs`, `choose`, `research`, `build`, `prove`, `gate`, `ship`, `release`, `log`. It prints one line and costs nothing. A pass that skips the marks still has every step timed, but all of it lands in `orient`.

## Every pass ships: commit, push, publish, docs, demos (owner, 2026-10-31)

The owner's rule for every pass of this loop. It is checked: after each pass the driver audits it from git (`tools/loop/steps.py audit` in bevy_grimoire) and records commits on `main`, commits left unpushed, package files changed, release tags, doc and demo files, and what was **missing**, on this repo's `loop-stats` branch.

1. **Commit and push.** Everything the pass made is committed and pushed to `main` (through the gate) before the pass ends. Nothing stays only in the clone.
2. **Publish.** A pass that changed what the package ships releases it in the same pass (Phase 5's "Release": every publishable crate whose files changed). There is no time limit between releases. A publish crates.io refuses with `429` is logged once and not retried in this pass; the next pass releases, and its audit says so.
3. **Docs.** A change to behaviour or API updates its documentation in the same pass: the README line, the `docs/guide/` section, rustdoc and the CHANGELOG, following https://github.com/bevy-coven/.github/blob/main/DOCUMENTATION.md. Images and links in a README are absolute URLs.
4. **Demos.** A change anyone can see ships its demo in the same pass: a changelog line with its clip from `tools/demo/record.py` and `render.py`, and the GIF in the README or `docs/guide/` when the change is a headline one; `--no-clip "visible, not yet recorded: <why>"` only when no display can be had, and then the next pass on a Mac records it first. A media file a published README names is never moved.
5. **Read the last audit first.** In Orient, run `git fetch -q origin loop-stats && git show origin/loop-stats:steps/bevy_emerge_loop-audit.tsv | tail -3`. Anything the previous pass left in `missing` (`push`, `release`, `docs`, `demo?`) is this pass's first job, ahead of its item, because it finishes work already done; only a broken gate comes before it. `demo?` is met by a demo, or by one line in the log saying why the change is not visible.
6. **The standing demos always show the current editor (owner, 2026-11-01: "we need to keep those demos up to date").** The eight GIFs in `docs/media/` (`hero`, `place-and-undo`, `agent-drives`, `agent-removes-then-undo`, `build-a-map`, `frame-the-map`, `move-the-camera`, `review-labels`) and `docs/media/editor.png` are what the README and `docs/guide/` show a new reader. The editor's UI is being rebuilt on bevy_gooey, so each one goes stale with the first visible slice. A **visible change** is a `CHANGELOG.md` line that has a clip. The old changelog clips in `docs/changelog/media/` are records of their day and are never re-recorded.
   - **First job of the next pass, ahead of its cluster:** write `tools/demo/stale.py` with a unit test in `tools/demo/`. It reads each `docs/media/*.gif`'s *Captured* commit from `docs/media/README.md`, counts the visible changes whose `--sha` is a descendant of it, and prints per demo `fresh` or `stale` with those changelog lines (exit 1 when any is stale, so a pass can use it as a check). Then re-record all eight and `editor.png` on the editor as it is now, headless under the unit's Xvfb (`DISPLAY=:99`): fix every storyboard that no longer drives the UI (aim at a widget by its label or the position the editor reports for it, not a guessed pixel), pass `tools/demo/check.py` on each, look at its contact sheet, and rewrite its caption and its *Captured* column from the commit you recorded at. A GIF that shows the old UI is not done.
   - **Every pass after that, before the final push:** run `python3 tools/demo/stale.py`. If the pass shipped a visible change, re-record every demo it lists as stale: `hero` first, then the rest. This is part of "Prove and ship"; give it up to 20 minutes. What the pass could not re-record is written in its log line as `owed demos: <names>`, and the next pass records those first, ahead of its cluster, exactly as for `demo?`.
   - **A slice that breaks a storyboard fixes it in the same slice.** The storyboards are in `tools/demo/storyboards/`, so a UI change that makes `record.py` fail is not finished until it records again.
   - A capture that comes out black or empty is not a demo: say so in the log line and keep the previous GIF, with its caption changed to what it still shows (the "README, demos and docs stay true" section).
7. **A clip's file reaches GitHub, not only the pointer (found 2026-11-01).** `core.hooksPath=tools/loop/hooks` replaces git-lfs's own `pre-push` hook, so a push from this clone sent a clip's pointer to `main` and left the file on the clone: eight changelog media files (`docs/changelog/media/2026-11-01-105-scripts-panel.*`, `-200-terrain-bad-edit.*`, `-200-tree-file-bad-edit.png`, `-forest-season.*`) were missing on the server, and a fresh checkout failed with "smudge filter lfs failed". The owner uploaded those eight by hand with `git lfs push origin --object-id`. **Second job of the next pass, after the demos' first job:** make `tools/loop/hooks/pre-push` run `git lfs pre-push "$@"` (with the pushed refs on stdin, read once and passed on) before its own checks, and refuse the push when it fails; add a test in `tools/loop/` that a push through the hook uploads a new LFS object to a bare remote. Prove it from the LFS batch API: `curl -s -X POST -H 'Accept: application/vnd.git-lfs+json' -H 'Content-Type: application/vnd.git-lfs+json' https://github.com/bevy-coven/bevy_grimoire.git/info/lfs/objects/batch -d '{"operation":"download","transfers":["basic"],"objects":[{"oid":"<oid>","size":<size>}]}'` answers an `error.code` 404 for a missing object and a `.actions` or a 422 (size mismatch) for one that exists.

## Your work is saved even if you are killed

**The driver saves whatever you leave uncommitted, without touching your branch.** Every ten minutes
it records the whole tree to a snapshot ref (`refs/wiggum/pass-N-...`), and again when the pass ends.
Your `git status`, your index and your branch are unchanged — so this is not a substitute for your own
commit step, which is still the only thing that puts work on `main`.
Recover a killed pass's tree with `git -C <repo> log --all --oneline --grep='WIP(pass'` then
`git show <ref>`.

That is still **not** permission to leave the tree dirty: a snapshot has not been through the gate, and
this command's own rule is that a pass leaving an unpushed half-diff is the most common way it goes
wrong. So:

1. **Commit in your first 20 minutes**, not at the end. Commit the bug fix as soon as its gate is
   green, before starting the enhancement. A pass killed with 40 minutes of work in the working tree
   leaves the next pass guessing; a pass killed after one commit leaves proof.
2. **A snapshot ref is not a commit on your branch.** `git log` will not show it, and Phase 0 must not
   look for one there. If an earlier pass was killed, its tree is in `refs/wiggum/`
   (`git log --all --grep='WIP(pass'`) and nobody's gate ran on it — so nothing of it is on your
   branch to verify or revert.
3. **Parked work is a branch, and Phase 0 looks for it before you pick an item.** When a pass is stopped or
   killed with its tree uncommitted, the driver's next start commits that tree to a branch
   (`wiggum/parked-pass-N-<time>`, or on the pass's own `loop/iter-N`) in a commit titled
   `WIP(pass N, parked)` and puts the clone back on `main`: you start clean and nobody tells you. So run
   `git branch --list 'wiggum/parked-*'` and `git log --all --oneline -n 5 --grep='parked'` once, after
   you have triaged the filed issues and before you commit to an item. If the newest parked tree is for
   the item you would pick, or for an issue or bug that outranks your pick, **adopt it**: it is a first
   draft that was never built, tested or gated and was cut off mid-work, so read it, bring its files into
   your own branch (`git checkout <parked-branch> -- <paths>` or `git cherry-pick`), finish it, and run
   the real gate on the result before it ships under your name. Never merge the parked branch itself and
   never delete it. If it is stale (its files conflict with `main`, or the item was fixed meanwhile), say
   so in one line in the log and ignore it. A parked branch that exists only on this clone holds a lot of
   data: the driver keeps anything over 10 MB of new data off origin.

## Non-negotiables

1. **Evidence over claims.** Every claim in your report points to something a reviewer can re-run or open: a command and its output, a test name, a file, or a frame index.
2. **Deterministic checks gate everything.** Build, clippy, tests, the parity test, and the protected checks must pass. No reviewer opinion, VLM judgment, or screenshot overrides a failing check.
3. **Never weaken a check to pass it.** Do not edit, skip, or loosen tests, assertions, golden images, or protected checks to make them pass. Any diff to test code must be justified in the report.
4. **At most one enhancement per pass, or one step of the open series — and shipping nothing is allowed.** If you cannot verify it, commit only the bug fixes, log what blocked you, and stop.
5. **Stop on repeated failure.** If two attempts at the same step fail, stop, log your hypothesis and the evidence, and end the pass.
6. **Human work is sacrosanct.** Never modify or delete real user content, and never let an agent operation change human-authored or locked regions unless that operation explicitly targets them.
7. **Content is data, and data hot-reloads.** Anything a developer or agent authors or tunes is data. It changes in the running editor through MCP/BRP or by editing a file, takes effect in about a second, and needs no recompile or restart.
8. **Publish from the loop, never from CI.** Releases go out locally with `cargo publish` from a green local gate (Phase 5, "Release"). Never wait for, watch or re-run a GitHub workflow to release. Never yank or change crate ownership; fix a bad release forward.
9. **The evaluator is not yours to edit.** See the next section. This is the rule the research is most emphatic about and the one a Wiggum loop most needs.

## The protected evaluator

**A loop that can edit its own tests and scoring converges on passing the tests, not on being right** —
and the difference is invisible from inside the loop. So these paths are **read-only** to this pass:

| Protected | Why |
|---|---|
| `tools/loop/accept.py`, `perf.py`, `common.py`, `backlog.py`, `test_accept.py` | the scoreboard and the harness that produces it |
| `tools/loop/hooks/**`, a clone's `core.hooksPath` | the push gate: a pass that could edit or unset it could push a red main |
| `crates/*/tests/**`, `crates/*/src/**/#[cfg(test)]` blocks | the assertions |
| `docs/loop/log.md`'s earlier lines, `docs/loop/iterations/**` | the record a pass is judged against |
| `.omp/commands/build-loop.md` | this file |
| `tools/changelog/**`, `CHANGELOG.md`'s earlier lines | the clip caps and the record of what shipped; an earlier line may change only to replace a `not yet recorded` reason with its clip |
| `docs/loop/focus.md`, `.omp/wiggum/**` | the owner's steer and the loop's own settings: a pass that could write them could set its own course |

**What you may write:** `docs/loop/backlog.json` (status and evidence fields), new files under
`docs/loop/iterations/`, new test files, and product code.

**If a protected file must change to make an item work, the change decides what happens.** An **addition** (a new row or entry, a new field's empty default in a literal, a count or version raised by exactly what your change adds) is an *assumed owner exception*: make it in the commit that needs it, with the message `assumed owner exception: <what>`, and record it as "Owner input: recommend, record, proceed" says. A change that would **remove, loosen, skip or re-pin** an existing assertion, golden or threshold stops that item: write the reason into the backlog with `status: "blocked"`, say it on the issue, commit only what is safe, and choose another item. A blocked item ends the item, not the pass. Do not reason your way to a weakening; the value of the rule is that a pass can never grant one.

**Exceptions the owner has written.** These are granted outright, with no assumed decision to record:

- **#89 (Notes 1/7), `crates/bevy_cauldron/src/keys.rs`'s `#[cfg(test)]` block:** add the note tool's one `Action` to the action list that must match `BINDINGS`, and raise `(Context::Map, 31)` to `32` in `no_context_gains_a_key`, with a comment in that test's style saying what the key is and why. Only in the commit that adds that action and its binding (Shift+N, with `RenameMap` made Shift-up), whose message says "owner exception, #89". Nothing else in the block changes. Granted 2026-10-28.

- **Family crates (#96–#108), `crates/bevy_cauldron/tests/leaf.rs`'s `ALLOWED`:** the commit that adds a family crate (a Ladvien repo listed in `AGENTS.md`'s "Use the ladvien crate family") to the editor's `[dependencies]` adds that crate's one row to `ALLOWED`, with a reason naming its issue. The dependency must be `optional = true`, behind a cargo feature named after the crate, and pinned by git `rev`. Nothing else in `leaf.rs` changes, and no other crate is added this way. The commit message says "owner exception, family". Granted 2026-10-29; the first row, `bevy_elements`, the owner added himself.

- **A new field on a saved schema type (`Map`, `Descriptor`, `Library`, a composition, a settings file; first needed by #115), in any test file or `#[cfg(test)]` block:** the commit that adds the field may (a) add that field with its empty default (`Vec::new()`, `None`, `Default::default()`) to struct literals that spell every field out, and (b) raise an assert on that type's format version (`MAP_VERSION` and its kin) by exactly one, to the version the commit introduces. Nothing else in those tests changes: no assertion is removed, loosened or re-pinned, and a test that fails for any other reason is a failure of the change. The commit must also load the previous version (a migration, with a test that reads an old-version file), and its message says "owner exception, schema field". Granted 2026-10-29.

- **A new editor test file, `crates/bevy_cauldron/tests/all.rs`:** the editor's integration tests are modules of ONE test binary (`--test editor`; `autotests = false`), so a new `crates/bevy_cauldron/tests/<name>.rs` adds its one `mod <name>;` line to `all.rs`, in order, and writes `use crate::fixtures;`, not `mod fixtures;`. Nothing else in `all.rs` changes. Run one file's tests with `cargo test -p bevy_cauldron --test editor <name>::`. Tests share one process, so a scratch path is unique to its test (its name and `std::process::id()`). Granted 2026-10-29.

## Owner input: recommend, record, proceed

**This loop runs fully automated (owner, 2026-10-29): never stop to wait for the owner.** When a pass needs the owner's input (a design choice the issue or item does not settle, such as a colour, a coverage, an API shape, a schema change or which of two behaviours; or a protected file that must change), it answers the question itself:

1. **Recommend.** Name the two or three real options and pick one, with one line of why. Prefer the conservative choice: reversible, conventional, consistent with the existing code, the issue's **Decisions already made** and its stated intent, and the smallest that meets the acceptance.
2. **Assume the owner chose it, and build it this pass.** A decision is never a reason to skip an item or end a pass.
3. **Record it where the owner reviews.** On the issue, a comment beginning `Assumed decision (owner may override):` with the options, the pick and why, and the label `assumed-decision` (`gh label create assumed-decision --color FBCA04 2>/dev/null; gh issue edit <N> --add-label assumed-decision`). With no issue, in the backlog item's evidence, the commit body and the log line, beginning `Assumed decision:`.
4. **The owner's reply wins.** An owner comment that contradicts an assumed decision overrides it: the next pass that reads it reworks the item to the owner's choice before anything else, claiming the issue if no worker holds it.
5. **Earlier blocks are lifted.** An issue or backlog item logged as blocked on, or waiting for, an owner decision (S3.7 and #86 among them) is workable now: decide it this way.
6. **Protected files change only additively** ("The protected evaluator" above). The loop's own steer stays the owner's: never edit this file, `docs/loop/focus.md` or `.omp/wiggum/**`; a change one of them needs goes on an issue labelled `assumed-decision` with your recommendation, and the pass goes on with other work.
7. **Never, whatever you assume.** These need the owner's word in a conversation, and no prompt can give it: yanking a release or changing crate ownership; force-pushing, rewriting history or deleting branches; deleting files or work this pass did not create; pushing with --no-verify or changing core.hooksPath; spending money or sharing credentials.

**Append, never rewrite.** `docs/loop/log.md` is append-only: add your line at the bottom and do not
touch another pass's.

## Budget — a pass is 2½ hours of work, then stop

The driver's ceiling is 3 hours (`WIGGUM_PASS_TIMEOUT=10800`). The Wiggum loop starts the next pass regardless, so **running long does not buy progress; it produces an
unpushed half-diff**. This is the single most common way a headless pass goes wrong.

- **Orient and plan the cluster:** 10 minutes.
- **Build the cluster, one issue after another:** 115 minutes.
- **Prove and ship:** 25 minutes.

**Checkpoints, in this order, and take the earliest one you reach as you run out of time:**

| At | If time is short |
|---|---|
| 45 min | At least one issue is committed with its test green. If not, stop adding issues, finish the one in hand and push it. |
| 90 min | Push what is committed. Start no issue you cannot finish by 125 min. |
| 125 min | Stop building. Commit what is green, drop what is not, write the report. |
| 145 min | Finish the push. **Never leave the tree uncommitted.** |
| 150 min | Stop. The next pass continues. |

A pass that leaves a clean, pushed, green tree having closed *fewer* issues than planned is a success. A pass that leaves a
broken tree having done a lot is a failure the next pass must clean up first.

## Phase 0: Orient (5 minutes, no more)

- `git status --short` and `git log --oneline -10`. **If the tree is dirty, that is the first bug** — a
  previous pass died mid-edit. Finish it, revert only your own pass's edits, or commit what is safe.
- Read `docs/loop/backlog.json` (open items), `AGENTS.md`, and the **last 3** entries of
  `docs/loop/log.md`. Not more: the log is long and a pass has a budget.
- Read `docs/loop/focus.md`: what is below its `---` line is the owner's focus; nothing there, or no file, means no focus. It governs Phase 2.
- `python3 tools/loop/backlog.py check`.
- Record `uptime`, `df -h /`, `free -g`. On a machine with fewer than 20 GB free or under 4 GB available
  RAM, run tests with `-- --test-threads=4` and **do not start a build you cannot finish** — see
  "Machines" below.

**Do not read the whole loop log, the whole backlog archive, or the iteration reports.** They are for
humans and for grepping a specific fact. A pass that spends 20 minutes reading has failed.

## Machines — where a pass may run

This loop runs on **big** (since 2026-11-01): a Ryzen 9 5900X with 24 threads and 31 GB of RAM, shared with HomeStill and gooey's loop. Its unit caps the loop at 11 GB (`MemoryMax`) and sets `CARGO_BUILD_JOBS=10` and `RUST_TEST_THREADS=8`. **Never ssh to another machine to build or test**: this is the biggest one, and the round trips cost 8% of pass time on big_mac.

| | big |
|---|---|
| `cargo build/test/clippy` | yes, here |
| `accept.py` | yes, `--source <repo>/assets` → **10/10** (no `import` step: the golem fixture is not here). Compare with the previous big run. |
| `perf.py` | **no**: it times a live editor on a real display |
| Stills and clips | `tools/demo/record.py` under the unit's Xvfb (`DISPLAY=:99`); when a capture comes out black or empty, write `--no-clip "visible, not yet recorded: no display on big"` |
| Sanctuary's own `cargo test` | **no**: Sanctuary is not on big |

**Big's checkout must be a real clone, not an rsync target.** If `git status --short` shows double-digit modified files at a stale commit, the tree is somebody's scratch: stop, log it, and do not commit there.

## Phase 1: Bugs (bounded)

Candidates, in order: open bugs in `backlog.json`; red/flaky checks and clippy; parity breaks (a UI path
that changes state outside the registry, an operation with no MCP tool or no UI entry); a hands-on MCP
pass.

**Reproduce before fixing**, and decide must-fix with the three questions — **Blocks**, **Common**, **No
workaround**. Must-fix if Blocks, or any two. Borderline? Ask a fresh-context subagent given only the
evidence; if the two disagree, treat it as must-fix and say so.

**Write the regression check first and watch it fail.** For ECS logic a minimal `App` with the plugin,
`update()`, and an assertion is usually enough. Never `git stash` or `git checkout --` over uncommitted
work to make a "before" — a worktree at the base commit is the only honest baseline.

**Triage a red test before blaming it.** Rerun the binary alone; if it passes alone, run it at the base
commit in its own `CARGO_TARGET_DIR`. A failure the base shares is pre-existing: log it, do not attribute
it to this pass.

## Filed issues come before new features

**A filed issue is a consumer telling you what broke, which outranks anything this pass invented.**
Before you pick an item, list the repository's open issues and triage them:

```sh
gh issue list --repo bevy-coven/bevy_grimoire --state open --limit 100 \
  --json number,title,labels,createdAt --jq '.[] | "#\(.number) [\(.labels | map(.name) | join(","))] \(.title)"'
```

`gh` is installed and logged in on the laptop, big and big_mac (big_mac since 2026-10-28). If it is absent or its login fails, run this instead. It needs no login and prints the same list, but nothing can be filed or commented without `gh`, so say so in the log line:

```sh
curl -s "https://api.github.com/repos/Ladvien/bevy_grimoire/issues?state=open&per_page=100" \
  | python3 -c 'import json,sys; [print("#%s [%s] %s" % (i["number"], ",".join(l["name"] for l in i["labels"]), i["title"])) for i in json.load(sys.stdin) if "pull_request" not in i]'
```

A missing `gh` is never a reason to skip this step. If the command errors or prints a Python traceback, that is the rate-limit case below.

Then, in this order:

1. **A bug issue is the pass's item, ahead of any enhancement.** Anything reporting wrong behaviour —
   a crash, a panic, a visual artifact, a value that is implausible, a documented promise that is not
   kept — is a bug. Take the oldest such issue that is still reproducible, and say why it beats the
   others.
2. **An issue labelled `consumer` is the pass's item next, ahead of the owner's focus, the series and
   your own candidates.** A game that builds on these crates filed it and is waiting on it: its own loop
   comes back to the work by itself when the issue is closed as completed, so a stalled consumer costs two
   loops. Take the oldest open one you can work this pass, and say which. The `consumer:<game>` label
   says who asked and changes nothing about how you build it:
   - **Build what any game could use.** The game is the example, never a name in the API, a path or a
     default; these crates serve other games too. Where the issue proposes something game-shaped, build
     the general form and say so on the issue.
   - **Prove it here**, with a test in this repo that fails before and passes after against the issue's
     acceptance. Never edit, build or run the game to prove it, and never depend on it being on the machine.
   - **One that needs a decision the issue does not settle is decided, not skipped:** recommend, record and
     build it ("Owner input: recommend, record, proceed"). Only a change that would weaken a protected check
     cannot be worked: say exactly what is needed on the issue (`gh issue comment`; with no working `gh`, in
     the log line naming the issue), write one log line, and go on to the next one or to the focus. A
     blocked consumer issue never stops the pass.
3. **An issue labelled `owner` is the pass's item next**, after bugs and `consumer` issues and ahead of
   the focus's other items, the series and your own candidates. The owner wrote it. Take them in the order
   `docs/loop/focus.md` lists them; an `owner` issue the focus does not list comes after the listed ones,
   oldest first. Its **Depends on** line is binding: an issue whose dependency is still open is not ready,
   so take the next one that is. One is sometimes larger than its share of a pass: **ship one verified slice of it** (see the batch rule)
   (with its test and, if visible, its changelog clip), record which acceptance items are now met (on the
   issue with `gh issue comment`, or in the log line naming the issue when `gh` is not working), and close it with
   `Fixes #N` only in the commit that meets the last of them. Its **Decisions already made** are the design:
   follow them rather than stopping to ask. One that is blocked is handled like a blocked `consumer` issue.
4. **Every other open enhancement issue is the pass's item next, ahead of the focus's backlog items, the
   series and your own candidates** (the owner's rule, 2026-10-29): a filed request was asked for; a
   candidate this pass invents was not. The order of work is: bugs (Phase 1, then bug issues), then
   `consumer` issues, then `owner` issues in the focus's order, then every other open enhancement issue,
   oldest first. Only when each of them is done, blocked or a release issue does Phase 2 choose from the
   backlog, the series or your own candidates. A blocked issue gets its `gh issue comment` and is skipped;
   it never stops the pass. Name the issues you skipped, and why, in the log line, so the next pass sees
   the list was worked through.
5. **Close the loop.** An issue your pass fixes is referenced in the commit body (`Fixes #N`) **and**
   in the report, so the next pass can see it was addressed. Do not close an issue you did not fix,
   and do not close one whose fix this pass did not verify. For a `consumer` issue the close is the
   signal the game's loop waits for: close it only through a verified fix, never as not planned
   without the owner's word.
6. **A release issue is `bevy_emerge_loop`'s.** An issue asking to publish or bump a version is done by its next release (Phase 5, "Release"), which closes it with the crates.io URLs. Other workers skip it.

**When the API fallback answers `403` or `429`** (rate limit — 60 requests/hour per IP unauthenticated,
shared by everything on this network), do not treat it as "no issues": write one line saying the list
could not be read and continue. Never spend a second call to retry it in the same pass, and never infer
from silence that nothing is filed.

**A rate limit you have not hit is not a rate limit.** Make the call in every pass. It is one request; the driver sleeps at least 60 seconds between passes and backs off when passes find nothing, so this loop makes a few requests an hour against a limit of 60. Several idle passes on 2026-10-27 logged "issues not re-read (rate-limit risk)" without the call ever having answered `403` or `429`, which leaves a filed bug unseen. Only an answer you actually received lets you write "could not be read".

**An empty issue list costs nothing:** if it is empty or `gh` is unreachable, write one line saying so
and continue — never spend a pass on the lookup.

## Parallel workers: claim an issue before you build it

Several passes run at once (owner, 2026-10-29), each in its **own clone on its own machine**, all pushing to `main`. **Your worker name is your clone's directory name**: `basename "$(git rev-parse --show-toplevel)"` (`bevy_emerge_loop`, `bevy_emerge_w2`, `bevy_emerge_w3`, …). Names are unique across machines. The ranking above decides *what comes first*; these rules decide *who builds it*.

1. **Your own claim first.** `gh issue list --state open --label worker:<name>`: an issue carrying your label is your item this pass, ahead of the ranking. Finishing beats starting. **A claim you cannot advance on this machine is not your item.** If only a display, a clip, `clef-flash` or another worker's issue stands between it and done, release it now under item 4, with a comment naming the step it owes. Then take the next unclaimed issue in this same pass. Measured 2026-10-29: w3 held #99, whose remaining steps were a viewport draw and a clip, and logged five idle passes of about 25 s each beside unclaimed workable issues.
2. **Claim, then check.** Otherwise take the first issue the ranking gives you that carries no `worker:*` label, and claim it **before writing code**: `gh label create worker:<name> --color BFD4F2 2>/dev/null; gh issue edit <N> --add-label worker:<name>`, and a comment `claimed by <name> (pass <n>, <host>)`. Then re-read its labels: if another `worker:*` label is also on it, the name that sorts first keeps it; the other removes its own label and picks again.
3. **Another worker's claim is not yours,** however high it ranks: skip it and name it in the log line. The one exception is a stale claim, with no comment, commit or push naming the issue for 8 hours: take it over with a comment saying whose claim you replaced, and remove that label.
4. **Release when you stop.** Remove your label when the issue closes (`Fixes #N`), and when you leave it blocked (with the comment saying what it needs).
5. **Work that is not an issue is claimed by making it one** (owner, 2026-10-29: "They shouldn't stay idle."). A worker with no issue it can claim takes non-issue work: Phase 1's backlog bugs, the series' next step, the focus's backlog items (items 4–6), or a feature it researches itself (Phase 2). Claim it before writing code, so two workers never build one item: search for an open issue naming it (`gh issue list --state open --search "<backlog id or title>"`); with none, `gh issue create --label enhancement --label worker:<name> --title "<backlog id>: <title>"` with the backlog entry, or the research summary and its sources, as the body. Then it is a claimed issue like any other (items 1–4). Releases stay `bevy_emerge_loop`'s alone (Phase 5).
6. **A red check on `main` is everyone's bug, and one worker's item.** The first to see it opens a `bug` issue naming the failing test and the commit that turned it red (unless one is open) and claims it. The others treat that failure as pre-existing (Phase 1's triage) and go on.
7. **GitHub Actions is not your gate.** CI runs once a day (`.github/workflows/ci.yml`). Never wait for, poll or re-run a workflow: the local gate (Phase 4) is the gate, and a green local gate is what lets you push. At the start of a pass, read the last daily run once: `gh run list --workflow ci.yml --limit 1 --json conclusion,headSha,url`. A `failure` is a bug: open a `bug` issue naming that run unless one is open, and the ranking takes it first. The push runs the local gate too: each worker clone sets `core.hooksPath=tools/loop/hooks`, whose `pre-push` runs clippy and `cargo test --workspace` on the commit you push to `main` (only `backlog.py check` and `changelog.py check` when the push changes nothing but docs and media) and refuses the push when one fails. Give `git push` a timeout of at least 1800 s. A refused push is a red gate: fix it, or log the item as blocked and ship only what passes. Never push with `--no-verify` and never change `core.hooksPath`. Measured 2026-10-29: w3's passes 4–8 each reported "I did not run `cargo test --workspace`" and pushed `bevy_oracle` tests that failed 165 of 200 runs, so `main` was red.
8. **Merge, never force.** On a refused push, `git pull --no-rebase origin main` and push again at once. A clean merge (no conflict, no hand edit) into a commit that already passed the push gate in this clone runs only clippy over every target, in minutes, so the retry is cheap; a merge where you resolved a conflict runs the full gate again, and a conflict with another worker's change keeps both changes. Never amend or edit the merge commit itself: that is a hand edit, and it costs the full gate. **A push that ends "Connection to github.com closed by remote host" after the gate printed `passed` did not land: push the same commit again at once.** The gate remembers it and skips (`already passed the gate in this clone`). Measured 2026-10-29: bevy_emerge_loop's pass 127 gated #115's commits for 30 minutes twice and lost both pushes this way.
9. **Every visible feature ships with its clip and its docs** (Phase 5): a changelog line with a demo clip, and its section in the README or `docs/guide/`. A worker on big records under its unit's Xvfb display first. If no clip there passes the look, it writes `--no-clip "visible, not yet recorded: no display on big"`, and **`bevy_emerge_w2` (a Mac) records the oldest such line before claiming new work** in its next pass. A feature without a clip after a day is a bug in the docs.

## What this editor needs from a family crate: the two-way street

The owner's crates (`bevy_elements`, `bevy_flora`, `bevy_lights`, `bevy_society`, `bevy_speech_bubbles`, `bevy_stigmergy`, `bevy_light_grid`, `bevy_orca`, `bevy_carnage`, `bevy_fairy`, `isomesh`; issues #96–#108) are integrated by **configuring them, never by forking, wrapping or patching them here**. When an item needs a change in one of them (a missing API, a setting that cannot change at runtime, a bug), the change is made **in that crate's repository** and this editor then takes it by moving its pin.

1. **Check for one first.** `gh issue list --repo Ladvien/<crate> --state open --label consumer:bevy_grimoire` and read the titles. Add to an open one rather than filing a twin.
2. **File it.** `gh issue create --repo Ladvien/<crate> --label consumer --label consumer:bevy_grimoire`, with a title saying what the crate should do, and a body with **Problem** (what the editor cannot do, with the call or setting), **Want** (the crate-side behaviour, general enough for any game or tool), **Acceptance** (a test in that repository that fails before and passes after), and **Needed by: bevy-coven/bevy_grimoire#N**. Comment on the editor issue with the link.
3. **Mark the editor item waiting.** Its backlog evidence (or a comment on its issue) gets `Needs: Ladvien/<crate>#M`. The item is not ready while that issue is open; choose another item. A waiting item never stops the pass.
4. **Who builds it** depends on whether that crate's own loop is running. `docs/loop/focus.md` lists, under "Family loops running", the crates whose loops the owner has running.
   - **Listed:** leave it to that loop, and re-check the issue each pass.
   - **Not listed:** this pass may build it **at the source**, as its item for the pass:
     - clone the crate's repository outside this clone (`~/family_src/<worker>/<crate>`, fetched fresh; `<worker>` is yours, from "Parallel workers", so two workers on one machine never share a crate's tree);
     - follow that repository's `AGENTS.md`, and never edit its existing tests or goldens;
     - write the failing test first, make the fix, and run that crate's own test suite;
     - commit there with `Fixes #M` and push to its `main`, merging `origin/main` if refused, never forcing;
     - say on the issue what changed and which test proves it.
   - **Leave that crate's version and release to its own loop**; the pin below is a git revision, so nothing here waits on a publish. A crate-side change is one item: the pin bump below happens in the next pass, or in the same pass if time remains.
5. **Take it.** When the issue is `CLOSED` as completed, find a commit on the crate's `main` that contains the fix (`git ls-remote https://github.com/Ladvien/<crate> refs/heads/main`). Move this editor's pin to it (`rev = "<sha>"` in the workspace `Cargo.toml`), run the full gate, and only then continue the editor item. A pin bump that breaks the gate is a bug in this pass, not in the crate: say so on the issue and leave the pin where it was.
6. **Pins are git revisions, never paths.** A path dependency builds against another loop's clone mid-edit; a crates.io version waits on that crate's own release.

## Phase 2: Choose the enhancement

**The owner's focus outranks the series and your own candidates.** If `docs/loop/focus.md` has a focus, choose the enhancement that best advances it; when it names a series step, a backlog item or an issue, that is the item. Bugs, filed bug issues, `consumer` issues, `owner` issues and then every other open enhancement issue still come first (Phase 1 and the issues section above): **you are choosing here only when none of them can be worked this pass.** An item waiting on a family crate is not ready (see "the two-way street"); building that crate-side change at the source is an item of its own. If the focus cannot be advanced this pass because it is done, blocked, or needs a decision, write one log line saying which and why, then choose as below. With no focus, choose as below.

If an open `series` has a next step you can work this pass, take it and skip the list. A series whose next step is blocked or waiting on the owner does **not** skip the list: log it in one line and choose from the backlog as below. The backlog's open items are real work; a pass that idles beside them has wasted itself.

Propose 2–3 candidates drawn from the backlog, from friction in Phase 1, or from the lenses below. One
line each: the user problem, how often it is hit, the expected saving, the risk. Rank, pick one, record
the others in the backlog.

**Everything blocked is not a reason to stop: record a clip, then build.** When the focus, the series and your candidates are all blocked or waiting on the owner, take one `not yet recorded` line in `CHANGELOG.md`, newest first: record its clip on this build (Phase 5's recipe; scratch copy of the project, never the consumer's own files), look at the still and a contact sheet, confirm it shows what the line says, and replace the reason with the clip. A clip that does not show what the line says is not shipped; log which line and why, and leave it. **A pass never ends idle** (owner, 2026-10-29): with no such line left, or none this machine can record, research a feature of your own through the lenses below, home-still first (`distill_search`; for a paper the corpus lacks, `paper_search`, `paper_download`, `scribe_convert`, then `distill_index`), then shipped editors (Mixar, Unity, Unreal, Godot, LDtk, Tiled). Claim it as an issue ("Parallel workers", item 5) and build its first slice with a failing-first test. "Nothing workable" is never a log line; a pass that ships nothing names the concrete blocker.

**These are not blockers** (owner, 2026-10-30, after w3, w4 and w5 idled passes in a row on each of them):

- **"More than one pass", "a new family crate", "a first heavy build".** Slice it. Slice 1 adds the crate as an optional feature, pinned by `rev`, and starts the build. The build is the pass's work, not a reason to skip the pass. #98, #100, #103, #106, #107 and #108 each started this way.
- **"Recording clips is `bevy_emerge_w2`'s job".** w2 records first, but any worker on a Mac (laptop, big_mac, bmb) records a `not yet recorded` line when it has no issue. On 2026-10-30 at 01:05, 47 lines were waiting.
- **"Labelled `decide`" or "needs `clef-flash`"**, when the issue has steps that need no model: build those steps.
- **"Needs a real game" (#95):** the consumer game is Sanctuary, at `/Users/ladvien/sanctuary` on the laptop. Work on a scratch copy of it, never on the checkout itself.

A log line that repeats an earlier pass's reasons for shipping nothing is itself a failed pass.

**Lenses**, and the research says which are already answered:

- **Describe → set up → inspect → present.** Where does manual work or unverified output get in the way?
- **Constraints, not coordinates** *(research #2 — not built)*. An agent placing pieces by number
  produces collisions and out-of-bounds layout; a relational constraint graph that a solver satisfies
  does not, and it is readable and editable before it runs. **This repo has the solver and no constraint
  layer.** A `layout_constrain` taking relations (`in front of`, `reachable from`, `clear in front of`,
  `faces`, `along path`, `visible from`), stored as data beside the map, reporting which soft constraints
  it could not satisfy, is the largest unbuilt item in the research and the best single candidate.
- **A small, searchable tool surface** *(research #3 — partially built)*. 14 tools today, and
  tool-selection accuracy falls off past ~10–30. Every new capability must go through generic apply, not a
  new tool; if the count grows, the tool to build is a capability *search*, not a fifteenth tool.
- **Kit admission rules from shipped practice** *(research #4 — not built)*. Project-wide standards
  (door frame, minimum passage ≈ 2× capsule, max slope, floor thickness) as data every kit conforms to;
  automated kit stress tests (loop a hallway back, stack sub-kits, build a pillared room from hall
  pieces); no patch-up pieces; integer-multiple sub-kit footprints.
- **Role-weighted repetition** *(research #5)*. Repeated clutter bothers players more than repeated
  structure; a repetition metric should weigh by role.
- **Graybox first** *(research #6, narrowed)*. Staged whole-world generation is **dropped** — the
  developer builds in turns. What survives: graybox pieces, and a generator that only runs inside the
  request's anchor region.
- **Per-turn measurement** *(research #7–#9, per the addendum)*. Seeds, judges and personas measure a
  **turn**, not a finished world. Every session re-checks earlier turns' assertions after a new turn
  (preservation), and each turn is marked `expect: Act` or `expect: Ask` — asking too often fails as
  surely as asking too rarely.
- **Options on request** *(research #10, narrowed)*. "Show me three courthouses" — not automatic variety.
- **Agent self-inspection, data hot reload, responsiveness, expressiveness.** As before.

**Not candidates:** refactors with no user-visible effect, new subsystems, dependency upgrades, or
save-format changes without a migration.

## Phase 3: Build

Follow existing patterns; reuse the theme and widgets. A new user action is a registered operation, with
preview and undo declared, and both front ends dispatch it. Keep the diff to the plan; if it must grow,
stop and re-scope rather than run long.

## Phase 4: Prove it

1. **Deterministic gate, run once — by the push.** While building, run only what the change touches: that crate's tests (`cargo test -p bevy_cauldron --test editor <module>::` for the editor, `cargo test -p <crate>` otherwise) and `cargo clippy -p <crate> --all-targets -- -D warnings`. Never run `cargo test --workspace` by hand: the push is the full gate (`tools/loop/hooks/pre-push` runs clippy over every target and `cargo test --workspace`, and remembers a commit that passed), so one green push is the pass's one full run. Measured 2026-11-01: passes ran the full gate about 2.75 times each, 23% of pass time. Run `backlog.py check` and `changelog.py check` before the push (seconds). The plan's check failed before and passes after; quote the failing assertion.
2. **Visual proof**, where a surface changed: stills and a sequence, reviewed against a reference, captured under the unit's Xvfb (`DISPLAY=:99`). If a capture comes out black or empty, fall back to a layout assertion on `Text`/`ComputedNode` entities, which is stronger than a capture for "is it drawn".
3. **`accept.py` before and after**, same machine, same source. A pass that lowers `reached` is a bug.
4. **Independent review** — one `reviewer` subagent over the diff. **If it cannot be spawned (a rate limit,
   no model), say so in one line and continue**: a Wiggum loop that stops for an unavailable reviewer
   stops forever. Five consecutive passes have already lost it to a cloud 429.
5. **Self-review fallback** when the reviewer could not run: answer the five questions you would have given
   it, in the report, and say they are yours.

## Phase 5: Ship, report, stop

- Commit as `feat(<area>): <user-visible change>` or `fix(<area>): …`, with a body carrying the problem,
  the change and where the evidence is.
- **Push.** A pass that does not push has hidden its work from every later pass. `git push` to the loop
  branch; merge to `main` only if the gate is green and `docs/PROJECT.md`'s conventions allow it.
- Write `docs/loop/iterations/<date>-<area>-<short-sha>.md` from the template below, **short**: a pass
  that runs long and writes a 200-line report has spent the next pass's budget on prose.
- Append one line to `docs/loop/log.md`. Append only.
- **One changelog line, and a clip if the change can be seen** (AGENTS.md, "The changelog: a line for every
  change, a clip for every visible one"). A visible change: a storyboard of the result run with
  `tools/demo/record.py` on the fixture project, `render.py <events.json> --out
  docs/changelog/media/<date>-<id>.mp4` (20 s at most; the still is written beside it), watch the still and
  the clip, then `changelog.py add --id <id> --sha <sha> --title "<what it added>" --clip <date>-<id>`. A change
  nobody can see: `--no-clip "<what kind of change>; proved by <test>"`. On a machine with no display, a visible
  change gets `--no-clip "visible, not yet recorded: no display on <host>"` and a pass on a display records it.
  A pass that has time left may record one older `not yet recorded` line and replace its reason with the clip.
- Update `backlog.json`; `python3 tools/loop/backlog.py archive`, then `check`.
- **Release** (owner, 2026-10-29: loops publish, and never wait on CI to do it). **Only `bevy_emerge_loop` releases**; other workers never bump a version, tag or publish. At the end of its pass, after its own push and with the gate green on that pushed commit, it releases every publishable crate whose files changed since its last release: `git diff --quiet <crate>-v<version> HEAD -- crates/<crate> Cargo.toml` fails, or the crate has never been published.
  1. Version: `cargo semver-checks check-release -p <crate>` for a crate already on crates.io (`cargo install cargo-semver-checks --locked` if missing). Pre-1.0: a breaking change bumps MINOR and resets PATCH, anything else bumps PATCH; never 1.0.0. Update dependents' version requirements and `Cargo.lock`. Commit `release: <crate> v<version>[, …]`.
  2. `cargo publish -p <crate> --dry-run`, then `cargo publish -p <crate>`, in dependency order, locally, with the credentials `cargo login` stored; never print them.
  3. `git tag -a <crate>-v<version> -m "<crate> v<version>"` per crate, then push the commit and tags (merge, never force).
  4. `cargo info <crate>@<version>` resolves. Put the crates.io URLs in the report's Release section.

  **Do not add version headings to `CHANGELOG.md`**: `tools/changelog` owns its format. A first publish of this workspace's own crate names is allowed. **E-006 is the owner's yes** (2026-10-29): it is `bevy_emerge_loop`'s first non-issue item. Decide what it leaves open (which crates ship, versions on path dependencies, the git-pinned family dependencies) under "Owner input: recommend, record, proceed". A failed publish is logged with its error and retried next pass.

### README, demos and docs stay true

A README is read before anything else, so a false line there costs more than a false line anywhere else.
An audit on 2026-10-27 found one in each of the four loop repos: a `tour.gif` captioned "at noon" over a HUD
reading 10.00 h with a running clock; a `ties.gif` captioned for rivalry colours that appear in none of its 240
frames; a hero caption promising a flame catch that its file never shows; and a demo video captioned as the full
recording of the GIF above it when it shows different objects. Before you push a change a user can see:

1. **Put it in the section it belongs to.** Do not append to the introduction or to the end, and do not
   restructure the README in a feature pass. It reads in this order — what it is in one line, the demo, what
   you get, install, quick start, then the detail sections — and an item added in the wrong place undoes it.
2. **A caption describes the file as it exists.** Open the GIF or still (frame count, size, any HUD burned into
   it) and write only what you can read off it; a figure quoted from program output comes from running that
   program now, not from the log. Recipes: `tools/demo/` (`record.py`, `render.py`, `storyboards/`). A clip that passes `check.py` is necessary, not sufficient: look at it.
3. **Media you cannot regenerate here is logged, not left lying.** With no GPU or display on this machine, name
   the stale file and its path in the report and change its caption to what it still shows.
4. **Generated docs are regenerated, never edited.** `docs/mcp-tools.md` is rendered from the server's own tool
   declarations (`cargo run -q -p bevy_cauldron_mcp -- --emit-docs > docs/mcp-tools.md`) and
   `cargo test -p bevy_cauldron_mcp --test docs_are_current` must pass. A command the README tells a reader to
   run is run, not recalled.

## Guardrails

- Never modify or delete real user content. Test on scratch copies.
- Close an editor you launched by its pid; never `pkill` by name.
- No destructive git: no `reset --hard`, no force push, no history rewriting, no deleting branches or tags.
- Never yank, republish, or edit crates.io settings.
- Do not add dependencies without a logged reason.
- Never loop live VLM calls; use recorded or fixture responses.

## Report template (keep it short)

```markdown
# YYYY-MM-DD <area>: <one-line outcome>

Focus: <area> · Commits: <sha>..<sha> · Machine: <laptop|big> · Outcome: shipped | fixes only | nothing shipped

## Summary
Two or three sentences.

## Baseline
Gate: <status>. `accept.py`: <k>/<of> on <source>. Load <n>, free disk <n> GB, available RAM <n> GB.

## Bugs fixed
### B-###: <title>
Must-fix: blocks <y/n>, common <y/n>, no workaround <y/n> — <evidence>
Root cause: … · Regression check: `path::test_name` (failed before, passes after)

## Enhancement: <title>
Goal: one sentence · Files: … · Proof: <check that failed before>
MCP: operations/tools added or changed, undo strategy, preview behaviour

## Borrowed from Mixar
The six-line behaviour note, or "none".

## Performance
`perf.py` before → after, or "not measured" and why.

## Acceptance
`reached <k>/<m> → <k>/<m>`; first failure `<id>`.

## Review
Reviewer ran / could not run and why; the five questions and what each found.

## Test diffs
Every change to test code and its justification, or "none".

## Release
The crates and versions published, with their crates.io URLs, or one line saying why there was none (not `bevy_emerge_loop`; nothing packaged changed; E-006 not finished).

## Next focus
One line.
```

## Clean up when you are done

Worktrees and build directories outside the clone are not free: a `target/` for this crate is tens of GB, big's
`/tmp` is RAM, and on 2026-10-27 earlier passes had left 37 GB of baseline builds and 5 GB of scratch behind.
**After you ship and before you write the pass's last log line, whether or not you shipped,** remove what *you*
made this pass, and list what earlier passes left.

1. **A baseline is a pair that can be found.** Make it with `git worktree add --detach <path> <sha>` (never a
   branch name: a worktree with a branch checked out stops anything else checking that branch out) and build it
   into `<path>-target`. Then `git worktree list`, plus the `*-target` directory beside each worktree, is the
   whole inventory.
2. **Remove yours.** `git worktree remove <path>` (plain, never `--force`) and `rm -rf <path>-target`, then your
   scratch under `/tmp` (captures, frames, venvs, gate logs, `rustdoctest*`). Remove only what you created this
   pass, and name each path in the log. If git refuses because the worktree has a change in it, name the file
   in the log and leave it.
3. **Leftovers from earlier passes are listed, never deleted.** A killed pass cannot clean up after itself, but
   an agent does not delete what it did not create, so list them once and let a person approve the removal:

   ```sh
   git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print}' | tail -n +2   # all but the clone
   du -sh <wt> <wt>-target                       # the size of each one that is not yours
   git -C <wt> status --short                    # empty: nothing is lost by removing it
   find <wt> <wt>-target -mmin -120 -print -quit # empty: untouched for two hours, so no live build is using it
   ```

   Log each with its path, size, and whether it is clean, already on `origin/main` and idle (the likeliest to be
   stale). Say so, and still leave it.
4. **Never remove** the clone itself, its own `target/` (the build cache the next pass starts from), anything you
   did not create this pass, or anything a process is using.
5. Write one log line: what you removed and the space it freed (`df -h .` before and after), and the leftovers
   you listed.

## Before you stop

- [ ] The tree is clean and **pushed**.
- [ ] Every deterministic check passes, or the pass is logged as blocked with the failure quoted.
- [ ] No protected file was edited, except what an owner exception in "The protected evaluator" lists, or an addition recorded as an assumed owner exception; no existing check was removed, loosened, skipped or re-pinned.
- [ ] `accept.py` ran before and after on the same machine and source.
- [ ] One enhancement at most, with a check that failed before and passes after.
- [ ] The report, the LOG line and the backlog are committed.
- [ ] Only bevy_emerge_loop released, locally, from a green gate; nothing was yanked.
- [ ] You stopped inside the budget.
- [ ] Worktrees and build directories you made are removed, and the leftovers earlier passes left are listed in the log, not deleted (see "Clean up when you are done").

README and docs follow https://github.com/bevy-coven/.github/blob/main/DOCUMENTATION.md; a feature is not done until its README line and, if visible, its GIF exist.
