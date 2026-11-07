# 2026-11-07 feel series: G-45 layout motion, built in bevy_gooey

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 7465bc2 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
No bevy_grimoire item in the epic is ready: UI-R01 #331 still needs G-44, G-45 and G-46. This pass claimed and built G-45 (bevy-coven/bevy_gooey#52) in `~/family_src/bevy_emerge_loop/bevy_gooey`. A node with `MovesSmoothly` that layout moves now travels to its new place instead of jumping.

## Baseline
The bevy_grimoire gate and `accept.py` were not run: no bevy_grimoire code changed.

## Enhancement: G-45 (bevy-coven/bevy_gooey#52)
- **Change:** `src/layout_motion.rs` adds `MovesSmoothly`, `LayoutTravel`, `mark_moving_parts` and `update_layout_motion`.
  - The system runs in `PostUpdate` after Bevy's layout. It springs a corner offset to 0 on `motion.default` and adds it to the `UiGlobalTransform` of the node and its subtree.
  - Bevy's layout rewrites every global each frame, so the patch is redone while the node moves, and nothing is written at rest.
  - A move lands in a node's first two frames, after a resize, scale, skin or parent change, on a parent scroll, and under reduced motion.
- **Assumed decisions (on the issue's label):**
  - **The drawing is moved through the computed `UiGlobalTransform`, not `UiTransform`.** The move is only known after layout. Writing `UiTransform` there would draw one frame at the new place before going back, and it would fight `MotionTarget` for the same component.
  - **Only tabs are marked.**
    - Marking toasts made two protected toast tests fail: their room already glides.
    - Marking status items failed the protected 200%-text status test: a fitting pass moves items a frame after layout.
    - Toolbar items are fitted the same way.
  - All of this is recorded in `docs/research/layout-motion.md`.
- **Proof:** `tests/layout_motion.rs`, 11 tests at 60 Hz.
  - The glide case: a block whose neighbour is taken out travels 70 px, no frame over a third of the move, and settles within the spring's settle time. Its child rides it.
  - An unmarked block jumps (the control).
  - Reduced motion lands the move.
  - A growing block stands still and the one under it travels.
  - A 1280×720 → 1600×900 resize leaves no offset on any block.
  - Tabs are marked, and toasts, status items and toolbar items are not.
  - A move in the frame after a node first appears lands.
  - A settled column writes nothing.
  - Taking the mark off stops the motion.
  - A `MotionTarget` slide is not read as a move.
  - A shown-again block does not fly in.
  - The API is new, so the file does not compile on e19492c.
- **bevy_gooey gate:** all green after the review fixes: fmt, clippy `--all-targets --all-features -D warnings`, nextest 1111/1111, doctests 248, the wasm check and rustdoc `-D warnings`.
- **Acceptance left to G-65:** the issue's two-toast item. G-65's own acceptance already says the same thing ("when the oldest leaves, the others travel without a jump"), and G-65 depends on this item, so it could not wait. This is recorded on #52 and #72.

## Borrowed from Mixar
None (the reference is Rune: `crates/rune/README.md`, "Animation").

## Performance
Not measured. One corner read per marked node per frame, and a subtree walk only while a node moves.

## Acceptance
`accept.py` was not run: no bevy_grimoire code changed.

## Review
The reviewer subagent ran and confirmed the core assumption: Bevy re-places every global each frame, so the patch does not pile up. All four P2 findings are fixed and tested:
1. Removing `MovesSmoothly` now stops the motion (`With<MovesSmoothly>`).
2. A node's own `UiTransform`, or an ancestor's scale or rotation, is no longer read as a move: the corner is read in the parent's space.
3. A node shown again after `Display::None` no longer flies in from its parent's corner.
4. The docs claimed dock tabs travel. They are respawned on each change, so they don't until G-63, and the docs now say so.

The three P3 findings are fixed too:
1. `FixedNode` subtrees are skipped.
2. The scratch lists are reused, so a move allocates nothing per frame.
3. The child-rides and corner-still checks now run on every frame.

## Test diffs
bevy_gooey: the new `tests/layout_motion.rs` and its `mod layout_motion;` line. No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. G-45 is in bevy_gooey's Unreleased section.

## Next focus
G-44 (colour motion) and G-46 (size motion), the rest of UI-R01 #331's needs.
