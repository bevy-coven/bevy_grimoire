# Plan — the editor must not die on a resize

Run: 2026-10-20-03. Feature: **`bevy_emerge_mapper . <map>` opens the Map door, and a resize
never kills the application.**

## The decision

**Two changes, and the measurement says both are needed.**

1. **The surface's image is created at the window's physical size.** The window is spawned by
   `WindowPlugin::build`, and `DefaultPlugins` is added before the editor's plugins in both entry
   points — so the size is known before the image is made, and the image never has to be resized at
   startup.
2. **A resize announces itself in the frame the camera reads it.** The editor writes the image's
   `AssetEvent::Modified` itself, in `PreUpdate`, before `camera_system` reads the message stream
   in `PostUpdate`.

### Why this and not the alternatives

The audit records two fixes already eliminated by measurement and says so: *"What is left, and what
the next person should measure first"*. This run measured it, and the answer is precise:

1. `Camera3d` asks for a depth texture sized to `camera.computed.target_info.physical_size`
   (`bevy_core_pipeline-0.20.0/src/core_3d/mod.rs:667`).
2. `camera_system` refreshes `target_info` only when the image fires an `AssetEvent::Modified`
   (`bevy_render-0.20.0/src/camera.rs:391`), and it reads that from the **message** stream.
3. `Assets::asset_events` — the only writer of that stream — is scheduled in `PostUpdate` in
   `AssetEventSystems`, and `camera_system` is `.before(AssetEventSystems)`. So the camera reads
   the stream **before** the flush writes to it, and learns about a resize **one frame late**.

The frame in between assembles the pass: a depth texture sized for the old extent and a colour
attachment for the new one. wgpu refuses a pass whose attachments differ, and the default error
handler treats that as fatal — so the application exits rather than dropping a frame.

- **Not "make the camera see the resize sooner."** The flush is Bevy's and it is scheduled after
  the camera reads it. Moving the editor's resize does not move Bevy's flush, so the event stays
  one frame late — which is exactly what the audit's second eliminated fix measured.
- **Not "swallow the validation error."** A validation error here means the renderer assembled a
  pass with attachments of two different sizes. Swallowing it leaves a renderer drawing an invalid
  pass, which is the *crippled program* the Pragmatic Programmer's Tip 38 warns against: *"a dead
  program normally does a lot less damage than a crippled one."* This is a bug, not exceptional
  data, so it gets fixed rather than absorbed.
- **Not "make the world camera not 3-D."** The depth attachment is not the fault; the *stale
  size* is. A 3-D camera needs a depth attachment, and the editor's world is 3-D.

**Why the editor may write the event.** `AssetEvent` is a `Message`, and an application may write
one. The engine's own writer cannot be scheduled earlier, so the editor writes its own — with the
same id and the same variant, so the camera cannot tell the difference.

### The measurement that decided it

`crates/bevy_emerge_mapper/examples/probe_surface_resize.rs` reports the image's size and the
sizes the cameras believe, frame by frame, for a window that changes size the way the compositor
changes one.

**Baseline** (the fix reverted):

```
the window is now [(3440, 1440)]
after one frame: image (3440, 1440), the cameras that share it
  [(-1, (3396, 1356)), (-3, (3396, 1356))] -> MISMATCH
```

**With the fix:**

```
the window is now [(3440, 1440)]
after one frame: image (3440, 1440), the cameras that share it
  [(-1, (3440, 1440)), (-3, (3440, 1440))] -> agree
```

**The baseline mismatches for exactly one frame, and that frame is the crash.** The fix never
mismatches — at the first frame, during a window resize, or after.

The two numbers in the baseline's mismatch — **3396×1356** and **3440×1440** — are the audit's
two numbers, verbatim.

## The shape of the change

### `crates/bevy_emerge_mapper/src/surface.rs`

- **`spawn_surface` reads the window's physical size** and creates the image at it. A headless run
  has no window and keeps the 1536×864 placeholder.
- **`fit_surface_to_window` moves to `PreUpdate`** and **writes the image's `AssetEvent::Modified`
  itself** when it resizes the image, so the camera refreshes its `target_info` in the same frame.
  Everything else it does — the density in `UiScale`, the mirror's scale — is unchanged.
- **The window camera is not in the invariant.** It renders to the *window*, not the image, so its
  target size is a different question and it is excluded from the ratchet deliberately.

### `crates/bevy_emerge_mapper/tests/the_surface_and_the_camera_agree.rs`

The ratchet for the bug. **It fails on the baseline** and passes with the fix — measured, not
assumed:

```
thread 'the_surface_is_created_at_the_windows_size' panicked at …:202:9:
assertion `left == right` failed: the camera at order -1 still believes the target is
(3396, 1356) while the image is (3440, 1440). A pass assembled from those two is refused by
wgpu, and the default error handler exits the application — this is the crash.
  left: (3396, 1356)
 right: (3440, 1440)
```

### `crates/bevy_emerge_mapper/examples/probe_surface_resize.rs`

The measurement, kept as an example so the next person can re-run it rather than re-derive it.

## Touched modules

| Crate | Module | Change |
|---|---|---|
| `bevy_emerge_mapper` | `surface.rs` | the surface is created at the window's size; a resize announces itself in the frame the camera reads it |
| `bevy_emerge_mapper` | `examples/probe_surface_resize.rs` (new) | the measurement |
| `bevy_emerge_mapper` | `tests/the_surface_and_the_camera_agree.rs` (new) | the ratchet |
| Sanctuary (consumer) | — | **none.** |

## Tests

1. **The surface is created at the window's size** — with a window entity of 3396×1356, the
   image is 3396×1356 from the first frame.
2. **A window resize never leaves the cameras behind** — the window changes to 3440×1440, one
   frame runs, and every camera that shares the image believes the same size the image is. **This
   is the crash, and it fails on the baseline.**
3. **A headless run is unchanged** — no window, so the 1536×864 placeholder stands, and
   `resize_surface` still resizes it.

## Schema impact

**None.** No file on disk changes; no crate other than the editor is touched.

## Risks

- **The editor writes an engine's message stream.** That is a supported thing to do — `AssetEvent`
  is a `Message` — but it is the kind of move that needs its reason written down, and it is written
  down at the call site. The alternative is a one-frame-late camera, which is the crash.
- **A window resize that changes only the *logical* size does not resize the image**, and the
  surface stays at its physical size — which is correct: the image is sized in physical pixels, and
  a logical resize that does not change the physical size does not change the render.
- **The window camera is excluded from the ratchet**, because it renders to the window rather than
  to the image. That exclusion is a *claim*, and the probe is what measures it: the window camera's
  target size tracks the window, and the other two track the image.
