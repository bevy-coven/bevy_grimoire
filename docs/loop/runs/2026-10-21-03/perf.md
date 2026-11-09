# Performance: 2026-10-21-03 — the editor's own eyes, over MCP

## The numbers that matter

### A still capture, over MCP, end to end

Measured 2026-10-21, against a live editor with Sanctuary's `town` map open
(727 placements, 300 x 20 x 1120 m), on the MCP server's process boundary.

    $ emerge_capture_still {"label": "live_probe"}
    isError: false
    the content blocks: ['image', 'text']
      an image: 1363909 bytes, mime image/png
      the answer's path: .../target/loop/captures/live_probe/still-1792604122449.png
      the state in it: [300.0, 20.0, 1120.0] dirty False
    in 206 ms

**206 ms** for a full-frame PNG of the editor **and** the state, over the wire.

**The PNG is 1.36 MB**, and that is the honest size of a 2560x1406 image of this editor. It is
**not** compressed for transport, and it should not be: the MCP `image` content block is base64
PNG, and the alternative — a smaller image — would be a different picture, not a smaller copy of
this one.

**For a caller that does not want the picture**, `emerge_state` returns the same state as JSON in
a few kilobytes, and it is the tool for that job. **The two tools exist because the two jobs are
different**: a machine wants the state, a model wants the pixels.

### A sequence, over MCP, end to end

    $ emerge_capture_sequence {"label":"live_seq2","seconds":1.0,"fps":4,
                               "operation":{"name":"map_set_settings",
                                            "parameters":{"axis":"y","axis_value":48.0}}}
    status: done | frames in the manifest: 4 | images returned: 4
    operation applied at: 0.121 s | frame 0 at: 0.061 s
    the operation's result: map is 300 x 48 x 1120 m, and the file was written
    in 880 ms

**880 ms** for four full frames and the operation between them.

**The timing is the editor's, not the server's.** The four frames are due at 0.00, 0.25, 0.50 and
0.75 s — the manifest's `t_s` — and the operation went in at 0.121 s, which is **after frame 0 was
taken at 0.061 s and before frame 1 was due at 0.25 s**. The editor's clock drove it; the server
made one call and waited.

**The server's round trip is not in the timing.** This is the whole point of the design: the
server's two round trips (capture, then apply) would have had no ordering between them, and the
readback lands a frame or two after the request. **On the editor's side, the ordering is a
property of the code.**

### The frames themselves

| | |
|---|---|
| resolution | 2560 x 1406 (the editor's own surface) |
| bytes per frame | 1,363,909 (PNG, uncompressed for transport) |
| frames at 4 fps | ~5.5 MB per second of sequence |
| a 10-second sequence | 40 frames, ~55 MB |

**The 10-second cap is a disk cap, and it is there because of this row.** An unbounded sequence
is an unbounded number of files, and the editor's `target/` directory is not a place to fill
without a bound.

### The headless case

**No frames.** The headless harness has no render world, so a screenshot never produces a
readback. The run still happens — the schedule, the operation, and the manifest — and the run
**says why**:

    {"error":"this editor has no render world, so no frame could be written",
     "frames":[...4 entries with their due times...]}

**A headless run is a schedule and an operation, and it is honest about not being a picture.**
That is the same property the whole feature rests on.

## The cost in the editor

| | |
|---|---|
| a new module | `sequencer.rs` — the clock, the frames, the operation's moment |
| a new resource | `Sequencer` (the runs) and `CanRender` (whether a frame is possible) |
| a system | one, in `Phase::Sense` — the same phase the other senses run in |
| per frame | one `Screenshot` spawn and one PNG write, and nothing else |

**The sequencer is idle when no run is in flight.** The system returns on the first line if there
is no run, and no frame is scheduled, no observer is spawned, and no file is touched.

**The operation goes through the registry, not around it.** `crate::ops::dispatch` is the same
call the editor's own size field makes, so a sequence cannot apply an operation the editor could
not have applied by hand.

## What was not measured

**The cost of a capture on the editor's frame time.** The screenshots are spawned from
`Phase::Sense`, and each one is a readback of the whole surface. **The editor was not measured
at 60 fps while capturing**, and the honest statement is that a capture is a readback per frame
and the editor's own cost of that is not in these numbers. The sequence's 4 fps is a *deliberate*
rate, not a measured limit — **the cap is a design decision, and the editor has not been shown
to be unable to go faster.**
