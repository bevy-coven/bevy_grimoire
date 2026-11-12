//! **A rig's clip is reached by `Cmd+P`, its name and `Enter`** (NV-11, #391; series getting around, #377).
//!
//! `docs/ui/navigation.md` measured a clip at a workspace chord and a click in the bench's list, against a target of the finder: the
//! finder indexed rigs and no clips, so a clip could not be named. A clip is a slot of a rig's manifest (`rigs.yaml`): the bench plays it
//! when its chip is soloed. So a clip is a row of the finder, and choosing it opens the Animation workspace on its rig with that slot soloed.
//!
//! The project here has one rig with two clips, and the finder is driven the way a person drives it: the chord, the clip's name typed
//! a key at a time, `Enter`. Nothing is queued by calling the reveal.
//!
//! Owner exception (2026-11-12): a_rigs_clip_is_found_by_the_finder.rs: main's new test wrote .ron input; converted to the YAML spelling.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy_cauldron::anim_stage::BenchScrub;
use bevy_cauldron::anim_tab::BenchState;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given between one key and the next.
const SETTLE_FRAMES: usize = 40;

/// The rig, and the clip a person asks the finder for. The clip is slot 1 of the manifest, so that a finder that only found the rig (or
/// the first slot) lands on the wrong answer.
const RIG: &str = "hound";
const CLIP: &str = "gallop";
const CLIP_SLOT: usize = 1;

/// A rig with an idle and a gallop. The mesh is never opened: the finder reads names from the manifest.
const RIGS: &str = "\
version: 3
rigs:
  hound:
    mesh: characters/hound.glb
    scale: 1.0
    slots:
    - clip: 0
      playback: !Free
        speed: 1.0
      note: idle
    - clip: 1
      playback: !Free
        speed: 1.0
      note: gallop
";

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("clip_finder_{tag}_{}", std::process::id()))
        .descriptor("stool", "alpha")
        .place("stool", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("assets/grimoire/rigs.yaml"), RIGS).unwrap_or_else(|e| panic!("rigs.yaml: {e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// One key event to the primary window.
fn key(app: &mut App, code: KeyCode, logical: Key, text: Option<&str>, state: ButtonState) {
    let window = app.world_mut().query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().single(app.world()).expect("one primary window");
    app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: text.map(Into::into), repeat: false, window });
    app.update();
}

/// The key a lower-case letter is typed with.
fn letter(c: char) -> KeyCode {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI,
        KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR,
        KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    ];
    assert!(c.is_ascii_lowercase(), "the driver types lower-case letters, not `{c}`");
    LETTERS[usize::from(c as u8 - b'a')]
}

/// **Cmd+P, the name, Enter.**
fn ask_the_finder_for(app: &mut App, name: &str) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        key(app, primary(), unidentified(), None, state);
        key(app, KeyCode::KeyP, unidentified(), None, state);
    }
    settle(app);
    let open = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_gooey::CommandPalette>>().iter(world).next().is_some()
    };
    assert!(open, "Cmd+P did not open the finder");
    for c in name.chars() {
        let s = c.to_string();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            key(app, letter(c), Key::Character(s.as_str().into()), (state == ButtonState::Pressed).then_some(s.as_str()), state);
        }
    }
    settle(app);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        key(app, KeyCode::Enter, Key::Enter, None, state);
    }
    settle(app);
}

#[test]
fn the_clips_name_in_the_finder_opens_its_rig_with_that_clip_soloed() {
    let mut app = editor("solo");
    ask_the_finder_for(&mut app, CLIP);
    let workspace = *app.world().resource::<Workspace>();
    assert_eq!(workspace, Workspace::Animation, "`{CLIP}` chosen in the finder left the editor on the {} workspace", workspace.title());
    assert!(*app.world().resource::<Mode>() == Mode::Anim, "the Animation workspace did not open the rig bench");
    let bench = app.world().resource::<BenchState>();
    let selected = bench.names().get(bench.selected).map(|n| (*n).to_owned());
    assert_eq!(selected.as_deref(), Some(RIG), "the bench is on {selected:?}, not on the clip's rig");
    let solo = app.world().resource::<BenchScrub>().solo;
    assert_eq!(solo, Some(CLIP_SLOT), "the clip `{CLIP}` is slot {CLIP_SLOT}; the stage has {solo:?} soloed");
}
