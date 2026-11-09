//! **Read a project's data at runtime**, the way a game does: every file the editor writes, in one
//! call, validated against each other.
//!
//! The editor (`bevy_emerge_mapper`) opens a project through its own `Project::open`, which carries
//! editor state — undo, thumbnails, the authoring kit. A game wants none of that, only the answer to
//! *"what is in `assets/emerge`, and is it consistent?"*. This is that answer, engine-free, so a
//! game, a test and a headless tool all read the same files in the same order and refuse the same
//! mistakes.
//!
//! # Files and where they live
//!
//! Relative to the project directory (`assets/emerge`, the one holding `kits.ron`):
//!
//! | file | required | read by |
//! |---|---|---|
//! | `kits.ron` | yes | [`Kits::parse`], then [`kits::bound_library`] reads each kit's own files |
//! | `vocab.ron` | yes | [`Vocabularies::parse`] |
//! | `rules.ron` | no — absent means no rules | [`Rules::parse`] |
//! | `compositions.ron` | no — absent means none | [`kits::bound_library`] |
//! | `maps/<name>.map.ron` | only the maps asked for | [`Map::parse`] |
//!
//! # What is refused, and when
//!
//! At load, not at first use: an unknown vocabulary token ([`Library::resolve`]), a hole whose
//! `accepts` names no slot token, a rule naming a descriptor the library lacks, a map placing a
//! descriptor the library lacks, or a map whose `name` is not its file's. Every error starts with the
//! path of the file it is about.

use std::collections::BTreeMap;
use std::path::Path;

use crate::composition::Compositions;
use crate::kits::{self, Bound, Kits};
use crate::map::Map;
use crate::rules::{self, Rules};
use crate::vocab::Vocabularies;

/// The project's vocabulary file, beside `kits.ron`.
pub const VOCAB_FILE: &str = "vocab.ron";

/// Where maps live under the project directory. The editor writes them here too.
pub const MAPS_DIR: &str = "maps";

/// Everything a game needs from `assets/emerge`, loaded and cross-checked.
#[derive(Clone, Debug)]
pub struct ProjectData {
    /// `kits.ron`: the binding, the lattice, the bashes.
    pub kits: Kits,
    /// Every bound kit, layered, and the **merged library** a map resolves against
    /// (`bound.library`).
    pub bound: Bound,
    /// `vocab.ron`.
    pub vocab: Vocabularies,
    /// `rules.ron`, or no rules when the file is absent.
    pub rules: Rules,
    /// `compositions.ron`, validated against [`Self::bound`]'s library; empty when absent. The same
    /// value as `bound.compositions`, held here so a reader need not know it travels with the bind.
    pub compositions: Compositions,
    /// The maps asked for, by name.
    pub maps: BTreeMap<String, Map>,
}

