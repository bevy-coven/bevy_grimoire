//! The game's verdict file: round trip, absence, corruption and a newer version.

use bevy_grimoire_core::live::{Verdict, VERDICT_VERSION};

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("grimoire_verdict_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_verdict_round_trips() {
    let d = dir("rt");
    let v = Verdict::now(false, "descriptor `x/y` does not resolve");
    v.write(&d).unwrap();
    assert_eq!(Verdict::read(&d).unwrap(), Some(v));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn no_live_directory_is_no_verdict() {
    let d = dir("none");
    assert_eq!(Verdict::read(&d).unwrap(), None);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_bad_or_newer_file_is_refused_by_path() {
    let d = dir("bad");
    std::fs::create_dir_all(d.join(".live")).unwrap();
    let path = Verdict::path(&d);
    std::fs::write(&path, "not ron").unwrap();
    let e = Verdict::read(&d).unwrap_err();
    assert!(e.contains("game.yaml"), "{e}");
    let newer = format!(
        "version: {}\nat_unix_ms: 1\nok: true\nmessage: \"\"\n",
        VERDICT_VERSION + 1
    );
    std::fs::write(&path, newer).unwrap();
    let e = Verdict::read(&d).unwrap_err();
    assert!(e.contains("game.yaml") && e.contains("newer"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
