//! **A project's files move from RON to YAML, losslessly and loudly** (GD-03, #365).
//!
//! Every project file the registry names is read as RON, written as YAML, and read back **as the same
//! type** to compare against the value the RON held. A value that would change stops the whole
//! migration, names the file, and writes nothing: a half-migrated project is the one outcome a person
//! cannot repair by hand. The RON originals are moved into `<root>/.dark_factory/migrated-ron/`.
//!
//! **The fixture projects are the shipped project and the release files `tests/old_data.rs` freezes** —
//! no synthetic text. A conversion proved on `(version: 1)` alone proves only that an integer
//! round-trips; these hold an enum in a tagged position (`dim: Width`), an `Option` inside a sequence
//! (`min: Some(0.9)`) and a nested tuple, which is where a format move actually loses a field.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::migrate::{self, ARCHIVE_DIR, COMMAND, RON_EXT, YAML_EXT};
use bevy_grimoire_core::{standards::Standards, yaml};

/// **A scratch project of this test's own**, named for the test and this process.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-yamlcut-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    dir
}

/// The workspace's shipped project directory, which every fixture copy starts from.
fn shipped_grimoire() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire")
}

/// **A copy of the whole shipped project**, which is the fixture the acceptance names: every kind the
/// shipped project carries, at the paths it carries them.
fn copy_of_the_shipped_project(tag: &str) -> PathBuf {
    let root = scratch(tag);
    let into = root.join(migrate::GRIMOIRE_DIR);
    copy_tree(&shipped_grimoire(), &into);
    root
}

/// Every file under `from`, copied to `into`, keeping the tree.
fn copy_tree(from: &Path, into: &Path) {
    std::fs::create_dir_all(into).unwrap_or_else(|e| panic!("{into:?}: {e}"));
    for entry in std::fs::read_dir(from).unwrap_or_else(|e| panic!("{from:?}: {e}")) {
        let entry = entry.unwrap_or_else(|e| panic!("{from:?}: {e}"));
        let target = into.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap_or_else(|e| panic!("{target:?}: {e}"));
        }
    }
}

