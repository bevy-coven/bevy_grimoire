//! **A family settings file that is RON but not the crate's own type is refused naming the file and the
//! field, and the last good settings stay** (#200, H3, kind `family`).
//!
//! The host used to call a settings file `applied` once it was valid RON, and left the crate's own type
//! to refuse it later on the status line, so `grimoire_state.reloads` said a file landed that the crate
//! had thrown away. In a build that has the crate (`bevy_elements`, `bevy_gooey`) the file is now read as
//! the crate's own type before it is taken: a refusal is one verdict that names `family/<crate>.ron` and
//! the field the type is wrong at, the text never reaches [`FamilySettings`], and a file mended back
//! (or to anything else the type reads) is `applied`. The three bad files of the issue's acceptance are,
//! for each crate: a file cut off (not RON), a field of the wrong type, and (for a crate with a required
//! field) a field missing.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::family_host::FamilySettings;
use bevy_grimoire_mapper::live::EditorReloads;
use serde_json::Value;

use crate::fixtures;

/// One crate's settings: a text its type reads, its three bad texts with a word the field must contain,
/// and what the status line says stays.
struct Case {
    krate: &'static str,
    good: String,
    other_good: String,
    bad: Vec<(&'static str, String, &'static str)>,
    stays: &'static str,
}

#[cfg(feature = "bevy_elements")]
fn elements() -> Case {
    use bevy_grimoire_mapper::environment::ElementsSettings;
    let good = ElementsSettings::defaults().to_ron();
    let clock = ron::to_string(&bevy_elements::WorldClock::default()).unwrap();
    let climate = ron::to_string(&bevy_elements::Climate::default()).unwrap();
    let mut other = ElementsSettings::defaults();
    other.clock.hour = 18.5;
    Case {
        krate: "bevy_elements",
        other_good: other.to_ron(),
        bad: vec![
            ("corrupt: a save cut off", good[..good.len() / 2].to_owned(), ""),
            ("wrong type: the clock is a number", format!("(clock: 5, climate: {climate})"), "clock"),
            ("missing field: no climate", format!("(clock: {clock})"), "climate"),
        ],
        good,
        stays: "previous environment stays",
    }
}

fn gooey() -> Case {
    Case {
        krate: "bevy_gooey",
        good: "(accent: Srgba((red: 0.2, green: 0.6, blue: 0.9, alpha: 1.0)))".to_owned(),
        other_good: "(min_target: 30.0)".to_owned(),
        bad: vec![
            ("corrupt: a save cut off", "(accent: Srgba((red: ".to_owned(), ""),
            ("wrong type: the accent is a number", "(accent: 5)".to_owned(), "accent"),
            ("missing field: a colour without its green", "(accent: Srgba((red: 0.2)))".to_owned(), "accent"),
        ],
        stays: "previous skin stays",
    }
}

fn cases() -> Vec<Case> {
    vec![
        #[cfg(feature = "bevy_elements")]
        elements(),
        gooey(),
    ]
}

fn reloads(app: &App) -> Value {
    app.world().resource::<EditorReloads>().state_json()
}

fn verdict(app: &App, file: &str) -> Value {
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "family" && v["file"] == file).cloned()).unwrap_or(Value::Null)
}

fn seq(app: &App) -> u64 {
    reloads(app)["seq"].as_u64().unwrap()
}

fn until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 8.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never saw: {what}; reloads {}", reloads(app));
}

fn pump(app: &mut App, secs: f64) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn settings(app: &App, krate: &str) -> Option<String> {
    app.world().resource::<FamilySettings>().text.get(krate).cloned()
}

fn project(tag: &str, case: &Case, text: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join(format!("{}.ron", case.krate));
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, text).unwrap();
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    (app, file)
}

