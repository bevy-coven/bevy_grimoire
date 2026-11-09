# Review — the editor must not die on a resize

Run: 2026-10-20-03. Three passes, judged on the captures in `evidence/`.

Every capture was taken by driving the **real editor** over BRP (`bevy_debugger/screenshot` with
`source: "offscreen"`, so the editor's own surface is read and no window is taken), against a copy
of Sanctuary's project. The "before" is not a picture: the crash was reproduced by **measurement**,
and the ratchet is proven to fail on the unfixed code.

---

## Pass 1 — Function: every state of the feature at full frame

**Verdict: GREAT**

### The states

| State | What it showed | Verdict |
|---|---|---|
| **The editor, opened from the command line on a map** | `MAP · town`, **727 placed**, the palette on the right, the inspector on the left, **213,046 tris drawn**. The editor **starts on the Map door and stays up**. | ✅ This is the state the ui_audit says was unreachable — *"`bevy_emerge_mapper . <map>` still dies inside two seconds"*, reproduced 4/4. |
| **A live window resize** (the compositor's own change) | The window was resized with AppleScript while the editor ran. The editor **survived**, the interface is unchanged, the map still draws. | ✅ The second half of the fix, live. |
| **The chrome bar, zoomed** | `MAP · town` and `213,046 tris drawn`, crisp at 1.9x. | ✅ The door's identity and the frame's cost, readable. |
| **The headless harness** | No window, so the 1536×864 placeholder stands, and `resize_surface` still resizes it. | ✅ Unchanged — the 1,219-test suite depends on it. |

### The proof that the crash is gone

The crash is a *race*, so a picture of a healthy editor is not evidence. The evidence is the
measurement, and it is in the probe (`examples/probe_surface_resize.rs`):

| | Baseline | With the fix |
|---|---|---|
| The image after the window changes size | 3440×1440 | 3440×1440 |
| What the cameras that share it believe | **3396×1356** | **3440×1440** |
| | **MISMATCH — the crash's frame** | agree |

**The two numbers are the audit's two numbers, verbatim** — 3396×1356 and 3440×1440 — and the
baseline mismatches for **exactly one frame**, which is the frame that assembles the invalid pass.

### The proof that the ratchet catches it

A regression test that passes on the broken code is a comment. Run against the **baseline**, the
new test fails with the crash's own numbers:

```
thread 'the_surface_is_created_at_the_windows_size' panicked at …:202:9:
assertion `left == right` failed: the camera at order -1 still believes the target is
(3396, 1356) while the image is (3440, 1440). A pass assembled from those two is refused by
wgpu, and the default error handler exits the application — this is the crash.
  left: (3396, 1356)
 right: (3440, 1440)
```

And it passes with the fix.

### One thing the first pass found, and it was in the *fix*

The first version of the fix created the surface at the window's size and stopped there. The
measurement said otherwise: the window's size **settles a frame later** (the compositor sizes it
after the first frame), so the surface was still resized — after the world camera existed, which
is the fatal case. **The fix needed both halves**, and only the measurement found that.

The second half is the one that matters: the editor writes the image's `AssetEvent::Modified`
itself, in `PreUpdate`, so `camera_system` refreshes its `target_info` in the **same frame** the
image changed. The engine's own writer cannot do this — it is scheduled after the camera reads
the message stream — but `AssetEvent` is a `Message`, and an application may write one.

---

## Pass 2 — Zoom: the chrome bar, cropped and upscaled

**Verdict: GREAT**

The chrome bar at 1.9x:

```
MAP · town                              213,046 tris drawn
```

* The type is **crisp** — the FiraMono face at 1.9x, no mush, the digits' counters clean.
* The **alignment** is the editor's own: the door's name is centred on the bar and the cost is
  flush right, and the two are the same size and the same grey.
* **No new colour, no new type role, no new spacing.** The fix changed *when* the surface is
  sized, not what is drawn — and the zoom is the evidence that nothing about the chrome moved.

**A note on what the zoom cannot show.** The crash was never a *picture* problem: it was a pass
assembled with two different attachment sizes, which the GPU refuses and the error handler turns
fatal. No amount of zooming would have found it. What the zoom does show is the thing the fix
*could* have broken — the chrome, the panels, the world — and it shows them unchanged.

---

## Pass 3 — Fresh eyes: a developer who has never seen the editor

**Verdict: GREAT**

A **clean copy** of Sanctuary's project, the editor started **from the command line on a map** —
the way a developer would actually start it:

```
cargo run -p bevy_emerge_mapper -- /path/to/project town --kit town
```

and the editor **opens on the map and stays open**. The Map door is the editor's primary surface
and it is now reachable without going through the chooser.

### Can I discover it?

There is nothing to discover: **there is no feature.** The editor does not grow a button, a mode,
a key, or a word. What changed is that a thing that used to *die* now *works* — the door opens.

That is the shape a bug fix should have. A crash has no interface to learn.

### Can I do the task?

I open the map, I see 727 placements, the palette, the inspector, and the town. I can pan, filter,
arm a piece, place it, undo, save — the whole loop, in the editor, on the first try, because the
application is running.

### Do I trust it?

Two things say yes, and neither is a promise:

* The **count** is honest: `727 placed`, and the file has 727 placements. The triangle count moves
  with the view.
* **The window can be resized and the editor keeps working** — which is the ordinary case a
  developer hits every time they drag the window, and the case the audit says was fatal.

### What a fresh eye would still ask

"Can I resize the window?" — yes, and it survives. "Does the editor open on a map?" — yes. There is
no third question, because there is no feature to learn.

---

## Summary

| Pass | Verdict | Note |
|---|---|---|
| 1 — Function | **GREAT** | The Map door opens from the command line and a live resize survives; the mismatch is measured away, and the ratchet fails on the baseline. |
| 2 — Zoom | **GREAT** | The chrome is unchanged and crisp; the fix moved *when* the surface is sized, not what is drawn. |
| 3 — Fresh eyes | **GREAT** | There is no feature to learn — there is a door that opens, and a window that can be dragged. |

Three GREEN reviews. Shipped.

### Evidence

| File | What it shows |
|---|---|
| `evidence/01-the-map-door-opens.png` | The editor, opened from the command line on a map: `MAP · town`, 727 placed, 213,046 tris drawn. |
| `evidence/02-after-a-live-window-resize.png` | The same editor after a live window resize — alive, and still drawing. |
| `examples/probe_surface_resize.rs` | The measurement, kept so the next person can re-run it rather than re-derive it. |
