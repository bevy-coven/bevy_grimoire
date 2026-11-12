//! **A bake is proved against the YAML it was made from** (GD-08, #370).
//!
//! The project is the sample game (`games/sample/`): a map, its kit, its entity file and its data, which is the project
//! the release path is exercised on. Every test bakes a **copy** of it into a scratch folder of its own, so no test
//! changes the repository's files.
//!
//! What is measured:
//!
//! - every baked file decodes to **the value its YAML reads to**, and the manifest names each file's kind, its format
//!   version and the SHA-256 of its YAML source;
//! - **a binary changed by hand fails the comparison**, naming the file, and so does one the manifest did not hash;
//! - **a YAML byte changed after the bake is refused**, naming the file, before anything is read from the bake;
//! - **a file that does not read fails the whole bake and nothing is written**.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::bake::{self, Baked, MANIFEST_FILE, Manifest};
use bevy_grimoire_core::{load, yaml};

/// **A scratch folder of this test's own**, named for it and this process.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bake-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    dir
}

/// The sample game's folder in this repository.
fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/sample")
}

/// **A copy of the sample game's project data**, which is every file a bake reads: its folders `assets/`, `game/` and
/// `scripts/`.
fn copy_of_the_sample(tag: &str) -> PathBuf {
    let root = scratch(tag).join("project");
    for dir in ["assets", "game", "scripts"] {
        copy_tree(&sample().join(dir), &root.join(dir));
    }
    root
}

fn copy_tree(from: &Path, into: &Path) {
    std::fs::create_dir_all(into).unwrap_or_else(|e| panic!("{into:?}: {e}"));
    for entry in std::fs::read_dir(from).unwrap_or_else(|e| panic!("{from:?}: {e}")).flatten() {
        let (source, target) = (entry.path(), into.join(entry.file_name()));
        if source.is_dir() {
            copy_tree(&source, &target);
        } else {
            std::fs::copy(&source, &target).unwrap_or_else(|e| panic!("{source:?}: {e}"));
        }
    }
}

fn manifest(dir: &Path) -> Manifest {
    let text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).unwrap_or_else(|e| panic!("{e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"))
}

/// **Every baked file decodes to the value its YAML reads to**, and the manifest says what each one is.
#[test]
fn every_baked_file_decodes_to_the_value_its_yaml_reads_to() {
    let root = copy_of_the_sample("decode");
    let out = root.parent().unwrap_or(&root).join("baked");
    let report = bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    assert!(report.files >= 4, "the sample has a map, a library, a kit and an entity file at least: {report:?}");

    let baked = Baked::open(&out, &root).unwrap_or_else(|p| panic!("{}", p.line()));
    let manifest = manifest(&out);
    assert_eq!(manifest.files.len(), report.files);
    for entry in &manifest.files {
        let source = std::fs::read(root.join(&entry.file)).unwrap_or_else(|e| panic!("{}: {e}", entry.file));
        let text = String::from_utf8(source.clone()).unwrap_or_else(|e| panic!("{e}"));
        let value = yaml::to_json(&text, &entry.file).unwrap_or_else(|p| panic!("{}", p.line()));
        assert_eq!(baked.value(&entry.file).unwrap_or_else(|p| panic!("{}", p.line())), value, "{} decodes to its YAML's value", entry.file);
        assert_eq!(entry.source_sha256, bake::sha256_hex(&source), "{}: the manifest holds the SHA-256 of the YAML source", entry.file);
        assert_eq!(entry.format_version, value.get("version").and_then(|v| v.as_u64()).map(|v| v as u32), "{}: and its format version", entry.file);
        assert!(!entry.kind.is_empty(), "{}: and its kind", entry.file);
    }
    for kind in ["map", "library", "kits", "entities"] {
        assert!(manifest.files.iter().any(|e| e.kind == kind), "the sample's `{kind}` is in the bake: {:?}", manifest.files.iter().map(|e| &e.file).collect::<Vec<_>>());
    }
    assert_eq!(bake::verify(&root, &out).unwrap_or_else(|p| panic!("{}", p.line())), report.files, "and the bake on disk verifies against the YAML");
}

