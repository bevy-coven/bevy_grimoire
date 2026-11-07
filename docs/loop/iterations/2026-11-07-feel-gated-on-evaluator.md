# 2026-11-07 feel series: the rest of epic #330 is gated on one owner decision

Focus: epic #330 (owner's steer) · Commits: none in code · Machine: big · Outcome: nothing shipped (blocker named)

## Summary
No bevy_grimoire item in the epic is ready, and no ready bevy_gooey feel item can be built without changing protected tests. The ready items are G-58, G-63, G-64 and G-65, and the earlier blocked ones are G-44, G-46 and G-57. Each one animates something that bevy_gooey's protected tests assert right after the interaction, and 54 of its test files run on a real-time clock. This pass filed one consolidated owner decision, bevy-coven/bevy_gooey#76, with the evidence and a one-line exception that keeps every assertion. It linked that decision from every blocked item and from the epic, and built nothing.

## Baseline
No gate or `accept.py` run: no code changed.

## The blocker (bevy-coven/bevy_gooey#76)
- **Real-time clocks.** Every `tests/*.rs` except the motion, presence, toast, tree and feel-series files lacks `TimeUpdateStrategy::ManualDuration`. A frame lasts milliseconds, so any spring or delay is still near its start when an assertion reads.
- **The assertions that break:**
  - `split.rs:817-821/857-861/892-895`: collapse within two frames.
  - `section.rs:295-300/335/360`: close is instant; open is over 40 px within a few frames.
  - `tooltip.rs:215-217`: one move opens the tooltip.
  - `list.rs` `walking_down_...`: the row is whole right after Down.
  - `motion.rs:258-344`: pins `PopoverSlide` and `POPOVER_SLIDE_PX`.
  - Colour reads right after hover or press.
- **Recommended exception:** a real-time test app may insert `MotionSettings::off()` (and, for `tooltip.rs`, a zero tooltip delay) in the commit whose motion would move what it asserts. No assertion changes, and each motion is proved by its own manual-clock tests.

## Items and their state
- **Blocked on #76 directly:** G-44 #51, G-46 #53, G-57 #64, G-58 #65, G-63 #70, G-64 #71, G-65 #72.
- **Blocked through their dependencies:** G-51, G-53, G-59, G-60, G-61, G-62, G-66.
- **Adoption:** UI-R01 #331 needs G-44 and G-46. UI-R02 to UI-R06 (#332-#336) depend on UI-R01.

## Review
No diff to review.

## Test diffs
None.

## Release
None: nothing packaged changed.

## Next focus
When the owner grants #76, take G-44 (colour motion) and G-46 (size motion), then UI-R01 #331. Until then a pass under this steer has no workable item.
