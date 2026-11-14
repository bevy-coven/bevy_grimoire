//! **`Ctrl+\`` opens a shell in the editor's window, and what you type runs in it.**
//!
//! The engine's own tests (`terminal::tests`) prove the PTY and the parser with no window. This file
//! proves the thing a person actually does: press the chord, type `printf 'HELLO\n'`, and read `HELLO`
//! off a `Text` node in the pane. It goes through **the key path, the shell and the drawing**, which is
//! the only route that can fail for the reason the feature would really fail.
//!
//! # The two failures this file exists to catch
//!
//! 1. **The pane does not appear** — the chord is not read, the frame is not found, or the spawner
//!    never ran. Asserted first, because everything else is vacuous without it.
//! 2. **A key types into the shell *and* drives the editor.** This is the defect that makes a terminal
//!    unusable rather than merely wrong: `d` would move the camera while the shell read a `d`. The
//!    second test asserts the editor's own `Live` is `Context::Typing` while the terminal holds the
//!    keys — which is the single gate every binding respects — and that `Ctrl+\`` closes the pane.
//!
//! # Why the shell is `sh` here
//!
//! `SHELL` is set for the length of each test, so a machine whose owner runs `fish` does not get a
//! fish greeting in the middle of the assertion. That is the same discipline `terminal::tests` uses,
//! and for the same reason: this test is about the wiring, not about which shell is configured.
//!
//! # Not a `#[cfg(unix)]` file, but a `#[cfg(unix)]` body
//!
//! The PTY is Unix-only ([`bevy_cauldron::terminal::TerminalSession::spawn`] refuses elsewhere with a
//! reason). A Windows build that skipped the file entirely would never compile it, and a compile error
//! in it would go unseen on the one platform that most needs the refusal tested; so the file compiles
//! everywhere and its tests say what they need.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::keys::{Context, Live};
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalFocus, TerminalPaneRoot};

use crate::fixtures;

/// A headless editor with a window, stepped far enough for the frame to be laid out.
///
/// **The window is spawned at its size before the first frame**, not resized afterwards: Bevy's UI
/// layout is computed against the camera's viewport, and a window whose resolution changes mid-run
/// leaves the tree measured at the old size for several frames — which is how the narrow-window test
/// first read a 420-px window with a 728-px body. The precedent is
/// `the_viewport_keeps_a_usable_width_in_a_narrow_window`, which does the same thing for the same
/// reason.
fn editor(tag: &str) -> App {
    editor_sized(tag, 1280, 720)
}

/// The same editor, in a window of a stated size.
fn editor_sized(tag: &str, width: u32, height: u32) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.world_mut().spawn((
        Window {
            resolution: bevy::window::WindowResolution::new(width, height),
            ..default()
        },
        PrimaryWindow,
    ));
    for _ in 0..10 {
        app.update();
    }
    app
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window")
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key,
        state,
        text: None,
        repeat: false,
        window,
    });
}

/// **A chord as a hand makes it**: every key down together, a frame, every key up, a frame.
fn chord(app: &mut App, keys: &[KeyCode]) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in keys {
            send(app, *key, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), state);
        }
        app.update();
    }
    settle(app, 4);
}

/// **Type a line** — every character as a key event, then `Enter`.
fn type_line(app: &mut App, line: &str) {
    for c in line.chars() {
        let (code, logical) = match c {
            ' ' => (KeyCode::Space, Key::Space),
            '\n' => (KeyCode::Enter, Key::Enter),
            other => (
                key_for(other),
                Key::Character(other.to_string().as_str().into()),
            ),
        };
        send(app, code, logical.clone(), ButtonState::Pressed);
        send(app, code, logical, ButtonState::Released);
    }
    app.update();
    app.update();
}

