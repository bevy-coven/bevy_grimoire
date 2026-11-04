//! **The editor must not be able to fill a disk with one repeated log line.**
//!
//! Measured on `big`, 2026-11-03: an editor left running with no audio device wrote
//! `/tmp/e381e.log` to **13.5 GB in about five minutes** — `/tmp` there is a 16 GB tmpfs, so it is
//! RAM — and the file was almost entirely this, repeated 81,448,543 times:
//!
//! ```text
//! ERROR rodio::stream: audio stream error: A backend-specific error has occurred: `alsa::poll()` returned POLLERR
//! ```
//!
//! The mechanism is not a bug in this crate: `rodio`'s `default_error_callback` logs once per failed
//! `poll` (`rodio-0.22.2/src/stream.rs:164`), and ALSA returns `POLLERR` immediately and forever when
//! there is no device, so the callback never yields. `bevy_audio` is in `DefaultPlugins`, and the
//! editor — a desk tool that needs sound only for the audio panel's Audition — links it whether or
//! not a device exists.
//!
//! Two things fix it, and they are independent:
//!
//! 1. **Silence that one target.** [`LOG_SILENCES`] turns `rodio::stream` off, and it goes in
//!    `LogPlugin`'s `filter` field. An audio device that fails is a fact worth one line, not four
//!    million; every other target keeps its level.
//! 2. **Do not open a device when nothing needs it.** `bevy_audio`'s plugin is added only when the
//!    `bevy_siren` feature is, which is the only thing that plays a sound.
//!
//! **The silence is a directive, not a level, and that is deliberate.** `LogPlugin` builds its
//! `EnvFilter` as `"<level>,<filter>"` and appends `RUST_LOG` after that
//! (`bevy_log-0.20.0/src/lib.rs:422`), so the app's own level must survive this module untouched —
//! a `warn` written here would silence every `info` line the editor prints. Only
//! `rodio::stream` is named.
//!
//! **Two of these were wrong first, and the tests are why they are right now.** `log_filter(None)`
//! once returned the silences alone, and `LogPlugin`'s `info` then went *first* and won, silencing the
//! whole editor; then a `warn` default did the same thing more politely. Both are recorded in
//! `tests/the_log_cannot_fill_a_disk.rs`, which drives a real subscriber composed the way `LogPlugin`
//! composes it.
//!
//! **The limit, measured:** `EnvFilter` resolves directives **last-wins**, so an ambient
//! `rodio=trace` cannot re-enable this (a longer target beats a shorter one) but
//! `RUST_LOG=rodio::stream=trace` — the exact target, typed on purpose — does. That is the honest
//! bound of a filter-string fix, and it is why the second half matters: a build without `bevy_siren`
//! opens no audio device at all, so there is no failing `poll` to log.
//!
//! **Why a module rather than two literals in `main.rs`:** `main.rs` is a binary, so no test can
//! reach a filter written there. `tests/the_log_cannot_fill_a_disk.rs` drives a real subscriber with
//! this constant and composes it the way `LogPlugin` does.

/// **Targets the editor keeps quiet, whatever the ambient `RUST_LOG` says.**
///
/// `rodio::stream` is the one that filled a disk. `cpal` is included because a missing device is
/// reported by both, and the pair is what a person sees when the editor opens on a box with no sound.
/// Deliberately narrow: a directive here must name a library that cannot be trusted to rate-limit
/// its own errors, not a subsystem whose news is wanted.
pub const LOG_SILENCES: &str = "rodio::stream=off,cpal=off";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_that_filled_the_disk_is_named() {
        // If someone edits LOG_SILENCES and drops this target, the regression that cost 13.5 GB
        // comes back. The behavioural test in `tests/` is the real proof; this names the cost.
        assert!(LOG_SILENCES.contains("rodio::stream=off"), "{LOG_SILENCES}");
    }

    #[test]
    fn nothing_here_sets_a_level() {
        // A bare level (`warn`, `off`, `info`) in this constant would silence or enable the whole
        // editor, because `LogPlugin` puts its own level first and a later bare level replaces it.
        for directive in LOG_SILENCES.split(',') {
            assert!(
                directive.contains('='),
                "{directive:?} in LOG_SILENCES has no `=`, so it sets a *level* for everything \
                 rather than silencing one target"
            );
            assert!(directive.ends_with("=off"), "{directive:?} is not `=off`");
        }
    }
}