#[test]
fn a_file_that_is_not_the_crates_type_is_refused_naming_the_field_once_and_the_last_good_stays() {
    for case in cases() {
        let name = format!("family/{}.ron", case.krate);
        let (mut app, file) = project("family_typed", &case, &case.good);
        assert_eq!(verdict(&app, &name)["outcome"], "applied", "{}: a file that reads is applied at launch", case.krate);
        assert_eq!(settings(&app, case.krate).as_deref(), Some(case.good.as_str()));

        for (what, bad, field) in &case.bad {
            let before = seq(&app);
            let raised_before = app.world().resource::<EditorState>().status.raised();
            std::thread::sleep(Duration::from_millis(50));
            std::fs::write(&file, bad).unwrap();
            until(&mut app, &format!("{}: {what}: the refusal", case.krate), |a| verdict(a, &name)["outcome"] == "refused");

            let v = verdict(&app, &name);
            assert_eq!(v["file"], name.as_str(), "{}: {what}: {v}", case.krate);
            let named = v["field"].as_str().unwrap_or_default();
            assert!(!named.is_empty() && named.contains(field), "{}: {what}: the verdict names the field `{field}`: {v}", case.krate);
            assert!(!v["reason"].as_str().unwrap().is_empty(), "{}: {what}: the verdict says why: {v}", case.krate);
            assert_eq!(settings(&app, case.krate).as_deref(), Some(case.good.as_str()), "{}: {what}: the last good settings stay", case.krate);
            assert_eq!(reloads(&app)["ok"], false, "{}: {what}", case.krate);
            assert_eq!(seq(&app), before + 1, "{}: {what}: one edit, one decision", case.krate);

            // Said once on the status line, naming the file and the field, and what stays.
            let raised = (app.world().resource::<EditorState>().status.raised() - raised_before) as usize;
            let said: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().rev().take(raised).map(|p| p.text.clone()).collect();
            assert_eq!(raised, 1, "{}: {what}: the refusal is said once: {said:?}", case.krate);
            if ron::from_str::<ron::Value>(bad).is_err() {
                // Not RON at all: the host's own sentence, which the gooey test pins by its words.
                assert!(said[0].contains(case.krate) && said[0].contains("not valid RON"), "{}: {what}: {said:?}", case.krate);
            } else {
                assert!(said[0].contains(&name) && said[0].contains(named) && said[0].contains(case.stays), "{}: {what}: {said:?}", case.krate);
            }

            // Nothing was edited since, so nothing is decided again however many frames pass.
            let refused_at = seq(&app);
            pump(&mut app, 1.2);
            assert_eq!(seq(&app), refused_at, "{}: {what}: no new decision without an edit", case.krate);

            // Mended back to the settings in force: applied, once.
            std::fs::write(&file, &case.good).unwrap();
            until(&mut app, &format!("{}: {what}: the mended file", case.krate), |a| verdict(a, &name)["outcome"] == "applied");
            assert_eq!(seq(&app), refused_at + 1, "{}: {what}: a repair is one decision", case.krate);
            assert_eq!(reloads(&app)["ok"], true, "{}: {what}", case.krate);
            pump(&mut app, 1.2);
            assert_eq!(seq(&app), refused_at + 1, "{}: {what}: the repair is not said again", case.krate);
        }

        // Mended to other text the type reads: applied, and it is the crate's settings now.
        std::fs::write(&file, &case.bad[1].1).unwrap();
        until(&mut app, "refused again", |a| verdict(a, &name)["outcome"] == "refused");
        std::fs::write(&file, &case.other_good).unwrap();
        until(&mut app, "another good text applied", |a| verdict(a, &name)["outcome"] == "applied");
        assert_eq!(settings(&app, case.krate).as_deref(), Some(case.other_good.as_str()), "{}", case.krate);

        // A refused file that is deleted is no longer refused.
        std::fs::write(&file, &case.bad[2].1).unwrap();
        until(&mut app, "refused once more", |a| verdict(a, &name)["outcome"] == "refused");
        std::fs::remove_file(&file).unwrap();
        until(&mut app, "the deleted file forgotten", |a| verdict(a, &name) == Value::Null);
        assert_eq!(reloads(&app)["ok"], true, "{}", case.krate);
    }
}