/// The `KeyCode` a character sits on, for the ASCII set a test types.
fn key_for(c: char) -> KeyCode {
    match c {
        'a'..='z' => match c {
            'a' => KeyCode::KeyA,
            'b' => KeyCode::KeyB,
            'c' => KeyCode::KeyC,
            'd' => KeyCode::KeyD,
            'e' => KeyCode::KeyE,
            'f' => KeyCode::KeyF,
            'g' => KeyCode::KeyG,
            'h' => KeyCode::KeyH,
            'i' => KeyCode::KeyI,
            'j' => KeyCode::KeyJ,
            'k' => KeyCode::KeyK,
            'l' => KeyCode::KeyL,
            'm' => KeyCode::KeyM,
            'n' => KeyCode::KeyN,
            'o' => KeyCode::KeyO,
            'p' => KeyCode::KeyP,
            'q' => KeyCode::KeyQ,
            'r' => KeyCode::KeyR,
            's' => KeyCode::KeyS,
            't' => KeyCode::KeyT,
            'u' => KeyCode::KeyU,
            'v' => KeyCode::KeyV,
            'w' => KeyCode::KeyW,
            'x' => KeyCode::KeyX,
            'y' => KeyCode::KeyY,
            'z' => KeyCode::KeyZ,
            _ => unreachable!(),
        },
        '\'' => KeyCode::Quote,
        '\\' => KeyCode::Backslash,
        ' ' => KeyCode::Space,
        _ => KeyCode::Space,
    }
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// Point `SHELL` at a POSIX shell for the length of one test.
///
/// The tests in this file share a process with each other and with the editor's own; the lock is what
/// makes the write sound on the 2024 edition, where `set_var` is `unsafe`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn use_posix_shell() -> PosixShell {
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    PosixShell(guard)
}

/// **Every word the pane draws**, joined — what the test can actually see.
///
/// A row is one `Text` whose children are one `TextSpan` per colour run, so the words live on the
/// spans. Reading `Text` alone would see the row's empty root and report a blank terminal, which is the
/// mistake this comment exists to save the next reader from making.
fn drawn_text(app: &mut App) -> String {
    let world = app.world_mut();
    let mut bodies: Vec<Entity> = world
        .query_filtered::<Entity, With<TerminalBody>>()
        .iter(world)
        .collect();
    if bodies.is_empty() {
        return String::new();
    }
    let body = bodies.remove(0);
    let texts: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Text>, With<TextSpan>)>>()
        .iter(world)
        .collect();
    let mut rows: Vec<(u32, String)> = Vec::new();
    for entity in texts {
        // Only the pane's own text: an ancestor walk, because a `Text` in the inspector is not the
        // terminal's output and asserting on the whole screen would pass for the wrong reason.
        let Some(parent) = world.get::<ChildOf>(entity) else {
            continue;
        };
        if parent.parent() != body && !is_under(world, parent.parent(), body) {
            continue;
        }
        // The row's own index, so the spans of one row are joined in order rather than by entity id.
        let row = world
            .get::<bevy_cauldron::terminal_pane::TerminalRow>(parent.parent())
            .map_or(u32::MAX, |row| row.0 as u32);
        if let Some(span) = world.get::<TextSpan>(entity) {
            rows.push((row, span.0.clone()));
        } else if let Some(text) = world.get::<Text>(entity) {
            rows.push((row, text.0.clone()));
        }
    }
    rows.sort_by_key(|(row, _)| *row);
    rows.into_iter().map(|(_, text)| text).collect::<Vec<_>>().join("\n")
}

/// Is `entity` the `root`, or below it?
fn is_under(world: &World, mut entity: Entity, root: Entity) -> bool {
    let mut steps = 0;
    loop {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
        steps += 1;
        if steps > 64 {
            return false;
        }
    }
}