/// **A binary corrupted by hand fails the comparison**, naming the file.
#[test]
fn a_binary_corrupted_by_hand_fails_the_comparison() {
    let root = copy_of_the_sample("corrupt");
    let out = root.parent().unwrap_or(&root).join("baked");
    bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    let entry = manifest(&out).files.into_iter().find(|e| e.kind == "map").unwrap_or_else(|| panic!("the sample has a map"));
    let binary = out.join(&entry.baked);
    let mut bytes = std::fs::read(&binary).unwrap_or_else(|e| panic!("{e}"));
    let at = bytes.len() / 2;
    bytes[at] ^= 0x01;
    std::fs::write(&binary, bytes).unwrap_or_else(|e| panic!("{e}"));

    let refused = bake::verify(&root, &out).expect_err("a binary changed by hand does not verify");
    assert!(refused.line().contains(&entry.file), "the file is named: {}", refused.line());
    let baked = Baked::open(&out, &root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert!(baked.value(&entry.file).expect_err("and a release does not read it").line().contains(&entry.file));
}

/// **After one YAML byte changes, the bake is refused**, naming the file.
#[test]
fn a_yaml_byte_changed_after_the_bake_is_refused_naming_the_file() {
    let root = copy_of_the_sample("stale");
    let out = root.parent().unwrap_or(&root).join("baked");
    bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    let entities = std::fs::read_dir(root.join(bake::ENTITIES_DIR))
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with(bake::ENTITIES_EXT))
        .unwrap_or_else(|| panic!("the sample has an entity file"));
    let mut text = std::fs::read_to_string(&entities).unwrap_or_else(|e| panic!("{e}"));
    text.push('#');
    std::fs::write(&entities, text).unwrap_or_else(|e| panic!("{e}"));

    let baked = Baked::open(&out, &root).unwrap_or_else(|p| panic!("{}", p.line()));
    let refused = baked.refuse_if_stale().expect_err("a YAML file that changed after the bake is refused");
    let name = entities.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
    assert!(refused.line().contains(&name), "the file is named: {}", refused.line());
    assert!(refused.line().contains("changed since the game was baked"), "{}", refused.line());
}

/// **A YAML file added after the bake is refused too**: a release does not quietly leave it out.
#[test]
fn a_yaml_file_added_after_the_bake_is_refused() {
    let root = copy_of_the_sample("added");
    let out = root.parent().unwrap_or(&root).join("baked");
    bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    std::fs::write(root.join(bake::ENTITIES_DIR).join(format!("later{}", bake::ENTITIES_EXT)), "entities: {}\n").unwrap_or_else(|e| panic!("{e}"));
    let baked = Baked::open(&out, &root).unwrap_or_else(|p| panic!("{}", p.line()));
    let refused = baked.refuse_if_stale().expect_err("a file the bake does not have is refused");
    assert!(refused.line().contains("later.entities.yaml"), "{}", refused.line());
}

/// **A file that does not read fails the bake, names the file, and writes nothing.**
#[test]
fn a_file_that_does_not_read_fails_the_bake_and_nothing_is_written() {
    let root = copy_of_the_sample("unreadable");
    let out = root.parent().unwrap_or(&root).join("baked");
    std::fs::write(root.join(bake::ENTITIES_DIR).join(format!("broken{}", bake::ENTITIES_EXT)), "entities:\n  a: {}\n  a: {}\n").unwrap_or_else(|e| panic!("{e}"));
    let refused = bake::bake(&root, &out).expect_err("a duplicate key is not baked");
    assert!(refused.line().contains("broken.entities.yaml"), "the file is named: {}", refused.line());
    assert!(!out.exists(), "and nothing is written: {out:?}");
}