/// A file that is the wrong type when the project opens has no last good settings to keep: it is named from
/// the start, the crate's defaults apply (the text is not in [`FamilySettings`]), and what mends it is applied.
#[test]
fn a_file_of_the_wrong_type_when_the_project_opens_is_named_and_not_taken() {
    for case in cases() {
        let name = format!("family/{}.ron", case.krate);
        for (what, bad, field) in &case.bad {
            let (mut app, file) = project("family_typed_launch", &case, bad);
            let v = verdict(&app, &name);
            assert_eq!(v["outcome"], "refused", "{}: {what}: {v}", case.krate);
            let named = v["field"].as_str().unwrap_or_default();
            assert!(!named.is_empty() && named.contains(field), "{}: {what}: the verdict names the field `{field}`: {v}", case.krate);
            assert_eq!(settings(&app, case.krate), None, "{}: {what}: the text is not the crate's settings", case.krate);
            let decided = seq(&app);
            pump(&mut app, 1.2);
            assert_eq!(seq(&app), decided, "{}: {what}: said once however many frames pass", case.krate);

            std::thread::sleep(Duration::from_millis(50));
            std::fs::write(&file, &case.good).unwrap();
            until(&mut app, &format!("{}: {what}: mended", case.krate), |a| verdict(a, &name)["outcome"] == "applied");
            assert_eq!(settings(&app, case.krate).as_deref(), Some(case.good.as_str()), "{}: {what}", case.krate);
        }
    }
}

/// A type refusal, then an edit that is not RON, then the text back: each step is one decision, in either
/// order of return. The watch holds the text it took (valid RON) and the settings in force are the last
/// the type read, so the two must agree about which edits are news.
#[test]
fn a_refusal_then_an_unreadable_edit_then_the_same_text_back_is_one_decision_per_edit() {
    for case in cases() {
        let name = format!("family/{}.ron", case.krate);
        let (mut app, file) = project("family_typed_back", &case, &case.good);
        let (_, wrong, field) = case.bad[1].clone();
        let (_, cut, _) = case.bad[0].clone();
        let step = |app: &mut App, text: &str, outcome: &str, what: &str| {
            let before = seq(app);
            std::thread::sleep(Duration::from_millis(50));
            std::fs::write(&file, text).unwrap();
            until(app, what, |a| verdict(a, &name)["outcome"] == outcome && seq(a) > before);
            let decided = seq(app);
            pump(app, 1.2);
            assert_eq!(seq(app), decided, "{}: {what}: said once however many frames pass", case.krate);
            assert_eq!(decided, before + 1, "{}: {what}: one edit, one decision", case.krate);
        };

        step(&mut app, &wrong, "refused", "the wrong type");
        let typed = verdict(&app, &name);
        assert!(typed["field"].as_str().unwrap_or_default().contains(field), "{}: {typed}", case.krate);
        step(&mut app, &cut, "refused", "then not RON");
        assert_ne!(verdict(&app, &name)["reason"], typed["reason"], "{}: the verdict follows the file", case.krate);

        // Back to the text the type refused: the verdict is the type's again, with its field.
        step(&mut app, &wrong, "refused", "back to the wrong type");
        let again = verdict(&app, &name);
        assert_eq!((&again["field"], &again["reason"]), (&typed["field"], &typed["reason"]), "{}: {again}", case.krate);
        assert_eq!(settings(&app, case.krate).as_deref(), Some(case.good.as_str()), "{}: the last good stays", case.krate);

        // Not RON once more, then the settings in force: applied once, and the crate takes them.
        step(&mut app, &cut, "refused", "not RON again");
        step(&mut app, &case.good, "applied", "the settings in force again");
        assert_eq!(reloads(&app)["ok"], true, "{}", case.krate);
        assert_eq!(settings(&app, case.krate).as_deref(), Some(case.good.as_str()), "{}", case.krate);
    }
}