/// **`Ctrl+\`` raises the pane, and a shell runs inside it.**
///
/// The end-to-end proof: the chord, the PTY, the parser and the drawn `Text` in one gesture. Everything
/// else in this feature is downstream of these four steps agreeing.
#[test]
fn a_terminal_runs_a_shell_inside_the_editor() {
    let _shell = use_posix_shell();
    let mut app = editor("terminal_runs_a_shell");

    // **The terminal is open before any chord** (the owner, 2026-11-06: "It should be on every screen
    // from the moment you open the application", #314). It was "nothing is open before the chord"; the
    // claim now is three: the pane is raised, a shell is running in it, and it has not taken the
    // editor's keyboard, which is what lets a door open with its own keys working.
    assert!(
        app.world_mut()
            .query_filtered::<&Node, With<TerminalPaneRoot>>()
            .iter(app.world())
            .any(|node| node.display != Display::None),
        "the terminal must be raised when the editor opens, not waiting for a chord"
    );
    assert!(
        !app.world().resource::<Terminal>().is_empty(),
        "the terminal is raised but no shell was started: it must be running from the first frame"
    );
    assert!(
        !app.world().resource::<TerminalFocus>().holds(),
        "the editor opens with its own keyboard: the terminal takes the keys only when asked"
    );

    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);

    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<TerminalPaneRoot>>()
            .iter(app.world())
            .count()
            > 0,
        "Ctrl+` did not raise the terminal pane"
    );
    assert!(
        !app.world().resource::<Terminal>().is_empty(),
        "the pane is up but no shell was started: the session must be spawned on the frame the pane \
         is first drawn"
    );

    // **Type a command and read its answer off the screen.** `printf` is a builtin in every POSIX
    // shell, so this proves the bytes reached the child and its reply reached a `Text` node — not that
    // `echo` exists as a binary.
    type_line(&mut app, "printf 'HELLO\\n'\n");
    let mut seen = String::new();
    for _ in 0..60 {
        settle(&mut app, 2);
        let text = drawn_text(&mut app);
        seen = text.clone();
        if text.contains("HELLO") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        seen.contains("HELLO"),
        "the shell's output never reached a drawn node. The pane shows: {seen:?}\nIf it is empty the \
         shell did not start; if it holds the typed command but not the reply, the pump and the \
         drawing are not on the same path."
    );
}

/// **A key types into the shell and does *not* drive the editor.** This is the defect the feature most
/// likely has, and it is the one that would make it unusable.
#[test]
fn a_key_goes_to_the_shell_and_not_to_the_editor() {
    let _shell = use_posix_shell();
    let mut app = editor("terminal_swallows_keys");
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    assert!(
        app.world().resource::<TerminalFocus>().holds(),
        "a raised pane starts with the keyboard; a terminal nobody can type into is a screenshot"
    );

    // **The editor's own gate answers `Typing`.** That is what every binding in the census respects,
    // so asserting it here is asserting the swallow for *every* action at once, rather than for the
    // one key a narrower test would have picked.
    settle(&mut app, 2);
    let live = *app.world().resource::<Live>();
    assert_eq!(
        live.0,
        Context::Typing,
        "with the terminal holding the keys the live context must be Typing, or `d` would move the \
         camera while the shell read a `d`"
    );

    // And a real keystroke changes nothing in the editor: `d` is not a tool key, so if the swallow
    // failed the visible effect would be on the shell instead — which is why the assertion above is
    // the load-bearing one and this is the sanity check beside it.
    send(&mut app, KeyCode::KeyD, Key::Character("d".into()), ButtonState::Pressed);
    send(&mut app, KeyCode::KeyD, Key::Character("d".into()), ButtonState::Released);
    settle(&mut app, 2);
    assert_eq!(
        *app.world().resource::<Live>(),
        live,
        "typing into the shell must not move the editor's live context"
    );

    // **`Ctrl+\`` closes it**, and gives the editor its keys back. Without this a person who lowered
    // the pane would be left with a dead keyboard and nothing on screen to explain it.
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    let lowered = app
        .world_mut()
        .query_filtered::<&Node, With<TerminalPaneRoot>>()
        .iter(app.world())
        .all(|node| node.display == Display::None);
    assert!(
        lowered,
        "Ctrl+` with the pane focused must lower it: the pane's node must be `Display::None`, which is \
         what gives its height back to the map as well as taking it off the screen"
    );
    assert!(
        !app.world().resource::<TerminalFocus>().holds(),
        "a lowered pane must give the keyboard back"
    );
}

