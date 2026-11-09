# Research — the editor must not die on a resize

Run: 2026-10-20-03. Feature: **the Map door must open from the command line, and a resize must not
kill the application.**

## The problem, in one sentence

`bevy_emerge_mapper . <map>` — the command that opens the editor straight onto a map — **dies inside
two seconds**, and the editor's primary surface is unreachable from a terminal.

## The evidence, measured

### What the audit recorded, and what is still true

`crates/bevy_emerge_mapper/docs/ui_audit.md` §F3 records the crash, reproduced **4/4**, and names
what is left to find:

```text
Attachments have differing sizes: the depth attachment's texture view has extent (3396, 1356, 1)
but is followed by the color attachment at index 0's texture view which has (3440, 1440, 1)
Quitting the application due to Validation RenderError
```

Those two numbers are **the window before and after the compositor finished sizing it**, and
`bevy_render`'s default error handler treats a validation error as **fatal** — so the application
exits rather than dropping a frame.

The audit records **two fixes eliminated by measurement**: a stale viewport (clamped, kept, but the
crash is unchanged) and a resize landing after the cameras read it (byte-identical crash, so the
cameras do not disagree about *when* they read the target).

### What this run found, by reading the engine

The audit says to measure one thing first, and it is the right thing: **three cameras share one
image and only the 3-D one has a depth attachment.** A probe
(`crates/bevy_emerge_mapper/examples/probe_surface_resize.rs`) confirms the structure:

```
  camera order  -3: 3d=false target=Some((1536, 864))     ← the ground, which clears
  camera order   0: 3d=false target=None                    ← the window
  camera order  -2: 3d=true  target=Some((1536, 864))     ← the world — the only 3-D one
  camera order  -1: 3d=false target=Some((1536, 864))     ← the interface
```

**Four cameras; three of them share one image; the world camera is the only one that asks for a
depth attachment.** So the pass that carries the world is the only pass with a second attachment,
and it is the only pass that can have attachments of different sizes.

Now the *why*, which is three facts read out of Bevy 0.20's source:

1. **`Camera3d` asks for a depth texture sized to the target.**
   `prepare_core_3d_depth_textures` (`bevy_core_pipeline-0.20.0/src/core_3d/mod.rs:667`) builds
   the descriptor with `size: physical_target_size.to_extents()`, where
   `physical_target_size` is `camera.computed.target_info.physical_size` — the size of the image
   **as it was when the camera last refreshed its target info**.

2. **A camera refreshes its target info only when it hears the image changed.**
   `camera_system` (`bevy_render-0.20.0/src/camera.rs:391`) refreshes `target_info` when
   `normalized_target.is_changed(&changed_window_ids, &changed_image_handles)`, and for an image
   target that is `changed_image_handles.contains(&image_target.handle.id())` — **an
   `AssetEvent::Modified` on the image**.

3. **The editor resizes the image in `Update`, and the camera reads it in `PostUpdate`.**
   `Assets::asset_events` — the system that *writes* those `AssetEvent`s into the message stream —
   is scheduled in **`PostUpdate`**, in `AssetEventSystems`. And `camera_system` is scheduled in
   `PostUpdate` **`.before(AssetEventSystems)`**.

So the order within one frame is:

| When | What | Effect |
|---|---|---|
| `Update` | `fit_surface_to_window` calls `image.resize(...)` — the image is now the **new** size, and an `AssetEvent::Modified` is **queued** | the texture is new; the camera does not know |
| `PostUpdate` → `camera_system` | reads the `AssetEvent` message stream — **empty**, because the queued event has not been flushed yet | `target_info` keeps the **old** size |
| `PostUpdate` → `asset_events` | flushes the queued `AssetEvent::Modified` into the message stream | now the camera *can* see it |
| **the render** | the depth texture is sized from the stale `target_info`, the colour attachment is the resized image | **two sizes in one pass** |
| next frame → `camera_system` | sees the event, refreshes `target_info` to the new size | one frame too late |

**The event is one frame late, and the frame in between is the one that assembles the mismatched
pass.** That is the crash, and it is why it is a *startup* crash: the surface is created at
1536×864 and resized to the window's physical size on the first frame that has a window, so the
mismatch happens exactly once, at boot, and the application is dead before the author sees
anything.

### Why the chooser path survives

