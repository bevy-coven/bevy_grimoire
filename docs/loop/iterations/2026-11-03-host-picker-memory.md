# 2026-11-03 loop: the host picker's memory rule is a test, and its ranking key is one function

Focus: the loop's own tooling (#195) · Commits: b303528..a70c4e8 · Machine: big · Outcome: shipped

## Summary

`otel_host_pick.py` already did what #195 asked — it refuses a host that cannot prove it has RAM — and
**had no test at all**, so any pass could have deleted the rule and every check would still pass. Six
tests now pin it, two verified load-bearing by breaking the picker, and the sort key is one function the
picker and the test share.

The pass also hit the same protected wall a sixth time on E-002 and stopped trying, rather than weakening
a pinned classification set.

## Baseline

Gate: green before (b303528). `accept.py`: not run (no solver path touched). Load 11.34, free disk 116 GB,
available RAM 13 GB.

## Bugs fixed

none.

## Enhancement: `otel_host_pick`'s memory rule, pinned (#195, option 4)

Goal: a host that cannot build is excluded by a rule a test holds, not by a habit.
Files: `tools/loop/test_otel_host_pick.py` (new, 6 tests), `tools/loop/otel_host_pick.py` (`ranking_key`
extracted; `main` calls it).
Proof: `python3 -m unittest tools.loop.test_otel_host_pick` — 6 passed. Two of them fail when the code is
broken, each checked:

| removed from the picker | the test that fails |
|---|---|
| the RAM term of `ranking_key` | `test_equally_idle_hosts_are_ordered_by_available_ram` |
| the no-series exclusion in `verdict` | `test_no_series_is_an_exclusion_not_a_zero` |

The picker's own docstring already stated the rule as check 3 of 5; the tests make it enforceable. Live
after the change: `--pick` answers `big`, so the tool still works.

**What this does not do:** solve the memory shortage. #195's other three options are a loop setting and
a protected test, which the loop may not change on its own; the comment on the issue asks for them and
records today's measurement (`load average: 11.34, 21.39, 18.08`; the previous pass spent 31 minutes of
cargo time on one feature).

MCP: none. Preview: none. Undo: none.

## Borrowed from Mixar

none.

## Performance

Not measured with `perf.py` (no live display here); this is loop tooling, not a frame path.

## Acceptance

`reached` unchanged — no solver path touched.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine:

1. **Do the tests test the real code?** Yes for the sort key since the extraction; the `verdict` tests
   always called the real function. Verified by mutating each rule and watching the matching test fail.
2. **Is a passing test that cannot fail possible here?** No: `test_exactly_the_floor_is_eligible` is the
   only one that asserts a pass, and it is paired with the one above it that asserts the same value
   excluded at `--min-mem-gb 24`.
3. **Could the extraction change behaviour?** No: the expression is character-identical inside
   `ranking_key`, and the live `--pick` still answers.
4. **Is `tools/loop/otel_host_pick.py` protected?** No — the protected list is `accept.py`, `perf.py`,
   `common.py`, `backlog.py`, `test_accept.py` and `hooks/**`. A new test beside it is a new file.
5. **Does this leave E-002's wall unrecorded?** No: it is on #129, third time, with the diff.

## Test diffs

- `tools/loop/test_otel_host_pick.py`: new. No existing test changed.

## Release

None: no publishable crate's files changed.

## Next focus

E-002 is blocked on the pinned classification sets (#129); #195's options 1 and 3 are the owner's; the `ui`
series waits on #210. The next pass's best unblocked item is a new capability rather than a migration.