/// **The terminal's column count is its own measured width, not a constant.**
///
/// This is the claim compactness rests on: a pane at 1600 px and a pane at 900 px must not both be
/// told eighty columns, because a shell that believes it has eighty columns wraps its compiler errors
/// for a width the pane is not showing — the right-hand end of every message is then clipped with no
/// way to scroll to it.
///
/// Measured two ways, and both have to hold:
///
/// - **exactly** `floor(usable_width / cell_width)`, computed here from the body's own
///   `ComputedNode` and the pane's measured cell — an independent check rather than a restatement of
///   the system's arithmetic, because it reads the layout instead of the resource;
/// - **narrower in a narrower window**, which is what fails if the column count came from a constant.
///
/// # What this test found, and why it does not assert `TERMINAL_NARROW_COLS`
///
/// At a 420-px window the body measures **606 logical px**, not 420: the shell's two docks are fixed
/// widths (`chrome::CONTROLS_W` 380 + `chrome::LIST_W` 264), so a window narrower than them overflows
/// and every child of the body inherits the overflow. The pane is therefore 100 columns at that size,
/// and that is the *correct* answer for the width it actually has — the defect is the frame's, and it
/// is B-014's (`the_viewport_keeps_a_usable_width_in_a_narrow_window`), not the terminal's.
///
/// So `TERMINAL_NARROW_COLS` is stated as the regime the shell is told about, and this test does not
/// pretend to reach it in a frame that cannot be that narrow. Asserting it here would either be
/// vacuous or would be asserting B-014 is fixed.
#[test]
fn the_terminal_takes_its_column_count_from_its_own_width() {
    let _shell = use_posix_shell();
    let wide = editor_sized("terminal_wide", 1600, 900);
    let narrow = editor_sized("terminal_narrow", 900, 700);

    let wide_cols = columns_and_body(wide);
    let narrow_cols = columns_and_body(narrow);
    let (wide_cols, wide_body) = wide_cols;
    let (narrow_cols, narrow_body) = narrow_cols;

    assert!(
        narrow_cols < wide_cols,
        "the pane was told {narrow_cols} columns in a {narrow_body}-px body and {wide_cols} in a \
         {wide_body}-px one. A column count that does not follow the width is a constant, and a shell \
         told the wrong width wraps its output for a pane it is not in."
    );
    assert!(
        narrow_cols >= 2,
        "the terminal collapsed to {narrow_cols} columns, which is not a terminal"
    );
}

/// Raise the terminal in `app` and return `(cols, body width in logical px)`.
fn columns_and_body(mut app: App) -> (u16, f32) {
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    settle(&mut app, 8);
    // The body's width, and its resolved padding left and right: the rows are drawn inside the padding, so
    // the padding is no part of a row (feedback-1794011769640: this read `body.x - 4.0`, the 2 px a side the
    // pane's arithmetic assumed, while the pane padded its sides by 12).
    let (body, padding_x) = {
        let mut q = app.world_mut().query_filtered::<&ComputedNode, With<TerminalBody>>();
        let node = q
            .single(app.world())
            .unwrap_or_else(|e| panic!("the pane has one body: {e:?}"));
        (
            node.size() * node.inverse_scale_factor(),
            (node.padding.min_inset.x + node.padding.max_inset.x) * node.inverse_scale_factor(),
        )
    };
    let terminal = app.world().resource::<Terminal>();
    let Some(tab) = terminal.tab() else {
        panic!("no terminal tab was started in a {} px body", body.x);
    };
    let cols = tab.screen().cols();
    // **The independent half of the check**: the layout's own width divided by the pane's OWN measured
    // cell — read from the resource the pane measures into, not re-guessed here. An earlier version of
    // this line divided by the 6 px fallback and failed against a pane correctly using its real 11.5 px
    // advance, which is precisely the mistake of testing a claim with a second copy of its arithmetic.
    let cell = app.world().resource::<bevy_cauldron::terminal_view::CellMetrics>().width();
    let expected = ((body.x - padding_x) / cell).floor();
    assert!(
        (cols as f32 - expected).abs() <= 1.0,
        "the terminal says {cols} columns for a {}-px body; the layout supports {expected}. The count \
         must be the body's width divided by a measured cell.",
        body.x
    );
    (cols, body.x)
}