The audit measured it: booting through the chooser never crashes, 0/4. And the reason is the same
race, won. By the time the editor's cameras exist, the menu has been on screen for many frames, the
image has settled at the window's size, and **the resize that would have mismatched has already
happened and been absorbed** — the camera's `target_info` was refreshed on a frame where the image
had not yet changed size, and the next frame found no change to report.

**So the bug is not "the editor cannot draw into a resized image". It is "the editor cannot survive
the *first* resize", and the chooser happens to sit in front of the first resize.**

## Sources

| # | Source | What it was used for |
|---|---|---|
| S1 | Akenine-Möller et al., *Real-Time Rendering*, 4th ed., ch. 23 "Graphics Pipelines" (in the local corpus: `book_real_time_rendering_4th_akenine`) | Double and triple buffering: what a *pending* buffer is for, and that a resize is a change of attachment identity, not a change of content |
| S2 | Scarpino, M., *The WebGPU Sourcebook*, CRC Press, 2024, app. B (in the corpus) | A render pass's colour attachment and what wgpu requires of the attachments it is given |
| S3 | Hunt, A., Thomas, D., *The Pragmatic Programmer*, 20th anniversary ed., Tip 38 "Crash Early" (in the corpus: `hunt_pragmatic_programmer_20th`) | The distinction that matters here: a **bug** is worth crashing on; **exceptional data** is worth handling gracefully |
| S4 | Gregory, J., *Game Engine Architecture*, 3rd ed., ch. 11 and 16 (in the corpus) | The engine's frame structure as a set of ordered phases, and the "batched update" discipline that puts subsystem updates in one place rather than per-object |
| S5 | *Bevy* 0.20 source, read directly: `bevy_camera-0.20.0/src/camera.rs`, `bevy_render-0.20.0/src/camera.rs`, `bevy_core_pipeline-0.20.0/src/core_3d/mod.rs`, `bevy_asset-0.20.0/src/assets.rs` | The mechanism, quoted above — this is the load-bearing source, and it is the engine this project is built on |

## Technique

### 1. A pass's attachments must agree, and the engine will not negotiate

S2 is blunt about what wgpu requires: a render pass is given a set of attachments, and the pass is
*assembled* with them. Two attachments of different sizes is not a warning — it is an invalid pass,
and the validation error is reported to the error handler, which by default is fatal.

S1 supplies the vocabulary for the fix. In a double-buffered renderer, the back buffer is drawn
into and the front buffer is shown; when the target's size changes, **the back buffer has to be a
different object**, because a texture has one size. The "pending" buffer of triple buffering exists
precisely so that a *new* buffer can be prepared while the old one is still being shown — so the
swap is never a moment where two sizes are in flight.

**The editor's surface is exactly this case, and the engine already has the mechanism**: the depth
texture is keyed by the target's size, so a resize *does* get a new depth texture. The bug is that
the camera hands the renderer the **old** size for one frame, and the renderer dutifully builds a
pass with a depth texture for the old size and a colour attachment for the new one.

### 2. A one-frame-late event is a bug, not exceptional data

S3's distinction is the one to apply here, and it cuts both ways.

**Exceptional data** is "a user might type something invalid" — and the right answer is to report it
and keep going. **A bug** is "something that was supposed to be impossible just happened" — and
S3's advice is *"crash earlier, and crashing is often the best thing you can do"*, because
*"a dead program normally does a lot less damage than a crippled one."*

**This is a bug**: the editor's own code resizes an image without telling the camera. It is not
exceptional data, it is not a malformed file, and it is not a user error. So the fix is **not** to
swallow the validation error — that would leave a renderer drawing a pass with mismatched
attachments, which is the "crippled program" S3 warns about.

The fix is to make the **event arrive in the frame it needs to**.

### 3. The frame structure is the design, so the fix is a frame-ordering fix

S4's engine chapter is about *batched updates*: a subsystem's state is updated in one place, at
one point in the frame, rather than scattered across the objects that use it. The editor's surface
is exactly such a subsystem, and its update is in the wrong place relative to the renderer's.

**The rule the fix follows:** *a resource's change must be visible to every reader of that
resource before the frame's render reads it.* Concretely — the surface is resized in a phase that
runs **before** the camera's own refresh, so the camera's `target_info` and the image's texture
are always the same size by the time the pass is assembled.

**Where that phase is.** The camera refreshes in `PostUpdate`, `.before(AssetEventSystems)`, and the
asset events are flushed in `PostUpdate` in `AssetEventSystems`. So the resize must happen **before
`camera_system`** and its `Modified` event must be **visible to `camera_system` in the same
`PostUpdate`**.

