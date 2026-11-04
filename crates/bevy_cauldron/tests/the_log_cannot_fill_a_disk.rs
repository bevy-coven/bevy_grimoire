//! **The filter the editor installs really drops the target that filled a disk** (#284).
//!
//! `src/log_filter.rs` has the measurement: an editor with no audio device wrote 13.5 GB to a 16 GB
//! RAM disk in about five minutes, 81,448,543 repetitions of
//!
//! ```text
//! ERROR rodio::stream: audio stream error: A backend-specific error has occurred: `alsa::poll()` returned POLLERR
//! ```
//!
//! The unit tests in `log_filter` check the constant. **This file checks the behaviour**, because a
//! filter string that parses is not a filter that silences. Each test installs a **real subscriber**
//! built the way `LogPlugin` builds its own — `<level>,<filter>`, then the ambient `RUST_LOG` after
//! that — emits real events on real targets, and reads back what was written. `with_default` scopes
//! it, so nothing here touches the process-global subscriber a test binary shares.

use std::io;
use std::sync::{Arc, Mutex};

use bevy::log::tracing_subscriber::fmt::MakeWriter;
use bevy::log::tracing_subscriber::EnvFilter;
use bevy_cauldron::log_filter::LOG_SILENCES;

/// What `LogPlugin` defaults its `level` to (`bevy_log`'s `Level::INFO`).
const APP_LEVEL: &str = "info";

/// A writer the test can read back.
#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Buf {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl io::Write for Buf {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Buf {
    type Writer = Buf;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// The filter `LogPlugin` ends up with, composed exactly as `bevy_log::LogPlugin::build_filter_layer`
/// does it: the app's `level`, then the plugin's `filter` (which is [`LOG_SILENCES`]), then the
/// ambient `RUST_LOG`'s directives one at a time.
fn composed(ambient: Option<&str>) -> String {
    let mut filter = format!("{APP_LEVEL},{LOG_SILENCES}");
    if let Some(env) = ambient.map(str::trim).filter(|s| !s.is_empty()) {
        filter.push(',');
        filter.push_str(env);
    }
    filter
}

/// Run `emit` under that filter, and return everything that reached the writer.
fn captured(ambient: Option<&str>, emit: impl FnOnce()) -> String {
    let buf = Buf::default();
    let filter = EnvFilter::try_new(composed(ambient))
        .unwrap_or_else(|e| panic!("the composed filter does not parse: {e}"));
    let subscriber = bevy::log::tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(buf.clone())
        .finish();
    bevy::log::tracing::subscriber::with_default(subscriber, emit);
    buf.text()
}

const ALSA_ERROR: &str =
    "audio stream error: A backend-specific error has occurred: `alsa::poll()` returned POLLERR";

/// **The exact line that filled the disk is dropped by default**, and by every ambient filter a
/// person would plausibly leave set while working on something else.
///
/// Measured, not assumed: `EnvFilter` resolves directives last-wins **per target**, so a directive
/// that names `rodio::stream` itself is the one thing that can undo this. That is the honest limit
/// of a filter-string fix, and it is why the second half of the fix exists — a build without
/// `bevy_siren` does not open a device at all — and why the cases below are the ones that matter:
/// none of them names the looping target, and none of them can bring the loop back.
#[test]
fn the_alsa_error_that_filled_a_ram_disk_never_reaches_the_writer() {
    for ambient in [
        None,                       // a plain `cargo run`, which is what filled the disk
        Some(""),
        Some("rodio=trace"),        // "let me look at audio", the likely way a person undoes this
        Some("trace"),              // everything
        Some("bevy_cauldron=trace,rodio=debug"),
    ] {
        let out = captured(ambient, || {
            bevy::log::tracing::error!(target: "rodio::stream", "{}", ALSA_ERROR);
        });
        assert!(
            !out.contains("POLLERR"),
            "the ALSA error reached the log with ambient {ambient:?}: {:?}. A plain run wrote 13.5 GB; \
             `rodio=trace` must not bring that back.",
            out.trim()
        );
    }
}

/// **The one escape hatch, stated so nobody "fixes" it by accident.** A person who types the
/// looping target's *full* path is asking for exactly this line; `EnvFilter` honours that, and this
/// test records the limit rather than pretending otherwise. If this ever starts failing, the
/// silencing got stronger and the comment above is what to update.
#[test]
fn naming_the_full_target_is_the_only_way_back_in() {
    let out = captured(Some("rodio::stream=trace"), || {
        bevy::log::tracing::error!(target: "rodio::stream", "{}", ALSA_ERROR);
    });
    assert!(
        out.contains("POLLERR"),
        "the filter now overrides even an explicit `rodio::stream=trace`. That is stricter than this \
         fix claimed; update this test and the note in `src/log_filter.rs` rather than deleting it"
    );
}

/// **The loop is what fills a disk, not one line**: a thousand of them write nothing at all.
#[test]
fn a_thousand_failures_write_nothing() {
    let out = captured(Some("rodio=trace"), || {
        for _ in 0..1000 {
            bevy::log::tracing::error!(target: "rodio::stream", "{}", ALSA_ERROR);
        }
    });
    assert!(out.is_empty(), "1000 silenced events wrote {} byte(s):\n{out}", out.len());
}

/// **And the fix is narrow**: silencing the loop must not silence the editor.
///
/// A filter that dropped everything would pass the tests above and would make the editor unreadable.
/// These are the targets a pass and a person actually read, at the levels they are written at.
#[test]
fn the_editors_own_lines_still_reach_the_log() {
    let out = captured(None, || {
        bevy::log::tracing::info!(target: "bevy_cauldron::project", "project: 90 descriptor(s)");
        bevy::log::tracing::info!(target: "bevy_cauldron::thumbs", "baked 90 thumbnails");
        bevy::log::tracing::warn!(target: "bevy_gltf::loader", "Unknown vertex attribute COLOR_1");
        bevy::log::tracing::error!(target: "bevy_cauldron::watch", "the map did not read");
    });
    for expected in [
        "90 descriptor(s)",
        "baked 90 thumbnails",
        "Unknown vertex attribute COLOR_1",
        "the map did not read",
    ] {
        assert!(
            out.contains(expected),
            "{expected:?} was silenced; the fix must be the audio target alone, or the editor stops \
             saying what it is doing. Got:\n{out}"
        );
    }
}

/// **A person's `RUST_LOG` still works for everything it names**, including an audio target that is
/// not the looping one — so a debug session keeps its news and loses only the loop.
#[test]
fn an_ambient_filter_is_kept_for_its_own_targets() {
    let out = captured(Some("bevy_siren=debug"), || {
        bevy::log::tracing::debug!(target: "bevy_siren::mix", "a bus is armed");
    });
    assert!(out.contains("a bus is armed"),
            "the caller asked for bevy_siren=debug and lost it:\n{out}");
}