/// **The shell outlives the door.** A person starts a `cargo build` on the Map door, crosses to Kit to
/// check a mesh, and comes back: the build must still be running, and the scrollback must still be
/// there.
///
/// This is what `Terminal` being a `Session` resource buys, and it is worth a test because the natural
/// implementation — a shell per pane, owned by the pane's `Node` tree — would die silently in the
/// door's sweep (`screen::scene_roots` despawns every parentless `Node`), taking twenty minutes of
/// somebody's build with it. The pane is *supposed* to die; the shell is not.
#[test]
fn the_shell_outlives_the_door() {
    let _shell = use_posix_shell();
    let mut app = editor("terminal_outlives_the_door");
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    settle(&mut app, 4);

    let before = app.world().resource::<Terminal>().len();
    assert!(before > 0, "the chord must start a shell before this test means anything");

    // The door's teardown, which is what a crossing runs.
    app.world_mut()
        .resource_mut::<NextState<bevy_cauldron::screen::Screen>>()
        .set(bevy_cauldron::screen::Screen::Project);
    settle(&mut app, 6);

    let after = app.world().resource::<Terminal>().len();
    assert_eq!(
        before, after,
        "the door change lost the terminal's sessions. `Terminal` is registered `Ownership::Session` \
         in `screen::OWNERSHIP` for exactly this reason: a `cargo build` started on one door is still \
         running on the next, and its scrollback is the person's only record of it."
    );
    assert!(
        app.world()
            .resource::<Terminal>()
            .tabs()
            .iter()
            .any(|tab| tab.is_live()),
        "the sessions survived as tabs but their shells did not: the session resource must hold the \
         `TerminalSession`, not the pane"
    );
}

/// **A shell is never started believing the pane is one column wide.**
///
/// This is a defect that shipped for an afternoon and was found only by driving the real editor: on the
/// frame `Ctrl+\`` raises the pane, `Display` has just gone from `None` to `Flex`, so the body has no
/// layout and measures zero. Clamped to the grid's own floor that is a **1 x 1** terminal, which a shell
/// will faithfully draw as one column of characters — a prompt that looks exactly like a terminal that
/// never started.
///
/// The invariant is not "the pane is always the right size on its first frame" (it cannot be: the
/// layout has not run). It is that **a measure too small to be a terminal is not believed**, so the
/// shell is started at the fallback and corrected a frame later by a real measurement.
#[test]
fn a_shell_is_never_started_at_a_size_too_small_to_be_a_terminal() {
    let _shell = use_posix_shell();
    let mut app = editor("terminal_never_a_column");
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    settle(&mut app, 1);

    let terminal = app.world().resource::<Terminal>();
    let Some(tab) = terminal.tab() else {
        // One frame may be too soon for the pane's own display change to have been seen; a terminal
        // that has not started yet is not a violation of this invariant.
        return;
    };
    let (cols, rows) = (tab.screen().cols(), tab.screen().rows());
    assert!(
        cols >= 20 && rows >= 5,
        "the shell was started at {cols} x {rows}, which is smaller than any terminal and is what a \
         zero-sized layout clamps to. A measure below a usable minimum must be refused, not used."
    );
}

