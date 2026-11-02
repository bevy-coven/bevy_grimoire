# 2026-11-02 demos: #235's badge reveal recorded, and the hot-reload series closed

Focus: the last `hot-reload` issue, then the owed clips · Commits: 350ad94..HEAD · Machine: big · Outcome: shipped

## Summary
#201's last question was answered by reading the code rather than by building: an `alias` field would have
**no writer**, because no placement-rename operation exists. So it is closed with the gap recorded on #129,
and the series is done. #235's reveal got its clip, which needed a real fix in `check.py` — a held key was
not a change moment it knew.

## Baseline
Gate green at 350ad94. `accept.py` not run: no engine behaviour changed. Load 2.9, free disk 156 GB, RAM 13 GB.

## #201 closed — with the reason, not without it
Its five acceptance items were met or explicitly resolved (pass 230). The residual case — rename **and**
move in one save — cannot be fixed by `alias`, because:
- `RenameMap`/`map_rename` renames the **map**; nothing in `crates/bevy_cauldron/src` assigns a `Placed::id`;
- so a placement's id changes only when a person edits the file by hand, which is exactly the case where
  the rows carry no evidence to pair on.
Recorded on **#129** as a slice it should hold (`placement_rename { from, to }`).

## The clip, and the checker gap it found
`docs/changelog/media/2026-11-02-badges-reveal.mp4`: the Map tab quiet, `K` held, the chord chips on the
door strip, INSPECTOR, the library, PLACE and a legend column, then released.

**`check.py` knew only `Ctrl/Cmd+Z` and `Home` among pressed keys**, so a storyboard of a held reveal
reported `change_moments` as **empty** and failed with no change line. `tools/demo/` is not protected
(established pass 222), so the fix is in the tool: `KeyK` is now a `reveal` moment, with two tests.
Measured after the fix: **`change reveal at 0.73 s: 9.56 %`**, scene 46.7 %, longest static 1.20 s, 0.43 MB
— **PASS**.

## Issues
- **#235 closed**, both acceptance items met under the owner's chip-only decision; it had **no changelog
  line at all**, now `235-badges-reveal`.
- **#201 closed**; **#129** gained the rename-operation slice; **UI-00** ticked for UI-41 and annotated that
  #210 waits on G-33.
- Everything else still blocked: G-33 is open, `#193` waits on bevy_elements#55, `#112`/`#113` on `:8093`,
  `#152` merged by this pass's #152 fix, `#195` is decided, `#197` `wontfix`.

## Test diffs
Two new cases in `tools/demo/test_check.py` (`HeldReveal`) plus the storyboard. No assertion in
`crates/*/tests/**` touched.

## Review
`reviewer` not spawned. Self-review: (1) *is #235 really met* — the reveal is gooey's and the clip passes,
and the part not done was the owner's own choice; (2) *did I weaken check.py* — the new rule adds a moment,
removes none, and the fail case is still a fail (a reveal with no change still reads FAIL); (3) *is the
alias reasoning right* — verified by reading for any `Placed::id` assignment, found none; (4) *could the
close mislead* — both issues say what is not done and where it went; (5) *what did I get wrong* — I first
wrote the storyboard with `key`+`hold` instead of `hold`+`secs`, and took three takes to clear the static
run because a held key is a still picture.

## Release
None: `bevy_grimoire` unchanged; `bevy_cauldron` is `publish = false`.

## Next focus
The UI series is blocked on G-33; the owed clips are the work.
