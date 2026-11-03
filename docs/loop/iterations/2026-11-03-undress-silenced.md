# 2026-11-03 elements: the editor survives a record run

Focus: #282 / owed clips · Commits: 21871c5..de2c642 · Machine: big · Outcome: fixes only

## Summary

A fourth `bevy_elements` despawn defect was fixed at the source: `get_entity` resolves when a command is
*queued*, but the removals run at the next flush, so every one of them could still hit a despawning
camera and close the window. All are `queue_silenced` now, the crate's dossier suite is 5/5 with no
command errors, and the editor survived a full record run for the first time. The clip still does not
ship, because the measurement says the scrubber does not move the sky.

## Bug fixed (in `bevy_coven/bevy_elements`, pinned by `de2c642`)

`undress_camera`'s commands were guarded by `get_entity`, which is the wrong check. It resolves at queue
time; the commands run later, and `Remove<ElementsCamera>` fires while the entity is **despawning**.
Measured on big 2026-11-03, after the previous pass's guards, five kinds of command still reported
`Entity despawned` and closed the window with `EXIT=101`:

```
remove::<DepthPrepass>            remove::<VolumetricFog>
remove::<DistanceFog>             remove::<SkyBackground>
remove::<(AtmosphereSettings, AtmosphereEnvironmentMapLight)>
remove::<ElementsDressing>
```

Every command in the system is now `queue_silenced` (`0778a62`): a camera that is gone has nothing to
undress and its components went with it.

**Proved:** `bevy_elements`' `tests/gpu_dressing.rs` **5 passed, 0 failed**, with no command errors in
the log at all — including `despawning_a_dressed_camera_does_not_panic`, which had failed on every
previous attempt.

## The clip: recorded end to end, still not shippable

The editor now stays up through a full storyboard: wheel, three `click_label` presses, 72 frames,
`events.json` written. The contact sheet shows the ENVIRONMENT panel with the hour buttons lighting in
turn. **But the sky does not follow the hour:**

| t | hour pressed | sky mean RGB |
|---|---|---|
| 1.0 | (before) | (124,153,190) |
| 2.8 | 6:00 | (112,137,170) |
| 4.8 | 18:30 | (123,147,178) |
| 7.0 | 23:00 | (119,143,173) |

All daylight blue, and the 23:00 frame in the sheet is a bright blue sky. Against the **file** path on
the same build — `(128,157,194)` at noon, `(27,30,35)` at 23:00 — the scrub path plainly does not move
the preview.

So the changelog line's words (*"the clock scrubber holds the preview at an hour of day"*) are false as
written, and `98-clock-scrub` keeps its `not yet recorded` reason. A clip whose 23:00 frame is midday
blue would be exactly the false caption the docs rule forbids.

**Reported on #282** with the discriminator for the two remaining causes: read `WorldClock`'s hour over
BRP after pressing `23:00`. If it reads 23.0, the crate's sky is not following its own clock; if 12.0,
`scrub_clock` or `ClockScrub` is not running. `WorldClock` is not in `world.list_resources` on this
build, so that check needs a small `grimoire_state` field first.

## A storyboard lesson worth keeping

The first attempt scrolled 12 notches, which put `6:00` at **y=6** — inside the top bar — and the click
navigated to the launcher (`door: null`), which looked like the panel vanishing. Scrolling 12 then back
4 put all three buttons at `y=488`, inside the panel, and every click landed. **A storyboard that scrolls
a panel must scroll to a position it has checked, not to a maximum.**

## Test diffs

None in this repository. The crate gained the silencing and its suite proves it.

## Review

Reviewer could not run. Five questions, mine: (1) is the fix load-bearing — the dossier suite went from
5 failing-with-errors to 5 passing with none; (2) did I ship a clip I could not show — no, and the
measurement is on the issue; (3) is the sky measurement fair — it is the same region and build as the
file-path measurement that does move, so the comparison holds; (4) did I confuse a harness fault with a
product fault — the earlier `door: null` was a harness/scroll fault and is now diagnosed as such; (5)
what remains — the `WorldClock` read, then the fix in whichever crate owns it.

## Release

None here: nothing packaged changed (`bevy_cauldron` is `publish = false`). The crate fix is on
`bevy_elements`' `main`.

## Next focus

Expose `WorldClock`'s hour through `grimoire_state`, press `23:00`, and read it — that separates the two
remaining causes on #282.
