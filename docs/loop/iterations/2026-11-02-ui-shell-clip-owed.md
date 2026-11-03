# 2026-11-02 ui: #224's test is green; its clip is owed, and the reason is measured

Focus: #224 (UI-30, the workbench shell) · Commits: none · Machine: big · Outcome: nothing shipped (acceptance item 1 verified, item 2 blocked on tooling, stated)

## Summary
`#224`'s first acceptance item — *a headless test finds each region by its accessible label, and resizing a
dock never covers the viewport* — is met and green. Its second item is a clip of the shell with docks
resized. Eight takes; the divider drags and the palette widens, but `check.py` rejects every one on a
timing rule rather than on content, and one passing take was overwritten before it could be re-rendered.

## Baseline
Gate: green (`05f73d3`). `accept.py`: not run. Load 6.9, 150 GB free, 13 GB available RAM.

## Acceptance item 1 — met
```
the_shell_is_a_gooey_dock::every_region_of_the_shell_is_a_registered_dock_panel ... ok
the_shell_is_a_gooey_dock::the_viewport_has_its_own_pane_in_the_dock_layout ... ok
test result: ok. 2 passed
```

## Acceptance item 2 — owed, and why
The divider **does** work: two contact sheets show the palette widening and the inspector growing. What
fails is the checker's arithmetic:

1. **A caption-only tail over 1.5 s is a static run.** `check.py` exempts a 1.5 s hold, so the last *motion*
   must be within 1.5 s of the end. My captions outlast that.
2. **A drag whose press lands in the viewport's x range (470–950) is scored as a "place" moment** and must
   change 5 % of the scene — even though a drag on a divider places nothing.

**Eight takes were tried.** One passed (scene 55.1 %, static 1.33 s) and was discarded by eye: its last
frames showed the map squashed by an over-long second drag, so its caption would have lied. Its
`events.json` was then overwritten by later takes, which is why it could not be re-rendered.

## The measurement a next pass needs
The drags were aimed at the panels' **edges** — INSPECTOR's right at x=422, PLACE's left at x=986, both read
from `UiGlobalTransform` on the headings. Those are not the handle: a drag beginning there left the divider
unmoved in the last take (a 3.67 s static run, i.e. only the cursor moved). The handle is `bevy_gooey`'s own
split component, and the next pass should read it the way `PaletteList` was made `pub` — one nameable
component ends the guessing.

## Test diffs
None.

## Review
Not applicable: no product change.

## Release
None.

## Next focus
Read the split handle's real hit area, then one drag per take ending on motion.