impl ProjectData {
    /// The same as the free [`load`], for callers that spell it `ProjectData::load`.
    pub fn load(emerge_dir: &Path, maps: &[&str]) -> Result<ProjectData, String> {
        load(emerge_dir, maps)
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Load the project in `emerge_dir` (`assets/emerge`) and the named `maps`.
///
/// Order: `kits.ron`, the bound kits, `vocab.ron` (resolved against the merged library so an unknown
/// token fails here), `rules.ron`, then each map. A map named here must exist — a game that asks for
/// `town` and gets nothing has a bug to hear about, not a default to continue with.
pub fn load(emerge_dir: &Path, maps: &[&str]) -> Result<ProjectData, String> {
    let kits_path = emerge_dir.join(kits::KITS_FILE);
    let kits = Kits::parse(&read(&kits_path)?).map_err(|e| format!("{}: {e}", kits_path.display()))?;

    let bound = kits::bound_library(emerge_dir, &kits)?;

    let vocab_path = emerge_dir.join(VOCAB_FILE);
    let vocab =
        Vocabularies::parse(&read(&vocab_path)?).map_err(|e| format!("{}: {e}", vocab_path.display()))?;

    // The lattice check the game's own map loader applies, on the layered merge — what is placed.
    bound
        .library
        .validate_lattices(kits.lattice.face_bands)
        .map_err(|e| format!("{}: {e}", emerge_dir.display()))?;

    // Unknown vocabulary tokens, and a surface class nothing offers, fail here.
    bound
        .library
        .resolve(&vocab)
        .map_err(|e| format!("{}: {e}", emerge_dir.display()))?;

    let compositions_path = emerge_dir.join(Compositions::FILE);
    vocab
        .check_slots(&bound.compositions.compositions)
        .map_err(|e| format!("{}: {e}", compositions_path.display()))?;

    let rules_path = emerge_dir.join(rules::FILE);
    let rules = if rules_path.is_file() {
        let parsed =
            Rules::parse(&read(&rules_path)?).map_err(|e| format!("{}: {e}", rules_path.display()))?;
        parsed
            .validate_against(&bound.library)
            .map_err(|e| format!("{}: {e}", rules_path.display()))?;
        parsed
    } else {
        Rules {
            version: rules::RULES_VERSION,
            blocks: Vec::new(),
        }
    };

    let ids: Vec<String> = bound.library.descriptors.iter().map(|d| d.id.clone()).collect();
    let mut loaded = BTreeMap::new();
    for name in maps {
        let path = emerge_dir
            .join(MAPS_DIR)
            .join(crate::naming::map_file_name(name));
        let map = Map::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        if map.name != *name {
            return Err(format!(
                "{}: calls itself `{}`. A map's name IS its file.",
                path.display(),
                map.name
            ));
        }
        // **The stamps are expanded here, and the map that is returned is the expanded one.**
        //
        // A map holds references to compositions, so the rows a group stands for are not in
        // `placements` — they are derived, and somebody has to derive them. When that somebody
        // was the editor only, the game got a level missing whatever the groups brought: the
        // editor drew a house, the game had no house, and nothing said so. Measured on
        // Sanctuary's `town.map.ron` — 728 rows reached the game where the editor had 746.
        //
        // So the loader is that somebody. It is the one door every consumer goes through, which
        // makes "forgot to expand" impossible rather than merely discouraged, and it is the same
        // expander the editor calls, so the two cannot disagree about what a stamp means.
        //
        // **A stamp that does not expand refuses the load**, by name. The alternative — a map
        // missing rows — is the exact failure this removes.
        let map = map
            .expanded(&bound.compositions.compositions, &bound.library)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // Validated **after** the expansion, so a stamped row naming a descriptor the library
        // lacks is caught here rather than at the game's first spawn.
        map.validate_against(&ids)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        loaded.insert((*name).to_owned(), map);
    }

    let compositions = bound.compositions.clone();
    Ok(ProjectData {
        kits,
        bound,
        vocab,
        rules,
        compositions,
        maps: loaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/emerge")
    }

    /// The shipped fixture binds two kits, has no `rules.ron` and no maps directory.
    #[test]
    fn the_fixture_project_loads_whole() {
        let data = load(&fixture(), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(!data.kits.bind.is_empty(), "the fixture binds kits");
        assert_eq!(data.bound.kits.len(), data.kits.bind.len());
        assert!(!data.bound.library.descriptors.is_empty(), "the merged library has pieces");
        assert!(data.rules.blocks.is_empty(), "no rules.ron means no block rules");
        assert_eq!(data.rules.version, rules::RULES_VERSION);
        assert_eq!(data.compositions, data.bound.compositions);
        assert!(data.maps.is_empty());
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("emerge_project_data_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        dir
    }

    /// A project with no kits is a real state (`Kits::validate`); it loads, and its one missing map
    /// is refused by path.
    #[test]
    fn a_missing_map_names_its_path() {
        let dir = scratch("missing_map");
        let kits = Kits {
            version: kits::KITS_VERSION,
            note: None,
            lattice: kits::Lattice::default(),
            bind: Vec::new(),
            bash: Vec::new(),
            authoring: None,
        };
        std::fs::write(
            dir.join(kits::KITS_FILE),
            kits.to_ron().unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let vocab = ron::to_string(&Vocabularies::default()).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.join(VOCAB_FILE), vocab).unwrap_or_else(|e| panic!("{e}"));

        let loaded = load(&dir, &[]).unwrap_or_else(|e| panic!("an empty project loads: {e}"));
        assert!(loaded.bound.library.descriptors.is_empty());

        let err = load(&dir, &["town"]).expect_err("the map does not exist");
        let want = dir.join("maps").join("town.map.ron");
        assert!(
            err.starts_with(&want.display().to_string()),
            "the error must lead with the map's path {}: {err}",
            want.display()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A missing `kits.ron` is refused by its path too, not with a bare io error.
    #[test]
    fn a_missing_kits_file_names_its_path() {
        let dir = scratch("missing_kits");
        let err = load(&dir, &[]).expect_err("no kits.ron");
        assert!(err.starts_with(&dir.join(kits::KITS_FILE).display().to_string()), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