/// **Every project file under `root`, relative with `/`**, sorted.
fn project_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect(root, root, &mut out);
    out.sort();
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}")) {
        let entry = entry.unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn a_copy_of_the_shipped_project_migrates_and_every_yaml_reads_to_its_ron_value() {
    let root = copy_of_the_shipped_project("shipped");
    // **What the registry names, not every `.ron` under `assets/`.** The shipped project also carries
    // `library_from_manifests.ron` and `library_kenney_from_manifests.ron`, which are the inputs a
    // generator reads and no kind's pattern matches: they are not project files and are left alone.
    let planned = migrate::plan(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    let all = project_files(&root);
    let ron_before: Vec<&String> = all
        .iter()
        .filter(|f| f.ends_with(".ron") && planned.iter().any(|(_, rel, _)| rel == *f))
        .collect();
    assert!(ron_before.len() >= 10, "the shipped project is the fixture: {} RON project files", ron_before.len());

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));

    assert_eq!(report.converted.len(), ron_before.len(), "every RON project file converted: {:?}", report.lines());
    for file in &report.converted {
        let path = root.join(&file.yaml);
        assert!(path.is_file(), "{} was written", file.yaml);
        assert!(!root.join(&file.rom).exists(), "{} was moved away, not left beside its YAML", file.rom);
        assert!(
            file.yaml.ends_with(&format!(".{YAML_EXT}")),
            "{} carries the new extension",
            file.yaml
        );
    }
    // **A generator's input is not a project file**, and the migration says so by leaving it: the
    // registry decides what a project is, not the extension.
    assert!(
        root.join(migrate::GRIMOIRE_DIR).join("library_from_manifests.ron").is_file(),
        "a file no kind names is left where it is"
    );
    let after = project_files(&root);
    let mut live: Vec<&String> = after.iter().filter(|f| !f.starts_with(ARCHIVE_DIR)).collect();
    live.retain(|f| f.ends_with(&format!(".{RON_EXT}")));
    // **Only the generator's inputs are left**, and they are not project files.
    assert_eq!(
        live,
        vec![
            &format!("{}/library_from_manifests.ron", migrate::GRIMOIRE_DIR),
            &format!("{}/library_kenney_from_manifests.ron", migrate::GRIMOIRE_DIR),
        ],
        "no project file is RON any more, only the generator's inputs: {after:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_kind_holding_an_enum_and_an_option_reads_back_to_the_same_value() {
    // The frozen `standards.ron` from 0.3.0: `dim: Width` is an enum variant, `min: Some(0.9)` an
    // Option inside a sequence item, and `multiple_of` a nested float. This is the file a value-losing
    // migration would break, written to a project of its own.
    let root = scratch("standards");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/old_data/core-0.3.0/standards.ron"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("standards.ron"), &text).unwrap_or_else(|e| panic!("{e}"));
    let was: Standards = ron::from_str(&text).unwrap_or_else(|e| panic!("the fixture is RON: {e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), 1, "{:?}", report.lines());

    let written = std::fs::read_to_string(grimoire.join("standards.yaml")).unwrap_or_else(|e| panic!("{e}"));
    let now: Standards = yaml::from_str(&written, "standards.yaml").unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(now, was, "the YAML holds the value the RON held:\n{written}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_old_files_are_moved_into_the_archive_and_not_deleted() {
    let root = scratch("archive");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/old_data/core-0.3.0/standards.ron"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("standards.ron"), &text).unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));

    let archived = root.join(ARCHIVE_DIR).join(migrate::GRIMOIRE_DIR).join("standards.ron");
    assert!(archived.is_file(), "the RON original is in the archive: {}", archived.display());
    assert_eq!(
        std::fs::read_to_string(&archived).unwrap_or_else(|e| panic!("{e}")),
        text,
        "the archived copy is the file's own text, not a rewrite"
    );
    assert_eq!(report.archived_at, ARCHIVE_DIR);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_migration_that_would_change_a_value_stops_names_the_file_and_writes_nothing() {
    // **A value the one writer cannot carry faithfully stops the migration.** Measured: every `f32` a
    // standards rule can hold — `NaN`, both subnormals, `f32::MAX`, `-0.0` — writes out and reads back as
    // the same text, so none of them is a counterexample. The check is therefore proved on the mechanism
    // itself rather than on a number: a file whose kind this crate cannot read is refused by name, and
    // nothing at all is written — neither that file nor the good one beside it.
    let root = scratch("refuse");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    // A kind this crate declares a type for, which converts.
    std::fs::write(grimoire.join("kits.ron"), "(version: 3, bind: [], bash: [], authoring: None)\n")
        .unwrap_or_else(|e| panic!("{e}"));
    // A kind it does not: a forest is read by the editor, and only the editor can compare its value.
    // A forest of a KIT: `<kit>/forest/<name>.ron`, which is where the kind's pattern points.
    std::fs::create_dir_all(grimoire.join("site/forest")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("site/forest/yard.ron"), "(trees: [])\n").unwrap_or_else(|e| panic!("{e}"));

    let refusal = match migrate::migrate(&root) {
        Ok(_) => panic!("a file the command cannot verify stops the migration"),
        Err(p) => p,
    };
    let line = refusal.line();
    assert!(line.contains("site/forest/yard.ron"), "the refusal names the file: {line}");
    assert!(
        !grimoire.join("kits.yaml").exists(),
        "the file that converted cleanly is not written either, so no half-migrated project is left"
    );
    assert!(!grimoire.join("site/forest/yard.yaml").exists(), "and the refused file is not written");
    assert!(grimoire.join("kits.ron").is_file(), "the RON originals are left where they were");
    assert!(grimoire.join("site/forest/yard.ron").is_file());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_comment_is_carried_above_the_field_it_was_written_about() {
    let root = scratch("comments");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("standards.ron"),
        "(\n    version: 1,\n    // a door is 0.9 to 1.2 m, and the module is a decimetre\n    rules: [],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    let written = std::fs::read_to_string(grimoire.join("standards.yaml")).unwrap_or_else(|e| panic!("{e}"));

    assert!(
        written.contains("# a door is 0.9 to 1.2 m, and the module is a decimetre"),
        "the comment survives into the YAML:\n{written}"
    );
    assert_eq!(report.converted[0].comments_carried, 1, "{:?}", report.lines());
    // And it reads: a YAML comment is not a field.
    let _: Standards = yaml::from_str(&written, "standards.yaml").unwrap_or_else(|p| panic!("{}", p.line()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_header_comment_goes_at_the_top_of_the_file() {
    let root = scratch("header");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    // A header describes the whole file, so it belongs above the first line — and it must survive, not
    // be reported as unplaceable. This is the shape every shipped project file carries.
    std::fs::write(
        grimoire.join("standards.ron"),
        "// this project follows the site kit's module\n(\n    version: 1,\n    rules: [],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    let written = std::fs::read_to_string(grimoire.join("standards.yaml")).unwrap_or_else(|e| panic!("{e}"));

    assert!(report.unplaced.is_empty(), "a header has a place, so nothing is unplaced: {:?}", report.lines());
    assert_eq!(report.converted[0].comments_carried, 1, "{:?}", report.lines());
    assert!(
        written.starts_with("# this project follows the site kit's module\n"),
        "the header is the file's first line:\n{written}"
    );
    let _: Standards = yaml::from_str(&written, "standards.yaml").unwrap_or_else(|p| panic!("{}", p.line()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_comment_after_the_last_field_is_listed_and_never_dropped() {
    let root = scratch("footer");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    // A footer stands after the last field: YAML has no node after the last one to hang it from, so it
    // is listed rather than moved somewhere it might not mean what it said.
    std::fs::write(
        grimoire.join("standards.ron"),
        "(\n    version: 1,\n    rules: [],\n)\n// measured against the shipped kits, 2026-08\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));

    assert_eq!(report.unplaced.len(), 1, "{:?}", report.lines());
    assert!(report.unplaced[0].comment.contains("measured against the shipped kits"));
    assert!(report.unplaced[0].file.ends_with("standards.ron"));
    let said = report.lines().join("\n");
    assert!(said.contains("comment not placed"), "the report says it: {said}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_project_with_a_ron_file_left_over_is_refused_naming_the_file_and_the_command() {
    let root = scratch("leftover");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("kits.ron"), "(version: 3, bind: [], bash: [], authoring: None)\n")
        .unwrap_or_else(|e| panic!("{e}"));

    let problem = migrate::leftover(&root)
        .unwrap_or_else(|p| panic!("{}", p.line()))
        .expect("a project holding a RON file is refused");
    let line = problem.line();
    assert!(line.contains("kits.ron"), "the refusal names the file: {line}");
    assert!(line.contains(COMMAND), "the refusal names the command: {line}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_project_of_yaml_files_is_not_refused() {
    let root = scratch("clean");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("kits.yaml"), "version: 3\nbind: []\nbash: []\nauthoring: null\n")
        .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(migrate::leftover(&root).unwrap_or_else(|p| panic!("{}", p.line())), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_project_file_is_named_dot_yaml_where_it_was_named_dot_ron() {
    let cases = [
        ("kits.ron", "kits.yaml"),
        ("maps/town.map.ron", "maps/town.map.yaml"),
        ("editor.ron", "editor.yaml"),
        ("destructible/site__cart.bake.ron", "destructible/site__cart.bake.yaml"),
    ];
    for (rom, yaml_name) in cases {
        assert_eq!(
            migrate::yaml_path(Path::new(rom)),
            Some(PathBuf::from(yaml_name)),
            "{rom} becomes {yaml_name}"
        );
    }
    assert_eq!(migrate::yaml_path(Path::new("oplog.jsonl")), None, "a JSON line log is not a project file");
    assert_eq!(migrate::yaml_path(Path::new("kits.ron.tmp")), None, "a temp file is not one");
}

/// **Every file a release wrote, frozen under `tests/old_data/`**, as `(kind, path in the project, the
/// file's text)`: the same inventory `tests/old_data.rs` reads, restated here rather than reached for,
/// because that test's list is a private `const` in another binary.
///
/// This is the strongest fixture the migration can be proved on: 26 real files from two releases and
/// one editor commit, holding an enum in a tagged position, `Option`s inside sequences, nested tuples
/// and a baked fracture's chunk tables — every shape a format move loses a field in.
fn frozen_files() -> Vec<(&'static str, &'static str, &'static str)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/old_data");
    let read = |rel: &str| {
        let path = dir.join(rel);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    };
    // Leaked so the tuple can hold `&'static str` beside `include_str!`'s lifetime without a `Vec<String>`
    // alloc per file in the test: this runs once per test process.
    let leak = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    vec![
        ("kits", "kits.ron", leak(read("core-0.1.0/kits.ron"))),
        ("vocab", "vocab.ron", leak(read("core-0.1.0/vocab.ron"))),
        ("library", "scp/library.ron", leak(read("core-0.1.0/library.ron"))),
        ("library", "site/library.ron", leak(read("core-0.1.0/library_site.ron"))),
        ("policy", "site/project.ron", leak(read("core-0.1.0/policy.ron"))),
        ("policy", "furniture/project.ron", leak(read("core-0.1.0/policy_furniture.ron"))),
        ("compositions", "compositions.ron", leak(read("core-0.1.0/compositions.ron"))),
        ("rigs", "rigs.ron", leak(read("core-0.1.0/rigs.ron"))),
        ("rules", "rules.ron", leak(read("core-0.3.0/rules.ron"))),
        ("standards", "standards.ron", leak(read("core-0.3.0/standards.ron"))),
        ("repetition", "repetition.ron", leak(read("core-0.3.0/repetition.ron"))),
        ("map", "maps/yard.map.ron", leak(read("core-0.3.0/map.ron"))),
        ("notes", "maps/yard.notes.ron", leak(read("core-0.3.0/notes.ron"))),
        ("plan", "maps/yard.plan.ron", leak(read("core-0.3.0/plan.ron"))),
        ("references", "maps/yard.references.ron", leak(read("core-0.3.0/references.ron"))),
        ("relations", "maps/yard.relations.ron", leak(read("core-0.3.0/relations.ron"))),
        ("terrain", "site/terrain/yard.ron", leak(read("core-0.3.0/terrain.ron"))),
        ("water_plan", "site/water/yard.ron", leak(read("core-0.3.0/water_plan.ron"))),
        ("water_field", "site/water/yard.field.ron", leak(read("core-0.3.0/water_field.ron"))),
        ("volume", "site/volume/yard.ron", leak(read("core-0.3.0/volume.ron"))),
        ("data_registry", "site/data.ron", leak(read("core-0.3.0/data_registry.ron"))),
        ("family_list", "family.ron", leak(read("core-0.3.0/family_list.ron"))),
        ("agent", "agent.ron", leak(read("core-0.3.0/agent.ron"))),
        ("play", "play.ron", leak(read("core-0.3.0/play.ron"))),
        ("destructible_marks", "destructible.ron", leak(read("editor-3f4084c/destructible.ron"))),
        ("destructible_bake", "destructible/site__cart.bake.ron", leak(read("editor-3f4084c/site__cart.bake.ron"))),
    ]
}

/// **A literal, not `frozen_files().len()`**: a number that is its own measure can never fail, and this
/// floor stops a fixture being dropped to make a kind pass.
const FROZEN_MIN: usize = 26;

#[test]
fn every_file_a_release_wrote_converts_losslessly() {
    let frozen = frozen_files();
    assert_eq!(frozen.len(), FROZEN_MIN, "the inventory is every frozen file: {}", frozen.len());
    let mut broken = Vec::new();
    for (kind, file, text) in frozen {
        // Each frozen file is laid into a project of its own at the path its kind is written at, so the
        // migration walks it exactly as it walks a real one — including the kinds under a kit directory.
        let root = scratch(&format!("frozen_{}", file.replace(['/', '.'], "_")));
        let at = if file.contains('/') && !file.starts_with("maps/") {
            // A kit's own file: `<kit>/<name>.ron` under `assets/grimoire`.
            root.join(migrate::GRIMOIRE_DIR).join(file)
        } else if file.starts_with("maps/") {
            root.join(migrate::GRIMOIRE_DIR).join(file)
        } else if matches!(kind, "family_list" | "agent" | "play" | "destructible_marks" | "destructible_bake") {
            // A root file: beside `assets/`, not inside it.
            root.join(file)
        } else {
            root.join(migrate::GRIMOIRE_DIR).join(file)
        };
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{parent:?}: {e}"));
        }
        std::fs::write(&at, text).unwrap_or_else(|e| panic!("{at:?}: {e}"));
        match migrate::migrate(&root) {
            Ok(_) => {}
            Err(p) => broken.push(format!("{kind} ({file}): {}", p.line())),
        }
        let _ = std::fs::remove_dir_all(&root);
    }
    assert!(broken.is_empty(), "these files would change value if written as YAML:\n{}", broken.join("\n"));
}

#[test]
fn a_comment_above_a_named_list_item_goes_above_that_item() {
    // A vocabulary's tokens are a list, and its comments sit above individual tokens (`// marking is
    // what the height heuristic stood in for`, over `( name: "marking", … )`). The token names itself,
    // so the comment is carried above that token's row rather than listed as unplaceable — which is the
    // shape `assets/grimoire/vocab.ron` is written in.
    let root = scratch("named_item");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("standards.ron"),
        "(\n    version: 1,\n    rules: [\n        (\n            name: \"door_width\",\n            \
         kind: Some(\"door\"),\n            dim: Width,\n            min: Some(0.9),\n        ),\n        \
         // the wall module is a decimetre, and `min` is the only dimension this rule bounds\n        (\n            \
         name: \"wall_module\",\n            kind: None,\n            dim: Depth,\n            multiple_of: Some(0.1),\n        ),\n    ],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    let written = std::fs::read_to_string(grimoire.join("standards.yaml")).unwrap_or_else(|e| panic!("{e}"));

    assert!(report.unplaced.is_empty(), "the token names itself, so its comment has a place: {:?}", report.lines());
    let at_item = written
        .lines()
        .position(|l| l.trim_start().starts_with("- name: wall_module"));
    let at_comment = written.lines().position(|l| l.contains("the wall module is a decimetre"));
    let (Some(item), Some(comment)) = (at_item, at_comment) else {
        panic!("both the item and its comment are in the file:\n{written}");
    };
    assert!(comment < item, "the comment stands above the item it describes:\n{written}");
    let _: Standards = yaml::from_str(&written, "standards.yaml").unwrap_or_else(|p| panic!("{}", p.line()));
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------------------------
// A comment must never land inside a value.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_comment_is_never_inserted_into_a_multi_line_string() {
    // **The YAML writer puts a multi-line string in a `|-` block**, so its body can contain a line that
    // reads exactly like a field (`patches: []`). A comment written above a *real* `patches:` field that
    // matched the string's line first would land inside the note — and the note would silently change.
    // This is the case `carry_comments` had to be moved inside [`convert`] for: the text that lands on
    // disk, comments and all, is what gets read back and compared.
    let root = scratch("block_scalar");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    // `project.ron` is a KIT's policy: `<kit>/project.ron`, not a root file.
    std::fs::create_dir_all(grimoire.join("site")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("site/project.ron"),
        "(\n    version: 2,\n    note: Some(\"the wall\\npatches: []\\nand more\"),\n    \
         // the patches below are the ones this kit states, and an empty list is a statement\n    patches: [],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), 1, "{:?}", report.lines());

    let written = std::fs::read_to_string(grimoire.join("site/project.yaml")).unwrap_or_else(|e| panic!("{e}"));
    let was: bevy_grimoire_core::policy::Policy = ron::from_str(
        "(\n    version: 2,\n    note: Some(\"the wall\\npatches: []\\nand more\"),\n    patches: [],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let now: bevy_grimoire_core::policy::Policy = yaml::from_str(&written, "project.yaml")
        .unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(now, was, "the note is the same string it was, comment or no comment:\n{written}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_written_file_is_read_back_as_its_kind_and_matches_its_ron_value() {
    // **Every file the migration writes is parsed again, as the kind it is**, and compared with the value
    // its RON held. Checking that the text starts without a `(` proves nothing: a comment inside a block
    // scalar, a lost field or a re-spelled number all pass that and fail this.
    let root = copy_of_the_shipped_project("read_back");
    let planned = migrate::plan(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    let before: Vec<String> = planned
        .iter()
        .map(|(path, _, _)| std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{e}")))
        .collect();
    let kinds: Vec<&'static str> = planned.iter().map(|(_, _, k)| k.id).collect();

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), before.len(), "{:?}", report.lines());

    for (((rom, _, _), ron_text), kind) in planned.iter().zip(&before).zip(&kinds) {
        let yaml_path = migrate::yaml_path(rom).unwrap_or_else(|| panic!("{rom:?} has a yaml name"));
        let written = std::fs::read_to_string(&yaml_path).unwrap_or_else(|e| panic!("{yaml_path:?}: {e}"));
        let same = same_value(kind, ron_text, &written);
        assert!(same, "{kind} at {yaml_path:?} reads back to a different value than its RON held:\n{written}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// **Whether `written` reads as the same value `ron_text` did, as the type `kind` is.** One arm per kind
/// this crate owns; a kind it does not own cannot be compared and is a test bug, not a pass.
fn same_value(kind: &str, ron_text: &str, written: &str) -> bool {
    /// Read both, compare: `None` where either side refuses.
    fn both<T>(kind: &str, ron_text: &str, written: &str) -> bool
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq,
    {
        let was: T = match ron::from_str(ron_text) {
            Ok(v) => v,
            Err(e) => panic!("{kind}: the RON fixture does not read: {e}"),
        };
        match yaml::from_str::<T>(written, kind) {
            Ok(now) => now == was,
            Err(p) => panic!("{kind}: the YAML does not read: {}", p.line()),
        }
    }
    use bevy_grimoire_core as c;
    match kind {
        "kits" => both::<c::kits::Kits>(kind, ron_text, written),
        "vocab" => both::<c::vocab::Vocabularies>(kind, ron_text, written),
        "library" => both::<c::library::Library>(kind, ron_text, written),
        "policy" => both::<c::policy::Policy>(kind, ron_text, written),
        "rules" => both::<c::rules::Rules>(kind, ron_text, written),
        "compositions" => both::<c::composition::Compositions>(kind, ron_text, written),
        "standards" => both::<c::standards::Standards>(kind, ron_text, written),
        "repetition" => both::<c::repetition::RepetitionRules>(kind, ron_text, written),
        "rigs" => both::<c::rigs::Rigs>(kind, ron_text, written),
        "map" => both::<c::map::Map>(kind, ron_text, written),
        "data_registry" => both::<c::data_files::DataFiles>(kind, ron_text, written),
        other => panic!("`{other}` has no comparator in this test, so it proves nothing"),
    }
}

// ---------------------------------------------------------------------------------------------
// A comment is matched to its field exactly, and a value that differs refuses rather than panics.
// ---------------------------------------------------------------------------------------------

#[test]
fn an_item_is_matched_by_its_whole_name_not_a_prefix() {
    // `- name: wall` must not be taken for `- name: wall_module`: a comment above `wall` would otherwise
    // be inserted above the wrong row, and the file would still read.
    let root = scratch("prefix_name");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("standards.ron"),
        "(\n    version: 1,\n    rules: [\n        ( name: \"wall_module\", dim: Depth, multiple_of: Some(0.1) ),\n        \
         // this one is a door's width, and nothing else\n        ( name: \"wall\", dim: Width, min: Some(0.9) ),\n    ],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    let written = std::fs::read_to_string(grimoire.join("standards.yaml")).unwrap_or_else(|e| panic!("{e}"));

    assert!(report.unplaced.is_empty(), "{:?}", report.lines());
    let at_comment = written.lines().position(|l| l.contains("a door's width"));
    // **Exact**, or this test repeats the very bug it is about: `- name: wall` is a prefix of
    // `- name: wall_module`.
    let at_wall = written
        .lines()
        .position(|l| matches!(l.trim_start(), "- name: wall"));
    let at_module = written.lines().position(|l| l.trim_start().starts_with("- name: wall_module"));
    let (Some(comment), Some(wall), Some(module)) = (at_comment, at_wall, at_module) else {
        panic!("all three are in the file:\n{written}");
    };
    // The RON held `wall_module` first, so the shorter-named row is the second one.
    assert!(module < wall, "the file keeps the RON's order:\n{written}");
    assert!(comment > module && comment < wall, "the comment is above `wall` and not above `wall_module`:\n{written}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_written_text_that_differs_is_reported_by_line_and_never_panics() {
    // **`first_difference` compares the writer's own output line by line**, so it never slices a string at
    // a byte offset — which is what panicked on `é`/`è` (they share their first byte). This file holds
    // multi-byte characters and a float that is not a number, so both the character boundary and the
    // NaN-safe comparison are exercised by one file that must simply migrate cleanly.
    let root = scratch("multibyte");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("standards.ron"),
        "(\n    version: 1,\n    rules: [\n        (\n            name: \"café façade — porte\",\n            \
         kind: None,\n            dim: Width,\n            min: Some(NaN),\n        ),\n    ],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), 1, "{:?}", report.lines());
    let written = std::fs::read_to_string(grimoire.join("standards.yaml")).unwrap_or_else(|e| panic!("{e}"));
    assert!(written.contains("café façade — porte"), "the multibyte text survives:\n{written}");
    assert!(written.contains(".nan"), "and the NaN is written as the number it is:\n{written}");
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------------------------
// The walk stays inside the project, and a file already there is refused rather than replaced.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_symlinked_folder_is_never_walked_out_of_the_project() {
    // A symlink to a folder outside the root would have its RON file converted and written **outside the
    // project**, and its original moved out of that folder into the archive. The link is named so that a
    // file inside it really is a kind (`<kit>/library.ron`), which is what makes the escape reachable.
    let outside = scratch("symlink_outside");
    std::fs::create_dir_all(outside.join("elsewhere")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        outside.join("elsewhere/library.ron"),
        "(\n    version: 1,\n    note: None,\n    descriptors: [],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let root = scratch("symlink_root");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    // `assets/grimoire/site` is a kit directory, and it points outside the project.
    std::os::unix::fs::symlink(outside.join("elsewhere"), grimoire.join("site"))
        .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));

    assert!(report.converted.is_empty(), "nothing outside the project is converted: {:?}", report.lines());
    assert!(
        outside.join("elsewhere/library.ron").is_file(),
        "the file outside the project is untouched"
    );
    assert!(!outside.join("elsewhere/library.yaml").exists(), "and no YAML is written out there");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn a_yaml_file_already_beside_the_ron_refuses_the_migration() {
    // **Nothing is replaced.** A hand-written `kits.yaml` beside its RON is a file a person made; the
    // migration refuses and writes nothing rather than overwriting it.
    let root = scratch("no_replace");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("kits.ron"), "(version: 3, bind: [], bash: [], authoring: None)\n")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("kits.yaml"), "# a person's own\nversion: 3\n")
        .unwrap_or_else(|e| panic!("{e}"));

    let refusal = match migrate::migrate(&root) {
        Ok(_) => panic!("a file already at the target path stops the migration"),
        Err(p) => p,
    };
    assert!(refusal.line().contains("kits.yaml"), "the refusal names it: {}", refusal.line());
    assert_eq!(
        std::fs::read_to_string(grimoire.join("kits.yaml")).unwrap_or_else(|e| panic!("{e}")),
        "# a person's own\nversion: 3\n",
        "the person's file is not replaced"
    );
    assert!(grimoire.join("kits.ron").is_file(), "and the RON is left where it was");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn every_comment_in_the_shipped_rigs_file_finds_its_field() {
    // **The shipped `rigs.ron` is the fixture this needs**, and it is the reason the cursor is not the
    // whole rule. Each rig holds a `slots:` list, so a comment above that list moves the walk *into* the
    // rig's body; the next rig's key then opens **before** where that search reached. A single monotonic
    // cursor reports the next rig's comment as having no field — measured on this file: two comments
    // ("SCP-1048-A, the ear bear" and its sibling) were dropped from the report's carried count and
    // listed as unplaceable. A key that occurs once is placed at its one occurrence; only a repeated key
    // (`slots:`, `mesh:`) walks in field order.
    let root = scratch("shipped_rigs");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    let text = std::fs::read_to_string(shipped_grimoire().join("rigs.ron"))
        .unwrap_or_else(|e| panic!("the shipped rigs.ron: {e}"));
    std::fs::write(grimoire.join("rigs.ron"), &text).unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));

    assert_eq!(
        report.unplaced.len(),
        0,
        "every comment in the shipped file has a field to sit above: {:?}",
        report.lines()
    );
    let written = std::fs::read_to_string(grimoire.join("rigs.yaml")).unwrap_or_else(|e| panic!("{e}"));
    // And the two that a cursor alone loses are above the rigs they describe.
    for (comment, rig) in [
        ("SCP-1048-A, the ear bear", "scp1048_ear:"),
        ("SCP-1048-B, the infant-arm bear", "scp1048_infant:"),
    ] {
        let at_comment = written.lines().position(|l| l.contains(comment));
        let at_rig = written.lines().position(|l| l.trim_start() == rig);
        let (Some(comment_at), Some(rig_at)) = (at_comment, at_rig) else {
            panic!("`{comment}` and `{rig}` are both in the file:\n{written}");
        };
        assert_eq!(comment_at + 1, rig_at, "`{comment}` stands directly above `{rig}`");
    }
    let _ = std::fs::remove_dir_all(&root);
}