There is exactly one window where both are true: the resize must run in **`PreUpdate`** — before
`PostUpdate` — and its event must be flushed before `camera_system` reads it. The asset events are
flushed in `PostUpdate`, which is *after* `camera_system`… so the event is **still** one frame
late.

**Which means the resize cannot be signalled by an `AssetEvent` at all.** The camera's own
change-detection reads the *message* stream, and the message is written after the camera has
looked. The honest fix is to **not resize the image in place**: the camera's `target_info` comes
from the image's *current* size, so the image must be the size the camera will read.

There is a cleaner way to see it. `camera_system` refreshes `target_info` when the image's
`AssetEvent` says it changed. The image is resized in `Update`. The event is flushed in
`PostUpdate`, **after** the camera. So the camera learns about the resize one frame late **by
construction**.

**The fix that follows from the mechanism, not from the guess:** the surface's image must not
change size under the cameras' feet. Two ways:

* **(a) Size the image once, before any camera exists**, and never resize it — the window's size
  is read at startup and the surface is *created* at that size. Then there is no resize, no
  mismatch, and no event to be late.
* **(b) Resize the image, and tell the camera in the same frame** — by writing the event by hand,
  or by making the camera's target info follow the image.

(a) is the boring answer and the one the evidence supports: the surface's whole design is "one
image, sized to the window", and the *only* reason it is resized is that it starts at a
placeholder size. **If it starts at the right size, nothing has to be resized at all.**

## What great looks like

Acceptance criteria, phrased so a reviewer could fail the implementation against them.

1. **`bevy_emerge_mapper . <map>` opens.** The editor starts on the Map door, on a real project,
   and stays up. Measured by running the command and watching it survive past the point the audit
   says it dies.
2. **The surface is created at the window's size**, so there is no first-frame resize and no
   mismatched pass. A probe asserts the image's size equals the window's physical size from the
   first frame that has a window.
3. **A later resize — the window changing size, as the compositor does — does not leave the
   cameras behind.** The audit's two numbers, 3396×1356 and 3440×1440, are the two ends of that
   change, and the invariant is that **the image and every camera that shares it are the same
   number on every frame**, including the frame right after the change.
4. **The fix is not "swallow the validation error."** The error handler stays fatal; the
   *mismatch* stops happening. A test asserts the surface's image and the camera's target info
   agree, so the pass can never be assembled with two sizes.
5. **The ratchet fails on the broken code.** A regression test that passes on the baseline is a
   comment, not a test — so the test must be run against the baseline and seen to fail, with the
   crash's own numbers in the message.
6. **The headless harness is unaffected.** It has no window, so it keeps its placeholder size, and
   `resize_surface` still works — that is how a test asks for a shape.
7. **A refusal, not a degradation.** A surface that cannot be sized (no window, a zero size) is
   handled as it is today: the placeholder stands, and nothing is written.

## Gaps

The literature does not settle three things, and this project must decide them on its own.

- **Whether to size the image once or to resize it safely.** The literature's answer is the
  triple-buffering one: a target that changes size needs a *new* buffer, prepared while the old
  one is still shown. Bevy's own mechanism does that for the depth texture (it is keyed by size),
  so the editor's job is only to keep the camera's idea of the size in step with the image's.
  **This project's answer**, and it is *both*: the surface is created at the window's size, so the
  startup needs no resize at all; and a resize that does happen announces itself in the frame the
  camera reads it. The first is the fix; the second is the rule that makes the first unnecessary
  to think about twice.
- **How to keep them in step without a one-frame-late event.** Bevy's `camera_system` reads the
  `AssetEvent` *message* stream, and the flush is scheduled after the camera reads it — so an
  in-place resize is always one frame late, and the editor cannot move Bevy's flush. **This
  project's answer**: the editor writes the event itself, in `PreUpdate`. `AssetEvent` is a
  `Message`, and an application may write one; the editor writes the same id and the same variant
  the engine would have, so the camera cannot tell the difference — and it reads the message in
  the same frame the image changed.
- **What the *window* camera does.** It renders to the *window*, not the image, so its target
  size is the window's — a different question, and not the crash. The measurement says so: it
  keeps the old size after a resize while the two that share the image follow correctly, and it
  never had a depth attachment to mismatch. **This project's answer**: it is excluded from the
  ratchet, deliberately and with the reason written down — an exclusion that is a *claim* needs
  a measurement behind it, and the probe is that measurement.