/// **The two sources give one answer**: a type loaded from the YAML and from the bake is the same value, through the one
/// function.
///
/// One test function, because the source is the process's: a second test choosing the other source beside it would be a
/// race.
#[test]
fn the_one_load_function_answers_the_same_from_the_yaml_and_from_the_bake() {
    use bevy_grimoire_core::{kits::Kits, library::Library, map::Map};
    let root = copy_of_the_sample("load");
    let out = root.parent().unwrap_or(&root).join("baked");
    bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    let grimoire = root.join("assets/grimoire");
    let map_rel = bake::project_files(&root)
        .unwrap_or_else(|p| panic!("{}", p.line()))
        .into_iter()
        .find(|(_, kind)| kind == "map")
        .map(|(rel, _)| rel)
        .unwrap_or_else(|| panic!("the sample has a map"));
    let kit_rel = "assets/grimoire/kits.yaml";
    let _ = grimoire;

    load::use_yaml();
    let map_yaml: Map = load::load(&root, &map_rel).unwrap_or_else(|p| panic!("{}", p.line()));
    let kits_yaml: Kits = load::load(&root, kit_rel).unwrap_or_else(|p| panic!("{}", p.line()));
    let library_files: Vec<String> =
        bake::project_files(&root).unwrap_or_else(|p| panic!("{}", p.line())).into_iter().filter(|(_, k)| k == "library").map(|(r, _)| r).collect();
    let libraries_yaml: Vec<Library> = library_files.iter().map(|rel| load::load(&root, rel).unwrap_or_else(|p| panic!("{}", p.line()))).collect();
    assert!(!load::is_baked());

    load::use_baked(&out, &root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert!(load::is_baked());
    let map_baked: Map = load::load(&root, &map_rel).unwrap_or_else(|p| panic!("{}", p.line()));
    let kits_baked: Kits = load::load(&root, kit_rel).unwrap_or_else(|p| panic!("{}", p.line()));
    let libraries_baked: Vec<Library> = library_files.iter().map(|rel| load::load(&root, rel).unwrap_or_else(|p| panic!("{}", p.line()))).collect();
    load::use_yaml();

    let json = |v: &dyn erased::Json| v.json();
    assert_eq!(json(&map_yaml), json(&map_baked), "a map is one value whichever format held it");
    assert_eq!(json(&kits_yaml), json(&kits_baked));
    for (a, b) in libraries_yaml.iter().zip(&libraries_baked) {
        assert_eq!(json(a), json(b));
    }
}

/// A tiny shim so the comparison above reads as one function over three types.
mod erased {
    pub trait Json {
        fn json(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Json for T {
        fn json(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap_or_default()
        }
    }
}

/// **A tree that nests past the reader's depth is refused, not walked until the stack is gone** (review MUST-FIX 3).
///
/// postcard decodes a `Tree` with no bound of its own and a list frame is two bytes, so a crafted ~1 MiB binary nests half a
/// million deep: the decode and `Tree::into_value` would recurse until the stack ran out, which is an abort no `Result` and no
/// hash can stop — `baked_sha256` sits in the same untrusted manifest.
///
/// Two bounds are measured, because a bake has two doors: the **writer** refuses a value past the limit (so this build never makes
/// such a binary), and the **reader** refuses one past it, which is the door a crafted manifest reaches. The tree here is built
/// and written at `DEEP` — well past the limit and still far inside what a stack holds — so the reader is exercised without the
/// test itself depending on a half-million-deep fixture that would overflow while it was being built.
#[test]
fn a_tree_nested_past_the_limit_is_refused_by_both_the_writer_and_the_reader() {
    /// Past `MAX_TREE_DEPTH`, and shallow enough that writing it does not overflow anything itself.
    const DEEP: usize = 200;

    // **The value the writer is asked for**: `[[[…]]]`, built iteratively so the fixture does not recurse.
    let mut value = serde_json::Value::Null;
    for _ in 0..DEEP {
        value = serde_json::Value::Array(vec![value]);
    }
    let refused = bake::encode("map", "maps/m.map.yaml", &value).expect_err("the writer refuses a tree past the limit");
    assert!(refused.line().contains(&bake::MAX_TREE_DEPTH.to_string()), "the refusal names the limit: {}", refused.line());

    // **The reader**, with the bytes a crafted manifest would hold: the tree as the bake scheme writes one, at a depth the writer
    // would have refused had it been asked.
    let mut tree = bake::Tree::Null;
    for _ in 0..DEEP {
        tree = bake::Tree::List(vec![tree]);
    }
    let bytes = postcard::to_allocvec(&(bake::BAKE_FORMAT, "map", "maps/m.map.yaml", tree)).expect("the container writes");
    let refused = bake::decode("maps/m.map.yaml", &bytes).expect_err("the reader refuses it");
    let said = refused.line();
    assert!(said.contains("does not decode"), "the refusal says what happened: {said}");
    assert!(said.contains(&bake::MAX_TREE_DEPTH.to_string()), "and names the depth a tree may nest to: {said}");
}

/// **A manifest cannot point a read outside the bake folder** (review MUST-FIX 4).
///
/// `Baked::value` joined `entry.baked` and `Baked::open` joined `entry.file` verbatim, so a manifest holding
/// `../../../../etc/passwd` — or an absolute path — read any file on the machine, and a matching `baked_sha256` in the same
/// manifest let it decode. The write side already had `join_inside`; the read side did not use it.
#[test]
fn a_manifest_path_that_climbs_out_of_the_bake_is_refused() {
    let root = copy_of_the_sample("traversal");
    let out = scratch("traversal_bake");
    bake::bake(&root, &out).unwrap_or_else(|p| panic!("{}", p.line()));
    let manifest_path = out.join(MANIFEST_FILE);
    let original: Manifest = serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    let named = original.files.first().map(|e| e.file.clone()).unwrap_or_else(|| panic!("the sample bakes files"));
    let real_baked = original.files.first().map(|e| e.baked.clone()).unwrap_or_default();

    for (what, baked, file) in [
        ("a baked path that climbs out", "../../../../etc/passwd".to_owned(), named.clone()),
        ("an absolute baked path", "/etc/passwd".to_owned(), named.clone()),
        ("a source path that climbs out", real_baked.clone(), "../../../../etc/passwd".to_owned()),
        ("an absolute source path", real_baked.clone(), "/etc/passwd".to_owned()),
    ] {
        let mut one = original.clone();
        if let Some(entry) = one.files.first_mut() {
            entry.baked = baked;
            entry.file = file;
        }
        std::fs::write(&manifest_path, serde_json::to_string(&one).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));

        // **`open` may refuse the entry outright, or `value` may refuse the read**: both are inside the bake, and neither may
        // reach a file outside it. What is not allowed is a read that succeeds.
        if let Ok(opened) = Baked::open(&out, &root) {
            match opened.value(&named) {
                Ok(value) => panic!("{what}: the bake read a file outside itself: {value:?}"),
                Err(problem) => {
                    let said = problem.line();
                    assert!(said.contains("outside") || said.contains("not in the bake"), "{what}: {said}");
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
