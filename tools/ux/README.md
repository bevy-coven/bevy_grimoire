# UX checks

Dev-only, like `tools/demo`. Nothing here is shipped or run by the push gate: these checks need a display.

## The first-run walk

`first_run_walk.py` is the 2026-11-04 first-run audit as a script (#304). It starts the editor under its own Xvfb with a scratch `HOME` and a copy of `assets/` as the example project, then walks a newcomer's first minutes: the first-run home, **Open the example**, one piece placed, back to the project page, a new map, the Projects home, a new Starter project with a piece placed, and a new Empty project given a mesh by dropping a `.glb` on the window.

Each step is asserted through BRP (the editor's own state and the text it draws), never by pixels, and is screenshotted. Two promises are held at every step: the process that answers is the one that started (a project opens in the same process), and a first placement takes at most two clicks once a map is open.

```sh
cargo build -p bevy_cauldron
python3 tools/ux/first_run_walk.py --bin "$CARGO_TARGET_DIR/debug/bevy_cauldron"
python3 tools/ux/first_run_walk.py --bin "$CARGO_TARGET_DIR/debug/bevy_cauldron" --screen 1024x640   # the small screen
```

It prints one line per step and the scratch folder holding the screenshots and the editor's log. **Look at every screenshot**: the script checks state, and only a person (or a reviewing agent) sees an overlap, clipped text, an internal word or a second accent button. Run it by hand at the end of a phase of UI work, and delete the scratch folder afterwards.