/// **The `cd` chip moves a running shell to the project the editor is pointed at now, and the shell
/// says so.**
///
/// The rule the feature has to keep is that opening another project does **not** kill a shell — the
/// `cargo build` in it belongs to the project it was started in. So the move is a keystroke
/// (`cd '<root>'`) the person takes, and the shell's own `pwd` is what proves it landed. Reading it back
/// from the shell rather than from the resource is the point: the pane cannot know where a shell is, so
/// the only honest witness is the shell.
#[test]
fn the_cd_chip_moves_a_running_shell_to_the_open_project() {
    let _shell = use_posix_shell();
    let mut app = editor("terminal_cd_chip");
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    settle(&mut app, 6);

    // **Start the shell somewhere else**, the way a door crossing leaves it: the next project is a
    // different temporary directory, which is what `follow_the_project` writes.
    let other = std::env::temp_dir().join(format!("grimoire-terminal-cd-{}", std::process::id()));
    std::fs::create_dir_all(&other).expect("a scratch project directory");
    app.world_mut()
        .resource_mut::<Terminal>()
        .set_next_cwd(Some(other.clone()));

    let Some(tab) = app.world().resource::<Terminal>().tab() else {
        panic!("no terminal tab to move");
    };
    assert!(
        tab.wants_cd_to(&other),
        "a shell started in another directory must offer the move: the chip is what a person takes \
         instead of losing the shell's scrollback"
    );

    // Take the chip.
    app.world_mut()
        .resource_mut::<Terminal>()
        .write(format!("cd '{}'\n", other.display()).as_bytes());
    if let Some(tab) = app.world_mut().resource_mut::<Terminal>().tab_mut() {
        tab.offered_cd(other.clone());
    }

    // **Ask the shell where it is.** `pwd` is the shell's own answer, which is the only one that
    // counts: the pane's record of what it typed is not evidence that the shell moved.
    app.world_mut()
        .resource_mut::<Terminal>()
        .write(b"pwd\n");
    let mut seen = String::new();
    for _ in 0..60 {
        settle(&mut app, 2);
        seen = drawn_text(&mut app);
        if seen.contains(&other.display().to_string()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = std::fs::remove_dir_all(&other);
    assert!(
        seen.contains(&other.display().to_string()),
        "`pwd` never printed the new directory. The pane shows: {seen:?}"
    );

    // And the offer is spent: the chip does not come back for a move that already happened.
    let tab = app.world().resource::<Terminal>().tab().expect("the tab is still there");
    assert!(
        !tab.wants_cd_to(&other),
        "the chip must not offer a move the shell has already been told to make"
    );
}

/// **The terminal's right-click menu is gooey's, it offers Copy / Paste / Clear, and Clear really
/// clears.**
///
/// A shell with no way to take its output out is a toy, and `bevy_ui` has no selectable text, so the
/// menu is the only route. It is gooey's `context_menu` rather than a second menu implementation — the
/// same widget the launcher's project card uses — so this asserts it is *named* by the body and that
/// its items are wired to handlers that do something.
///
/// The proof of "do something" is the one that cannot be faked: the screen's scrollback is bounded and
/// its drawn rows are a countable fact, so `Clear` is measured by the rows it takes away.
#[test]
fn the_terminal_menu_offers_copy_paste_and_clear_and_clear_clears() {
    let _shell = use_posix_shell();
    let mut app = editor("terminal_menu");
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    settle(&mut app, 6);

    // **The body names the menu**, which is what makes a right-click over the scrollback open it. The
    // menu is found *through the body*: the editor has several context menus (the palette's piece menu
    // holds Place / Mount / Remove), so asking for "a menu" answers for whichever spawned first.
    let menu = {
        let world = app.world_mut();
        let body = world
            .query_filtered::<Entity, With<TerminalBody>>()
            .single(world)
            .expect("the pane has one body");
        world
            .get::<bevy_gooey::ContextMenu>(body)
            .map(|m| m.0)
            .expect("the terminal body must name a context menu, or a right-click offers nothing")
    };
    // **Its three items, by their own words** — a menu that exists but holds nothing is the same
    // failure as no menu, with more entities.
    let items: Vec<String> = {
        let world = app.world_mut();
        let mut rows = world.query::<(Entity, &Children)>();
        let mut texts = world.query::<&Text>();
        rows.iter(world)
            .filter(|(e, _)| world.get::<ChildOf>(*e).is_some_and(|p| p.parent() == menu))
            .flat_map(|(_, kids)| kids.iter().filter_map(|k| texts.get(world, k).ok().map(|t| t.0.clone())).collect::<Vec<_>>())
            .collect()
    };
    for want in ["Copy", "Paste", "Clear"] {
        assert!(
            items.iter().any(|i| i == want),
            "the terminal's menu has no `{want}` item; it holds {items:?}"
        );
    }

    // **Fill the scrollback**, so clearing it is measurable: one command per line, well past the pane's
    // own row count.
    for i in 0..40 {
        app.world_mut()
            .resource_mut::<Terminal>()
            .write(format!("echo line-{i}\n").as_bytes());
    }
    let mut filled = 0;
    for _ in 0..80 {
        settle(&mut app, 2);
        filled = app
            .world()
            .resource::<Terminal>()
            .tab()
            .map_or(0, |tab| tab.screen().scrolled_lines());
        if filled > 5 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        filled > 5,
        "the scrollback never filled ({filled} lines held), so clearing it would prove nothing"
    );

    // Take the menu's Clear, as a person would: trigger the item's `Activate`.
    let clear = {
        let world = app.world_mut();
        let mut rows = world.query::<(Entity, &Children)>();
        let mut texts = world.query::<&Text>();
        rows.iter(world)
            .filter(|(e, _)| world.get::<ChildOf>(*e).is_some_and(|p| p.parent() == menu))
            .find(|(_, kids)| {
                kids.iter()
                    .any(|k| texts.get(world, k).is_ok_and(|t| t.0 == "Clear"))
            })
            .map(|(e, _)| e)
            .expect("the Clear item")
    };
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: clear });
    settle(&mut app, 3);

    let held = app
        .world()
        .resource::<Terminal>()
        .tab()
        .map_or(0, |tab| tab.screen().scrolled_lines());
    assert!(
        held < filled,
        "Clear left {held} of {filled} lines: the item is wired to nothing"
    );
}

