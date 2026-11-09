# Perf — the editor must not die on a resize

Run: 2026-10-20-03. What the change costs, measured.

## The number that matters

**The editor opens.** Before, `bevy_emerge_mapper . <map>` died inside two seconds, reproduced 4/4 —
so the editor's primary surface was unreachable from a terminal, and every session that started
that way ended at the chooser instead. That is not a speed; it is the difference between a
workflow and a detour.

## The change's cost

**One extra message write, and one extra query, on the frames where the surface resizes.**

| | Before | After |
|---|---|---|
| `spawn_surface` | the image is created at 1536×864 and resized on the first frame with a window | the image is created at the window's physical size, and **is not resized at startup** |
| `fit_surface_to_window` | resizes the image in `Update` | resizes the image in `PreUpdate` and **writes one `AssetEvent::Modified`** when it does |
| The steady state | — | **unchanged**: the system already compared the sizes before writing, and it still does |

**The steady-state cost is zero.** The resize only happens when the window's physical size changes —
on a display change, or a drag that crosses a scale factor. The event is written only then, so a
frame that does not resize writes nothing.

**The image is created at the window's size**, which is the same allocation the old code did on
its first frame — it has simply moved earlier, and it is now the *right* size the first time.

## The measurement

`crates/bevy_emerge_mapper/examples/probe_surface_resize.rs` reports the image's size and the
sizes the cameras believe, frame by frame, for a window that changes size the way the compositor
changes one.

**Baseline** (the fix reverted):

```
at the first frame: image (1536, 864), the cameras that share it [(-1, (0, 0)), (-3, (0, 0))]
the window is now [(3440, 1440)]
after one frame: image (3440, 1440), the cameras that share it
  [(-1, (3396, 1356)), (-3, (3396, 1356))] -> MISMATCH
```

**With the fix:**

```
at the first frame: image (3396, 1356), the cameras that share it [(-1, (0, 0)), (-3, (0, 0))]
the window is now [(3440, 1440)]
after one frame: image (3440, 1440), the cameras that share it
  [(-1, (3440, 1440)), (-3, (3440, 1440))] -> agree
```

**One frame of mismatch, in the baseline; none, with the fix.** The two numbers in the baseline's
mismatch are the audit's two numbers, verbatim.

## The suite

| | Time |
|---|---|
| `cargo test --workspace --no-fail-fast` | 253 s (1,221 pass, 1 fail — `no_two_leaders_cross`, red since 2026-09-03 and not this work's) |
| `cargo test -p bevy_emerge_mapper --test the_surface_and_the_camera_agree` | 0.36 s for 2 tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | **no finding in any file this change touches** |
| `cargo fmt --check` | `surface.rs` keeps its **pre-existing** 3 diffs; the new files are clean |
| `cargo doc --no-deps` | no warning from any link this change adds |
| Headless boot against Sanctuary's real project | `town` → 12 frames, 8,192 entities, no panic; `town_block` → 4,096 entities, no panic |
| Sanctuary's own suite | 1,014 pass, 4 fail — the same four that were already red |

## The live check

The editor was started from the command line on Sanctuary's town map, resized **with the window
still running** (AppleScript, the compositor's own change), and screenshotted afterwards:

* it **survived** — the process was alive after the resize;
* it **still drew** — the interface unchanged, the map intact, the triangle count moving with the
  view as it settled.

That is the check a crash needs and a unit test cannot give: **the thing that used to die, did
not.**
