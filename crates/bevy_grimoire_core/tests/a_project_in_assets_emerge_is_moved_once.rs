//! A game's project from before the rename keeps its data in `assets/emerge`. Grimoire reads
//! `assets/grimoire`, so `migrate_legacy_data_dir` moves the old folder once, and refuses to choose
//! when both folders exist.

use std::path::PathBuf;

use bevy_grimoire_core::project_data::migrate_legacy_data_dir;

fn root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-migrate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets")).unwrap_or_else(|e| panic!("{e}"));
    dir
}

#[test]
fn only_the_old_folder_is_moved_to_the_new_one() {
    let dir = root("old");
    std::fs::create_dir_all(dir.join("assets/emerge")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir.join("assets/emerge/kits.yaml"), "()").unwrap_or_else(|e| panic!("{e}"));

    let said = migrate_legacy_data_dir(&dir).unwrap_or_else(|e| panic!("{e}"));

    assert!(said.is_some(), "a move is announced");
    assert!(dir.join("assets/grimoire/kits.yaml").is_file(), "the project's files moved with it");
    assert!(!dir.join("assets/emerge").exists(), "the old folder is gone");
    assert_eq!(migrate_legacy_data_dir(&dir), Ok(None), "a second open moves nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_folders_are_refused_and_left_alone() {
    let dir = root("both");
    for folder in ["assets/emerge", "assets/grimoire"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.join(folder).join("kits.yaml"), folder).unwrap_or_else(|e| panic!("{e}"));
    }

    assert!(migrate_legacy_data_dir(&dir).is_err());

    for folder in ["assets/emerge", "assets/grimoire"] {
        let kept = std::fs::read_to_string(dir.join(folder).join("kits.yaml")).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(kept, folder, "{folder} is untouched");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_already_in_assets_grimoire_is_left_alone() {
    let dir = root("new");
    std::fs::create_dir_all(dir.join("assets/grimoire")).unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(migrate_legacy_data_dir(&dir), Ok(None));
    assert!(dir.join("assets/grimoire").is_dir());
    let _ = std::fs::remove_dir_all(&dir);
}
