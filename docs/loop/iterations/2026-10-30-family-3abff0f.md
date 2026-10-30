# 2026-10-30 family: #103 slices 3–5 landed on main after hours unpushed

Focus: family (#103) · Commits: 3abff0f, 107398c · Machine: bmb (Mac, worker bevy_emerge_w7) · Outcome: fixes only

## Summary
Local main was 21 commits ahead of origin: #103 slices 3–5 (a5bf126 `light_map`, 1e73078 `light_pool`, 1768ce8 the LIGHT MAP overlay) had passed the gate repeatedly (`.git/gate.log`, last 0e65224 passed 2345 s) but never landed, because each merge with origin conflicted on CHANGELOG.md/log.md, so every retry ran the 30–40 min full gate and origin moved meanwhile. This pass merged origin/main into the last gated commit (0e65224) directly, not into the later ungated merge 1e90d30, which the hook's clean-merge path accepts: clippy only, 210 s, landed 7c5a791..3abff0f. The changelog and log lines that 0e65224's merge had deferred were restored in a docs-only push (107398c, no gate).

## Baseline
Load 9–19 on 14 cores, 24 GB free disk. `accept.py` not run: no product code changed this pass (the landed commits were gated by earlier passes).

## Bugs fixed
### Unpushed slices (no backlog id)
Must-fix: blocks y (#103's work hidden from every other worker), common y (hit on every retry since 02:49), no workaround n. Root cause: a hand-resolved docs conflict in every merge defeats `pre-push`'s clean-merge fast path. Proof: `git merge-base --is-ancestor 1768ce86 origin/main` succeeds after 3abff0f.

## Enhancement
None this pass: a new code commit needs a 30–40 min full gate on bmb, past the budget.

## Borrowed from Mixar
None.

## Performance
Not measured.

## Acceptance
Not run (no product change).

## Review
Not spawned (no new code). Self-review, mine: (1) Was anything dropped? 3abff0f's tree is `git merge-tree` of 0e65224 and origin; local main merged it with an identical tree. (2) Lost docs? Three changelog and three log lines restored from 5ef284d. (3) 103-pool's sha now names its feature commit 1e73078 instead of the merge e6da84e. (4) Protected files? Only appends to CHANGELOG.md and log.md. (5) Gate bypassed? No: clippy-only is the hook's own path for a clean merge of a gated commit.

## Test diffs
None.

## Release
None: not `bevy_emerge_loop`.

## Next focus
#103: Flicker in the viewport preview (bevy_lights' FlickerPlugin driving a PointLight per fixture), night via ClockScrub, then the clip. Keep log/changelog lines in a docs-only commit pushed after the code lands.