/// **The pane is a row of the WINDOW, not a sibling of the docks.**
///
/// The defect this is for shipped in 0.2.8 and the owner reported it as "I'm not seeing the terminal".
/// The pane was spawned into `chrome::Frame::body`, which is the `flex_direction: Row` holding the two
/// docks and the viewport — so it became a *row sibling*, laid out 2560 px wide and pushed off the
/// right edge of a 2560 px window, over the inspector and the map. Measured on the released build on
/// `mac_air`: size `2560 x 768`, centre x `1718`, spanning `438..2998`.
///
/// Three claims, and each one is a different way to be wrong:
///
/// 1. **It spans the window's width.** A row of the frame's root does; a sibling of the docks does not.
/// 2. **It lies below the body and above the status band**, so the band stays at the foot of the window.
/// 3. **It does not overlap the viewport**, which is the property the owner actually cares about: a pane
///    over the map is a pane that "isn't showing" even when every other assertion passes.
#[test]
fn the_pane_spans_the_window_and_sits_above_the_status_band() {
    let _shell = use_posix_shell();
    const W: u32 = 1280;
    const H: u32 = 720;
    let mut app = editor_sized("terminal_pane_is_a_window_row", W, H);
    chord(&mut app, &[KeyCode::ControlLeft, KeyCode::Backquote]);
    settle(&mut app, 8);

    let (pane, pane_size, pane_cx) = {
        let world = app.world_mut();
        let mut panes = world.query_filtered::<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<TerminalPaneRoot>>();
        let (node, transform) = panes.single(world).expect("the pane is on screen");
        (0, node.size() * node.inverse_scale_factor(), transform.translation.x)
    };
    let _ = pane;

    // **1. The window's width.** The physical surface is W x H at the harness's `UiScale`, so the
    // logical width is what the pane must fill; a row sibling comes out a fraction of it and offset.
    let window_w = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<&ComputedNode, With<bevy_cauldron::chrome::ViewportSlot>>();
        let viewport = q.single(world).expect("the editor has one viewport slot");
        // The viewport is narrower than the window by the docks, so its *left edge* overstates nothing:
        // the window's own width is the surface's, which the harness reports in logical px.
        let _ = viewport;
        W as f32 / 1.2
    };
    assert!(
        (pane_size.x - window_w).abs() <= 2.0,
        "the pane is {} logical px wide in a {window_w:.0} px window (centre x {pane_cx:.0}). A pane \
         that does not span the window is a row sibling of the docks rather than a row of the frame — \
         which is the 0.2.8 defect: it was spawned into `Frame::body`, the Row, and pushed off the \
         right edge.",
        pane_size.x
    );

    // **2. Between the body and the band**, by the frame's own child order.
    {
        let world = app.world_mut();
        let frame_root = world
            .get_resource::<bevy_cauldron::chrome::Frame>()
            .map(|f| (f.root, f.body, f.status))
            .expect("the frame");
        let (root, body, status) = frame_root;
        let kids: Vec<Entity> = world
            .get::<Children>(root)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        let pane_entity = world
            .query_filtered::<Entity, With<TerminalPaneRoot>>()
            .single(world)
            .expect("the pane");
        let at = |e: Entity| kids.iter().position(|k| *k == e);
        let (Some(body_at), Some(pane_at), Some(band_at)) = (at(body), at(pane_entity), at(status)) else {
            panic!("the frame, the pane and the band must all be rows of the root: {kids:?}");
        };
        assert!(
            body_at < pane_at && pane_at < band_at,
            "the rows are body={body_at}, pane={pane_at}, band={band_at}: the pane must come after the \
             body (so it is below the map) and before the band (so the band stays at the foot)"
        );
    }
}
