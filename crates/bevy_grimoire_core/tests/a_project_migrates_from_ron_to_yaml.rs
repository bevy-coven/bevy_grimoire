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
    // **`NaN` is the one value RON and YAML cannot agree about**, and it is measured, not contrived: a
    // float that is not a number reads back as `NaN` and compares unequal to itself, so a migration
    // that wrote it out would be storing a value it cannot vouch for. This standards file holds one in
    // a real `f32` field (`min`), which `ron::from_str` accepts and the writer cannot round-trip.
    let root = scratch("refuse");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    let good = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/old_data/core-0.3.0/standards.ron"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire.join("kits.ron"), "(version: 3, bind: [], bash: [], authoring: None)\n")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        grimoire.join("standards.ron"),
        good.replace("min: Some(0.9)", "min: Some(NaN)"),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let refusal = migrate::migrate(&root).expect_err("a value that would change stops the migration");
    let line = refusal.line();
    assert!(line.contains("standards.ron"), "the refusal names the file that would change: {line}");
    assert!(
        !grimoire.join("standards.yaml").exists(),
        "the file that would change is not written: {}",
        grimoire.join("standards.yaml").display()
    );
    assert!(
        !grimoire.join("kits.yaml").exists(),
        "and the file that converted cleanly is not written either, so no half-migrated project is left"
    );
    assert!(grimoire.join("standards.ron").is_file(), "the RON originals are left where they were");
    assert!(grimoire.join("kits.ron").is_file());
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
fn a_comment_with_no_field_to_sit_above_is_listed_and_never_dropped() {
    let root = scratch("unplaced");
    let grimoire = root.join(migrate::GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("{e}"));
    // A file header: nothing above it, so nothing in the YAML to hang it from.
    std::fs::write(
        grimoire.join("standards.ron"),
        "// this project follows the site kit's module\n(\n    version: 1,\n    rules: [],\n)\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let report = migrate::migrate(&root).unwrap_or_else(|p| panic!("{}", p.line()));

    assert_eq!(report.unplaced.len(), 1, "{:?}", report.lines());
    assert!(report.unplaced[0].comment.contains("follows the site kit's module"));
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
