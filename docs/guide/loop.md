# The development loop

bevy_emerge is built one iteration at a time. An iteration fixes a bounded set of bugs, makes at most one enhancement, proves it, and writes a report. This page describes what an iteration leaves behind, so you can find it.

The brief an iteration follows is `.omp/commands/emerge-feature-loop.md`.

How the documentation is written, with the research behind it, is in [`docs/loop/doc-guidance.md`](../loop/doc-guidance.md).

## One iteration

Each iteration runs these phases in order:

0. **Orient.** Read the state files and the last report, and check the repository is in the state the last iteration left it.
1. **Bugs.** A bounded sweep. A must-fix bug is fixed in the iteration. Other bugs are fixed only when the fix is trivial and in code the iteration already touches.
2. **Choose the enhancement.** At most one. It must be provable by a check that fails before and passes after, or the iteration ships nothing.
3. **Build.**
4. **Prove it.** The check that failed must pass; no existing check may be weakened.
5. **Ship, report, stop.** Commit, open a pull request, merge, and write the report.
6. **Release.** Only when the iteration calls for one. Releases are not part of a normal iteration.

## State files

| File | What it holds |
|---|---|
| `docs/loop/backlog.json` | The working list of bugs and enhancements. Edited one item at a time. |
| `docs/loop/backlog-archive.json` | Fixed, rejected and stale items. |
| `docs/loop/log.md` | One line per iteration, newest at the bottom. |
| `docs/loop/iterations/` | One report per iteration, named `<date>-<area>-<sha>.md`. |
| `docs/loop/iterations/assets/` | Curated stills and frames a report cites. |
| `docs/loop/needs-decision/` | Questions the loop could not settle alone. |

The brief names the index `docs/loop/LOG.md`. The file in the repository is `docs/loop/log.md`; the brief will be corrected when it is next changed.

Raw captures and check logs go under `target/loop/<date>-<sha>/`. They are not committed; the report links only the curated assets.

## Reading a backlog item

Each item has a `kind` (`bug` or `enhancement`), a `status`, and a `must_fix` record: `blocks`, `common`, and `no_workaround`. A bug is must-fix when `blocks` is yes, or when two of the three flags are yes.

## Protected checks

Golden stills and golden state dumps under `tests/protected/` are read-only to the loop. If one has to change, the loop stops and flags it for a person.
