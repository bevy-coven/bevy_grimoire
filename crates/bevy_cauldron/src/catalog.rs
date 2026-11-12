//! **The project's files, read and written as kits and maps** — what the project page (`project_page.rs`) lists and acts on.
//!
//! The editor took its project from `argv` and nothing else, and **nothing on screen ever said which kit was loaded**: the window
//! title carries the map name. On 2026-08-15 that cost a session — three relaunches on `--kit site` while asking for a blank
//! slate, a typo'd `--kit site1`, and the question *"there are tons of tiles on the Map tab, where are they stored?"*, which had no
//! answer available from inside the editor. They were the site kit's 45 pieces, and the author had no way to know. So a project is
//! listed as its kits, with how many pieces each holds, and its maps, and can make a new one of either.
//!
//! This was `chooser.rs`'s lower half; its screen is the project page now. Everything here is plain data and plain functions over
//! files, so it is unit tested without a window.
//!
//! # Ordering: fixed and alphabetical, and the reason is in the corpus
//!
//! Sears & Shneiderman 1994, *Split menus* (`10.1145/174630.174632`) is the paper that settles whether a list like this should
//! reorder by use, and it declines to split a menu like this one: the benefit *"will increase with menu length and with more skewed
//! distributions"*, their own guideline caps the high-frequency zone at four items, and with four kits that zone would be the whole
//! list. So the order never moves, and `the_catalog_order_never_moves` pins it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bevy_grimoire_core::composition::{Body, Compositions};
use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::naming;
use bevy_grimoire_core::policy::LIBRARY_FILE;

/// Where kits live, under the project root. The same directory `Project::open` resolves `--kit`
/// against, named once so the two cannot drift.
pub const GRIMOIRE_DIR: &str = "assets/grimoire";

/// Where maps live, under the project directory. The same name `Project::open` resolves, quoted
/// rather than re-decided so the chooser cannot list a map the editor would not open.
pub const MAPS_DIR: &str = "maps";

/// **What one map file is**, as much as the chooser needs to say about it without opening a project.
#[derive(Clone, Debug, PartialEq)]
pub enum MapSummary {
    /// Parsed. The numbers the screen shows beside the row.
    Read {
        placements: usize,
        stamps: usize,
        bounds: (f32, f32, f32),
        /// Where the map's floor starts, in metres.
        origin: (f32, f32, f32),
        /// The author's note on the map.
        note: Option<String>,
        /// The named combination of kits the map draws its pieces from; `None` is every kit.
        bash: Option<String>,
    },
    /// **On disk and unreadable, which is a row and not an omission.** A map that fails to parse is
    /// exactly the one an author needs to be told about; dropping it from the list would present a
    /// broken project as an empty one.
    Unreadable(String),
}

/// One map beside its kit.
#[derive(Clone, Debug, PartialEq)]
pub struct MapEntry {
    /// The map's name — the file stem, which `naming::map_file_name` is the inverse of.
    pub name: String,
    pub path: PathBuf,
    pub summary: MapSummary,
}

/// One kit: a directory under [`GRIMOIRE_DIR`] holding a `library.yaml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Kit {
    /// What `--kit` would be given. `None` for the root kit, which `Project::open(kit: None)` opens
    /// by treating `assets/grimoire` itself as the kit — a mode a subdirectory scan can never produce,
    /// so it is carried explicitly rather than left unreachable.
    pub flag: Option<String>,
    /// What the screen shows. The directory's own name either way, so the root kit reads `grimoire`.
    pub label: String,
    pub dir: PathBuf,
    /// Descriptors in `library.yaml`. **The fact this screen exists to show.**
    pub pieces: usize,
    /// **The name this kit answers to** — its `kits.yaml` binding when it has one, and its own ids'
    /// namespace otherwise.
    ///
    /// The binding first, because that is the one that is true for a **flat** library: the furniture
    /// kit's 75 ids carry no namespace at all, so reading them answers `None` while `kits.yaml` says
    /// `furniture`. Keying anything off the ids alone made the kit selection inert on every kit that
    /// ships — see `Project::kit_of`.
    pub namespace: Option<String>,
    /// **Whether this project's `kits.yaml` binds this directory** — which is the only thing that
    /// decides if `Project::open` can open it.
    ///
    /// Not derivable from [`namespace`](Self::namespace), and that cost a wrong fix on 2026-09-03:
    /// `read_kit` fills `namespace` from the kit's **own** `library.yaml`, and the binding pass then
    /// overwrites it — so an unbound kit carries a namespace too, and `namespace.is_some()` is true
    /// for every kit that names itself. `site` is the live example: it declares `site` in its own
    /// library and appears nowhere in `kits.yaml`.
    pub bound: bool,
    /// Every id this kit defines, so a placement can be traced back to the kit that provides it
    /// without re-reading a library per frame. `read_kit` parses it anyway.
    pub ids: BTreeSet<String>,
}

/// Every kit under a project root, and every map in the project.
///
/// **The maps are the project's, not a kit's**, and that is the change of 2026-08-16. They sat
/// inside the kit that drew them while a project *was* one kit; a map now resolves against the merge
/// of every bound kit, so there is no kit to file it under.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub kits: Vec<Kit>,
    pub maps: Vec<MapEntry>,
    /// **Which kit new work lands in** — `kits.yaml`'s `authoring`, read once at scan. `None` when
    /// the project has no kits yet, which is the same state as a project that never had one.
    pub authoring: Option<String>,
    /// **The named combinations a map can be given** — `kits.yaml`'s `bash` names, in file order,
    /// read in the same breath as the bindings. a map's "uses pieces from" is one of them; a project
    /// declaring none has nothing to choose, and `cycle_bash` says so.
    pub bashes: Vec<String>,
}

impl Catalog {
    /// **Scan a project root.**
    ///
    /// A directory is a kit iff it holds a `library.yaml` — the same test `Project::open` applies
    /// before it will accept a `--kit`, quoted rather than re-decided so the chooser cannot offer a
    /// kit the editor would then refuse.
    ///
    /// Only `library.yaml` is parsed. A full `Project::open` would additionally read the policy, the
    /// compositions and every mask, for a list nobody has chosen from yet — seconds of work per kit
    /// to fill in one number.
    pub fn scan(root: &Path) -> Result<Catalog, String> {
        let base = root.join(GRIMOIRE_DIR);
        if !base.is_dir() {
            return Err(format!(
                "{} is not a project: it has no `{GRIMOIRE_DIR}` directory. Run the editor from the \
                 repository root, or pass the root as the first argument.",
                root.display()
            ));
        }

        let mut kits = Vec::new();
        // The root kit first, if it is one — `Project::open(None)` opens exactly this.
        if let Some(mut kit) = read_kit(&base, None)? {
            // The root kit is opened by `Project::open(None)` and needs no entry in `kits.yaml`.
            kit.bound = true;
            kits.push(kit);
        }

        let entries =
            std::fs::read_dir(&base).map_err(|e| format!("cannot read {}: {e}", base.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            if let Some(kit) = read_kit(&path, Some(name))? {
                kits.push(kit);
            }
        }

        // **The binding names a kit, and it is the only name a flat library has.** Read after the
        // scan rather than during it: a directory that is a kit and a directory that is *bound* are
        // two questions, and this screen lists the first while the editor loads the second.
        let kits_ron = std::fs::read_to_string(base.join(bevy_grimoire_core::kits::KITS_FILE))
            .ok()
            .and_then(|t| bevy_grimoire_core::kits::Kits::parse(&t).ok());
        let bindings = kits_ron
            .as_ref()
            .map(|k| k.bind.clone())
            .unwrap_or_default();
        for kit in &mut kits {
            if let Some(b) = bindings.iter().find(|b| Some(b.dir.as_str()) == kit.flag.as_deref()) {
                kit.namespace = Some(b.namespace.clone());
                kit.bound = true;
            }
        }
        // **Where new work lands**, read in the same breath as the bindings — it is a field of the
        // same file, and a second read would be a second chance for the two to disagree.
        let authoring = kits_ron.as_ref().and_then(|k| k.authoring.clone());
        // **The declared bashes**, from the same parse and in file order — `B` on a map row walks
        // them in exactly this order, so the screen and the file agree about what "next" is.
        let bashes = kits_ron
            .as_ref()
            .map(|k| k.bash.iter().map(|b| b.name.clone()).collect())
            .unwrap_or_default();

        // **Fixed order, every scan.** See the module note on Sears & Shneiderman: nothing here is
        // sorted by use, and `the_catalog_order_never_moves` is what keeps that true.
        kits.sort_by(|a, b| a.label.cmp(&b.label));
        // One list for the project. `maps/` may not exist yet in a project nobody has saved from,
        // and that is an empty list rather than an error — the `+ new map` row is the instruction.
        let maps = read_maps(&base.join(MAPS_DIR))?;
        Ok(Catalog {
            kits,
            maps,
            authoring,
            bashes,
        })
    }
}

/// One directory, if it is a kit at all.
fn read_kit(dir: &Path, flag: Option<String>) -> Result<Option<Kit>, String> {
    let library = dir.join(LIBRARY_FILE);
    if !library.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&library)
        .map_err(|e| format!("cannot read {}: {e}", library.display()))?;
    let parsed = bevy_grimoire_core::library::Library::parse(&text)
        .map_err(|e| format!("{}: {e}", library.display()))?;
    let pieces = parsed.descriptors.len();
    // A library disagreeing with itself is refused at open by `kits::bound_library`. Here it is a
    // list nobody has chosen from yet, so it reads as "no single namespace" rather than as a refusal
    // that would stop the whole screen drawing.
    let namespace = parsed.namespace().ok().flatten().map(str::to_owned);
    let ids: BTreeSet<String> = parsed.descriptors.iter().map(|d| d.id.clone()).collect();

    let label = dir.file_name().map_or_else(
        || dir.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    Ok(Some(Kit {
        flag,
        label,
        dir: dir.to_path_buf(),
        pieces,
        namespace,
        // Set by the binding pass in `Catalog::scan`; a kit read on its own is not yet bound.
        bound: false,
        ids,
    }))
}

/// Every `*.map.yaml` beside a kit, alphabetical.
fn read_maps(dir: &Path) -> Result<Vec<MapEntry>, String> {
    const SUFFIX: &str = ".map.yaml";
    let mut out = Vec::new();
    // **No `maps/` yet is no maps**, not a broken project: it is made by the first save, and a
    // project nobody has saved from is exactly where the `+ new map` row is the instruction.
    if !dir.is_dir() {
        return Ok(out);
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let Some(name) = file.strip_suffix(SUFFIX) else {
            continue;
        };
        // `Map::parse` validates, so a map whose name or bounds are unusable lands in `Unreadable`
        // with the reason the editor would have given — reported, never dropped.
        let summary = match std::fs::read_to_string(&path) {
            Err(e) => MapSummary::Unreadable(format!("cannot read: {e}")),
            Ok(text) => match Map::parse(&text) {
                Err(e) => MapSummary::Unreadable(e),
                Ok(map) => MapSummary::Read {
                    placements: map.placements.len(),
                    stamps: map.stamps.len(),
                    bounds: map.bounds,
                    origin: map.origin,
                    note: map.note.clone(),
                    bash: map.bash.clone(),
                },
            },
        };
        out.push(MapEntry {
            name: name.to_owned(),
            path: path.clone(),
            summary,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// **Where the game names the pieces it needs.** Scanned by [`dependents`], and the reason that
/// function exists: `src/site/kit.rs` holds `assets/grimoire/site` in a `&'static str`, so no scan of
/// the editor's own content can ever see the game's dependency on it. These files can be — they
/// name descriptor ids in the same namespace the kit provides.
const GAME_KIT_DIR: &str = "assets/site";

/// **Every id a kit provides** — its descriptors, and the compositions authored beside them.
///
/// Read off the files rather than inferred from the directory name, because what a kit provides is
/// a property of what it defines. `site/` and `site_greybox/` both provide the identical 45 `site/*`
/// ids; that is what makes one a re-skin of the other rather than a second kit, and it is why
/// deleting one of them can be safe while deleting the last one is not.
fn provided_ids(dir: &Path) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();

    let library = dir.join(LIBRARY_FILE);
    if library.is_file() {
        let text = std::fs::read_to_string(&library)
            .map_err(|e| format!("cannot read {}: {e}", library.display()))?;
        let parsed = bevy_grimoire_core::library::Library::parse(&text)
            .map_err(|e| format!("{}: {e}", library.display()))?;
        ids.extend(parsed.descriptors.into_iter().map(|d| d.id));
    }

    let comps = dir.join(Compositions::FILE);
    if comps.is_file() {
        let text = std::fs::read_to_string(&comps)
            .map_err(|e| format!("cannot read {}: {e}", comps.display()))?;
        let parsed =
            Compositions::parse(&text).map_err(|e| format!("{}: {e}", comps.display()))?;
        ids.extend(parsed.compositions.into_iter().map(|c| c.id));
    }

    Ok(ids)
}

/// **Every id one map or composition file names**, structurally — never by looking at the text.
///
/// A `note:` mentioning `site/wall` is prose, and a scan that could not tell the difference would
/// refuse a deletion for a sentence somebody wrote.
fn ids_named(path: &Path) -> Result<BTreeSet<String>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut ids = BTreeSet::new();

    if path.file_name().is_some_and(|n| n == Compositions::FILE) {
        let parsed = Compositions::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        for comp in parsed.compositions {
            for member in comp.members {
                match member.body {
                    // A slot names a *token*, not an id — `Body::target` would hand back the token
                    // and it would be compared against ids it can never be one of.
                    Body::Descriptor { id, .. } | Body::Composition { id } => {
                        ids.insert(id);
                    }
                    Body::Slot { .. } => {}
                }
            }
        }
        return Ok(ids);
    }

    let map = Map::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    ids.extend(map.placements.into_iter().map(|p| p.descriptor));
    ids.extend(map.stamps.into_iter().map(|s| s.of));
    Ok(ids)
}

/// **What would be left naming a piece nothing provides, if this kit went.**
///
/// The rule is not "is anything using this kit" but **"is this kit the last provider"** — the
/// distinction the re-skin pair forces. Removing one of two directories that define the same ids
/// costs nothing; removing the only one strands every reference to them.
///
/// Two readers, because there are two formats and one of them belongs to another crate. Maps and
/// `compositions.yaml` are parsed (see [`ids_named`]). The game's kit files under [`GAME_KIT_DIR`]
/// are read as text: `SiteKit`'s schema lives in the game, `bevy_cauldron` does not depend on the
/// game and must not start, and a quoted-id match over a file that is a list of quoted ids is
/// exact enough to be worth more than the coupling.
///
/// Returns paths relative to `root`, in scan order, or an empty list when nothing is stranded.
pub fn dependents(root: &Path, kit: &Kit, catalog: &Catalog) -> Result<Vec<String>, String> {
    let mine = provided_ids(&kit.dir)?;
    if mine.is_empty() {
        return Ok(Vec::new());
    }

    // What another directory would still provide once this one is gone.
    let mut remaining = BTreeSet::new();
    for other in &catalog.kits {
        if other.dir != kit.dir {
            remaining.extend(provided_ids(&other.dir)?);
        }
    }
    let lost: BTreeSet<&String> = mine.difference(&remaining).collect();
    if lost.is_empty() {
        return Ok(Vec::new());
    }

    let show = |p: &Path| {
        p.strip_prefix(root)
            .unwrap_or(p)
            .display()
            .to_string()
            .replace('\\', "/")
    };
    let mut found = Vec::new();

    // **Every map, and the one composition collection.** Both are the project's now, so nothing
    // here is "this kit's own, excepted": a tile that seats this kit's pieces lives in the project's
    // `compositions.yaml` and would be stranded exactly like a map would.
    let comps = root.join(GRIMOIRE_DIR).join(Compositions::FILE);
    let files = catalog
        .maps
        .iter()
        .map(|m| m.path.clone())
        .chain(comps.is_file().then_some(comps));
    for file in files {
        if ids_named(&file)?.iter().any(|id| lost.contains(id)) {
            found.push(show(&file));
        }
    }

    // And the game, which no amount of scanning `assets/grimoire` would ever have found.
    let game = root.join(GAME_KIT_DIR);
    if game.is_dir() {
        let entries = std::fs::read_dir(&game)
            .map_err(|e| format!("cannot read {}: {e}", game.display()))?;
        let mut game_files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with("kit_") && n.ends_with(".yaml")
                })
            })
            .collect();
        game_files.sort();
        for file in game_files {
            let text = std::fs::read_to_string(&file)
                .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
            if lost.iter().any(|id| text.contains(&format!("\"{id}\""))) {
                found.push(show(&file));
            }
        }
    }

    Ok(found)
}

/// **The refusal, when [`dependents`] found something** — or `None` when it did not.
///
/// Terse, on the precedent the root-kit guard set when its explanation was reported as too much:
/// *"just say it can't be deleted."* So it says which kit, how many files, and names the first
/// three — enough to go and look, read in the time it takes to reach for the next key.
///
/// **Three, and then a count.** Naming all 51 would be a screen of paths where the point is that
/// there are 51 of them.
fn strands(label: &str, users: &[String]) -> Option<String> {
    const SHOWN: usize = 3;
    if users.is_empty() {
        return None;
    }
    let listed = users
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let rest = users.len().saturating_sub(SHOWN);
    let more = if rest > 0 {
        format!(", and {rest} more")
    } else {
        String::new()
    };
    let files = if users.len() == 1 { "file" } else { "files" };
    Some(format!(
        "`{label}` is the last kit providing pieces that {} {files} still name — {listed}{more}",
        users.len()
    ))
}

/// **The two files a new kit is made of**, as text: its `library.yaml` and its `project.yaml`. One place,
/// so `kit_create` can tell on undo that a kit's files are still exactly what it wrote.
pub(crate) fn new_kit_files(name: &str) -> (String, String) {
    let library = format!(
        "{{version: 1, note: \"Empty on purpose — `{name}` is a new kit. Import meshes on the Meshes tab; they land here.\", descriptors: []}}\n"
    );
    // **No lattice setting here, and that is the change of 2026-08-16.** `face_bands` and
    // `snap_divisor` describe a lattice, and a kit does not have one — a map has exactly one, so
    // they live on `Map` now. A new kit's policy is genuinely empty, and it says so rather than
    // carrying a number it cannot own.
    let policy = format!(
        "{{version: 2, note: \"The policy layer for `{name}`. No patches yet: `Project::open` refuses a rule that matches nothing, so one cannot be added before the pieces it names exist.\"}}\n"
    );
    (library, policy)
}

/// **Make a new, empty kit** — a directory the editor will accept as one.
///
/// A kit is four decisions and two files: a `library.yaml` saying it has no pieces yet, and a
/// `project.yaml` saying how finely its tiles divide. Both are written empty and **say so in their
/// own `note:`**, because an empty kit and a broken one look identical on disk otherwise — that is
/// the confusion `assets/grimoire/site_v2` was created out of, and its files carry the same kind of
/// note for the same reason.
///
/// No `compositions.yaml`: `policy::layered_library` treats its absence and an empty one as the same
/// state, deliberately, so writing one would be a file that means nothing. The editor makes it when
/// the first tile is saved.
///
/// No map either. The maps column will read *"no maps in <kit> yet — the `+ new map` row is the only one there"*, which is
/// the instruction rather than a report (`docs/ui.md` §1.4) and is exactly what to do next.
pub fn create_kit(root: &Path, raw_name: &str) -> Result<PathBuf, String> {
    let name = naming::to_snake_case(raw_name);
    if name.is_empty() {
        return Err(format!(
            "`{raw_name}` leaves nothing usable as a name. Kits are snake_case — lowercase \
             letters, digits and single underscores, starting with a letter."
        ));
    }
    let dir = root.join(GRIMOIRE_DIR).join(&name);
    if dir.exists() {
        return Err(format!(
            "`{name}` already exists under {GRIMOIRE_DIR}. Pick another name, or open the one that \
             is there."
        ));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;

    let (library, policy) = new_kit_files(&name);
    bevy_grimoire_core::save::save_atomic(&dir.join(LIBRARY_FILE), &library)?;

    bevy_grimoire_core::save::save_atomic(&dir.join(bevy_grimoire_core::policy::POLICY_FILE), &policy)?;

    // **And bound, or it is a directory the project does not load.** Making a kit the editor cannot
    // open would be a verb that appears to work and does not — the `--kit` refusal an author would
    // then hit names `kits.yaml`, which they never edited.
    //
    // Bound as its own name, because an empty library carries no namespace to contradict. Re-point
    // it by hand when the kit turns out to be a second skin of one that already exists — that is a
    // decision about the project, and `Kits::validate` refuses two directories for one namespace.
    bind_kit(root, &name)?;
    Ok(dir)
}
/// **Write a new project beside this one** — the directory that holds kits, maps, compositions and
/// the vocabulary.
///
/// The shape `assets/grimoire/` *is* a project (`CLAUDE.md`'s Data model); this is the verb that
/// makes one. Nothing is opened in-process — switching the live root is a separate change — so the
/// caller reports `bevy_cauldron <dir>` and the author runs it.
pub fn create_project(parent: &Path, raw_name: &str, vocab_source: &Path) -> Result<PathBuf, String> {
    let vocab = std::fs::read(vocab_source).map_err(|e| format!("cannot copy {} to vocab.yaml: {e}", vocab_source.display()))?;
    create_project_with(parent, raw_name, &vocab)
}

/// [`create_project`] with the vocabulary's bytes in hand rather than a file to copy: the launcher's new-project
/// wizard (`new_project.rs`) has no open project to copy one from, and writes the one built into the editor.
pub fn create_project_with(parent: &Path, raw_name: &str, vocab: &[u8]) -> Result<PathBuf, String> {
    let name = naming::to_snake_case(raw_name);
    if name.is_empty() {
        return Err(format!(
            "`{raw_name}` leaves nothing usable as a name. Projects are snake_case — lowercase \
             letters, digits and single underscores, starting with a letter."
        ));
    }
    let dir = parent.join(&name);
    if dir.exists() {
        return Err(format!(
            "`{name}` already exists beside this project. Pick another name, or open the one that \
             is there."
        ));
    }
    write_project_files(&dir, vocab)?;
    Ok(dir)
}

/// **Make the folder `dir` a project in place**: it gets what a project needs and keeps everything it held. Refused for a
/// path that is not a folder, and for one that already is a project (its files are not rewritten).
pub fn make_project_here(dir: &Path, vocab: &[u8]) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!("{} is not a folder", dir.display()));
    }
    if dir.join(GRIMOIRE_DIR).exists() {
        return Err(format!("{} is already a project", dir.display()));
    }
    write_project_files(dir, vocab)
}

/// What a project is made of, written under `dir`: `kits.yaml` with no kit, and the vocabulary.
fn write_project_files(dir: &Path, vocab: &[u8]) -> Result<(), String> {
    let grimoire = dir.join(GRIMOIRE_DIR);
    std::fs::create_dir_all(&grimoire).map_err(|e| format!("cannot make {}: {e}", grimoire.display()))?;

    // **The version comes from the constant, never a literal** — the same rule the chooser's own
    // fixture records: bumping `KITS_VERSION` must not write a file the schema refuses.
    let kits = format!(
        "{{version: {}, bind: [], authoring: null, bash: []}}",
        bevy_grimoire_core::kits::KITS_VERSION
    );
    let kits_path = grimoire.join(bevy_grimoire_core::kits::KITS_FILE);
    bevy_grimoire_core::atomic_file::write(&kits_path, kits).map_err(|e| format!("{}: {e}", kits_path.display()))?;

    // **Copy the vocabulary byte-for-byte.** A copy preserves the rationale comments; serializing
    // `Vocabularies` would delete them. The source is the current project's `vocab.yaml`, which is
    // the only vocabulary on hand.
    bevy_grimoire_core::atomic_file::write(&grimoire.join("vocab.yaml"), vocab)
        .map_err(|e| format!("cannot write {}: {e}", grimoire.join("vocab.yaml").display()))?;

    // **Write nothing else.** No `compositions.yaml` (absence and empty are the same state, per
    // `create_kit`'s own note), no `maps/` (`save_atomic` creates it), no kit — the `+ new kit`
    // row makes one, and because `create_kit` calls `bind_kit`, that first kit also becomes the
    // authoring kit with no extra code.
    Ok(())
}

/// **Write the map's bash**, leaving every other field as it was. Same door `write_settings` uses:
/// parse, set, validate, serialize, atomic write.
///
/// Through the `Map` schema the editor and the game read, rather than spliced as text — `map.rs`'s
/// own rule: *"an grimoire map is serialized normally and never text-spliced"*, because every reason
/// a map has lives in a field.
pub fn write_bash(path: &Path, bash: Option<&str>) -> Result<(), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut map = Map::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    map.bash = bash.map(str::to_owned);
    map.validate()?;
    let out = map.to_yaml()?;
    bevy_grimoire_core::save::save_atomic(path, &out)
}

/// **Add a binding for `name`**, leaving the rest of `kits.yaml` as it was.
fn bind_kit(root: &Path, name: &str) -> Result<(), String> {
    let path = root.join(GRIMOIRE_DIR).join(bevy_grimoire_core::kits::KITS_FILE);
    // **A directory that is not a project hits this first** — `+ new kit` in a bare folder has no
    // `kits.yaml` to bind into. A raw io error would read as "the disk failed"; this says what the
    // directory is and what makes one.
    let text = std::fs::read_to_string(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "{} does not exist — this directory is not a project. Make one with `+ new project`.",
                path.display()
            )
        } else {
            format!("cannot read {}: {e}", path.display())
        }
    })?;
    let mut kits =
        bevy_grimoire_core::kits::Kits::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    kits.bind.push(bevy_grimoire_core::kits::Bind {
        namespace: name.to_owned(),
        dir: name.to_owned(),
    });
    // **The first kit made is where work lands**, because it is the only kit there is. Not a
    // default chosen over alternatives — there are none — and an author who makes a second one and
    // wants it instead says so in this file.
    if kits.authoring.is_none() {
        kits.authoring = Some(name.to_owned());
    }
    let out = kits.to_yaml().map_err(|e| format!("{}: {e}", path.display()))?;
    // Parsed back before it is written, so a binding that would refuse to load is refused here —
    // where the author is standing — rather than at the next open.
    bevy_grimoire_core::kits::Kits::parse(&out).map_err(|e| format!("{}: {e}", path.display()))?;
    bevy_grimoire_core::save::save_atomic(&path, &out)
}

/// **Point `authoring` at `name`, leaving the rest of `kits.yaml` as it was.**
///
/// The same shape as `bind_kit`: parsed, edited, re-parsed before writing, and `save_atomic`'d —
/// so a file that would refuse to load is refused here, where the author is standing, rather than
/// at the next open. `name` must be bound; the refusal names the bound kits, because "not bound"
/// without a list is a search.
pub(crate) fn set_authoring(root: &Path, name: &str) -> Result<(), String> {
    let path = root.join(GRIMOIRE_DIR).join(bevy_grimoire_core::kits::KITS_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut kits =
        bevy_grimoire_core::kits::Kits::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if !kits.bind.iter().any(|b| b.dir == name) {
        return Err(format!(
            "`{name}` is not bound in {}. The bound kits are: {}.",
            path.display(),
            kits.bind
                .iter()
                .map(|b| format!("`{}`", b.dir))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    kits.authoring = Some(name.to_owned());
    let out = kits.to_yaml().map_err(|e| format!("{}: {e}", path.display()))?;
    // Parsed back before it is written, so a binding that would refuse to load is refused here —
    // where the author is standing — rather than at the next open.
    bevy_grimoire_core::kits::Kits::parse(&out).map_err(|e| format!("{}: {e}", path.display()))?;
    bevy_grimoire_core::save::save_atomic(&path, &out)
}

/// **Drop `name`'s binding.** Called when its directory goes, so the project does not keep asking
/// for a kit that is not there.
pub fn unbind_kit(root: &Path, name: &str) -> Result<(), String> {
    let path = root.join(GRIMOIRE_DIR).join(bevy_grimoire_core::kits::KITS_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut kits =
        bevy_grimoire_core::kits::Kits::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    // **The kit new work lands in is not deletable while it is that kit.** Removing it would leave
    // `authoring` naming nothing, which `Kits::validate` refuses — so the project would stop opening
    // as a result of a verb that said it had succeeded.
    if kits.authoring.as_deref() == Some(name) {
        // **Unless it is the last one**, in which case the project is going back to having no kits
        // and says so, exactly as a project that never had one does. One rule either way: a project
        // never points `authoring` at a kit it does not have.
        if kits.bind.len() > 1 {
            return Err(format!(
                "`{name}` is where new work lands. Point `authoring` in {} at another kit first.",
                path.display()
            ));
        }
        kits.authoring = None;
    }
    kits.bind.retain(|b| b.dir != name);
    let out = kits.to_yaml().map_err(|e| format!("{}: {e}", path.display()))?;
    bevy_grimoire_core::kits::Kits::parse(&out).map_err(|e| format!("{}: {e}", path.display()))?;
    bevy_grimoire_core::save::save_atomic(&path, &out)
}

/// **Write a new, empty map into a kit** — the chooser's one creating verb.
///
/// # The name is not defaulted
///
/// `Map::default()` leaves `name` empty on purpose, and `bevy_grimoire_core` says why at the field:
/// *"Empty, not 'untitled': a substituted name is a name nobody chose, and the second one collides
/// with the first."* So this refuses an unusable name by name rather than inventing one, and the
/// screen keeps the field blank until the author types something. `Map::validate` would refuse it
/// anyway; asking first means the refusal names the rule instead of a serializer.
///
/// Goes through `validate` then `save::save_atomic` — the same two steps `Project::save`
/// takes, so a map made here and a map saved from the editor cannot disagree about what a map is.
pub fn create_map(
    maps_dir: &Path,
    raw_name: &str,
    bounds: (f32, f32, f32),
    origin: (f32, f32, f32),
    note: Option<String>,
) -> Result<PathBuf, String> {
    let name = naming::to_snake_case(raw_name);
    if name.is_empty() {
        return Err(format!(
            "`{raw_name}` leaves nothing usable as a name. Names are snake_case — lowercase \
             letters, digits and single underscores, starting with a letter."
        ));
    }
    let path = maps_dir.join(naming::map_file_name(&name));
    if path.exists() {
        return Err(format!(
            "`{name}` already exists in this kit. Pick another name, or open the one that is there."
        ));
    }

    let map = Map {
        name,
        bounds,
        origin,
        note,
        ..Map::default()
    };
    map.validate()?;
    let text = map.to_yaml()?;
    bevy_grimoire_core::save::save_atomic(&path, &text)?;
    Ok(path)
}

/// **A destructive act that has been asked for and not yet agreed to.**
///
/// Deleting a map removes a file, and there is no undo for it here — the editor's undo stack does not survive the map being
/// gone. So the act is split in two: asking, which changes nothing, and agreeing, which is the answer to a question naming exactly
/// what will go. `docs/ui.md` §1.4's rule applies to the question as much as to a refusal — it names the map and the file, because
/// "are you sure?" is not information.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    /// What the row said — the map's name, or the kit's label.
    pub name: String,
    /// The file or directory that goes. **A path, never a row index**: a question remembering "row 2" deletes whatever row 2
    /// became if the list moved underneath it.
    pub path: PathBuf,
    /// `true` when `path` is a whole kit directory. Decides `remove_dir_all` against `remove_file`, and it is carried rather than
    /// re-derived so the question and the act cannot disagree about what is being removed.
    pub kit: bool,
}

/// **`1 map`, not `1 maps`.** A count that disagrees with its noun reads as something generated rather than something written,
/// in the one sentence that has to be trusted before a directory is removed.
pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

impl Catalog {
    /// The kit called `label`.
    pub fn kit(&self, label: &str) -> Option<&Kit> {
        self.kits.iter().find(|k| k.label == label)
    }

    /// The map called `name`.
    pub fn map(&self, name: &str) -> Option<&MapEntry> {
        self.maps.iter().find(|m| m.name == name)
    }
}

/// **Ask to delete the map `name`.** Changes nothing on disk — see [`Pending`].
pub fn ask_delete_map(catalog: &Catalog, name: &str) -> Result<Pending, String> {
    let map = catalog.map(name).ok_or_else(|| format!("there is no map `{name}` to delete"))?;
    Ok(Pending { name: map.name.clone(), path: map.path.clone(), kit: false })
}

/// **Ask to delete the kit `label`.** Changes nothing on disk.
///
/// A kit goes as a whole directory, which is why the question names what is inside it. The root kit is refused, and this is the
/// guard that matters: it is `assets/grimoire` itself — the directory every other kit is a subdirectory of, and the one holding the
/// shared vocabulary — so it is not offered rather than offered and warned about. A kit that something else still names is refused
/// too, with the files that name it ([`dependents`]): deleting one cost the shipped kit once, with 51 tests and the game's ability to
/// boot.
pub fn ask_delete_kit(root: &Path, catalog: &Catalog, label: &str) -> Result<Pending, String> {
    let kit = catalog.kit(label).ok_or_else(|| format!("there is no kit `{label}` to delete"))?;
    if kit.flag.is_none() {
        return Err(format!("`{}` is the default kit and cannot be deleted", kit.label));
    }
    let users = dependents(root, kit, catalog)?;
    if let Some(problem) = strands(&kit.label, &users) {
        return Err(problem);
    }
    Ok(Pending { name: kit.label.clone(), path: kit.dir.clone(), kit: true })
}

/// **Agree to it.** Removes what the question named.
///
/// **Unbind first, and refuse the whole act if that refuses.** The binding is the project's statement that this kit exists;
/// removing the directory while `kits.yaml` still names it leaves a project that will not open, which is a far worse outcome than a
/// refusal. In this order the failure costs nothing — the directory is still there.
pub fn delete_entry(root: &Path, pending: &Pending) -> Result<(), String> {
    if pending.kit {
        unbind_kit(root, &pending.name)?;
    }
    // One call or the other, chosen by what the question captured — not by looking at the path again, which could have become a
    // different kind of thing in between.
    let gone = if pending.kit { std::fs::remove_dir_all(&pending.path) } else { std::fs::remove_file(&pending.path) };
    gone.map_err(|e| format!("could not delete `{}`: {e}", pending.name))
}

/// **The next "uses pieces from" for the map `name`**, written to its file: `None → bashes[0] → … → bashes[n-1] → None`, read from
/// the file's current `bash` so the cycle follows disk. Returns the one it landed on (`None` is every kit).
///
/// Selecting is the only bash verb here: a bash is authored by hand in `kits.yaml`, the same way its `lattice` is, and ticking kits per
/// map would mean editing a shared combination from a map, which silently changes every other map naming it.
pub fn cycle_bash(catalog: &Catalog, name: &str) -> Result<Option<String>, String> {
    let Some(entry) = catalog.map(name) else {
        return Err("there is no map here to give a bash to".to_owned());
    };
    if matches!(entry.summary, MapSummary::Unreadable(_)) {
        return Err("this map will not open, so its bash cannot be set".to_owned());
    }
    if catalog.bashes.is_empty() {
        return Err("this project declares no bashes — add one to `kits.yaml` under `bash`".to_owned());
    }
    // **Read from the file, not from a summary.** The summary carries what a row shows, and this verb's whole job is to move a field the
    // row does not carry.
    let text = std::fs::read_to_string(&entry.path).map_err(|e| format!("cannot read {}: {e}", entry.path.display()))?;
    let map = Map::parse(&text).map_err(|e| format!("{}: {e}", entry.path.display()))?;
    let next = match map.bash.as_deref() {
        // Every kit is where the cycle starts, so the first press names the first bash.
        None => catalog.bashes.first().cloned(),
        // A name the project no longer declares lands back at every kit, which is the one state that is always true — and
        // `OpenMap::open` would refuse the map until it does.
        Some(current) => catalog.bashes.iter().position(|b| b == current).and_then(|i| catalog.bashes.get(i + 1)).cloned(),
    };
    write_bash(&entry.path, next.as_deref())?;
    Ok(next)
}

/// A map's size as a person says it: `10 × 10 m, 4 m tall` (width, depth, height in metres).
pub fn size_in_metres(bounds: (f32, f32, f32)) -> String {
    format!("{} × {} m, {} m tall", bounds.0, bounds.2, bounds.1)
}

/// **One entry in a kit's policy, keyed for removal.**
///
/// Keyed on the entry's place, not on its content: nothing forbids two patches sharing a `matches` key, so a removal keyed on
/// `matches` is ambiguous by construction.
#[derive(Clone, Debug, PartialEq)]
pub enum PolicyRow {
    /// An exclusion: a mesh path prefix the kit is not built from.
    Exclude(String),
    /// A patch, by its position within `patches`.
    Patch(usize),
}

/// One policy entry as a person reads it, with the row that removes it.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyEntry {
    pub row: PolicyRow,
    pub shown: String,
}

/// Where a kit's policy lives.
fn policy_path(kit: &Kit) -> PathBuf {
    kit.dir.join(bevy_grimoire_core::policy::POLICY_FILE)
}

fn read_policy(kit: &Kit) -> Result<bevy_grimoire_core::policy::Policy, String> {
    let path = policy_path(kit);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    bevy_grimoire_core::policy::Policy::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// **A kit's policy as rows**: its exclusions, then its patches. A kit with no `project.yaml` has declared nothing, which is no rows
/// and not a refusal.
pub fn policy_entries(kit: &Kit) -> Result<Vec<PolicyEntry>, String> {
    if !policy_path(kit).is_file() {
        return Ok(Vec::new());
    }
    let policy = read_policy(kit)?;
    let mut rows: Vec<PolicyEntry> = policy.exclude.iter().map(|pack| PolicyEntry { row: PolicyRow::Exclude(pack.clone()), shown: format!("Leaves out `{pack}`") }).collect();
    for (ordinal, patch) in policy.patches.iter().enumerate() {
        let what = match &patch.matches {
            bevy_grimoire_core::policy::Match::Id(id) => format!("the piece `{id}`"),
            bevy_grimoire_core::policy::Match::Kind(kind) => format!("every `{kind}`"),
        };
        rows.push(PolicyEntry { row: PolicyRow::Patch(ordinal), shown: format!("Changes {what}: {}", patch.because) });
    }
    Ok(rows)
}

/// **Remove one policy entry.** Nothing leaves the file system: a line is spliced out of the kit's `project.yaml`, which keeps every comment
/// in it (`yaml_surgery`).
pub fn remove_policy_entry(kit: &Kit, row: &PolicyRow) -> Result<(), String> {
    let path = policy_path(kit);
    match row {
        PolicyRow::Exclude(pack) => {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let mut exclude = read_policy(kit)?.exclude;
            exclude.retain(|p| p != pack);
            let out = bevy_grimoire_core::policy::rewrite_exclude(&text, &exclude).map_err(|e| format!("{}: {e}", path.display()))?;
            bevy_grimoire_core::save::save_atomic(&path, &out)
        }
        PolicyRow::Patch(ordinal) => bevy_grimoire_core::policy::remove_patch(&path, *ordinal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bevy_grimoire_core::map::Placed;

    /// A throwaway project root: `assets/grimoire/` plus whatever kits a test asks for.
    struct Root(PathBuf);

    impl Root {
        fn new(name: &str) -> Root {
            // **One parent per test, not one shared `$TMPDIR`.**
            //
            // `Catalog::scan` reads every *sibling* of the root as a project, so a fixture rooted
            // straight in `$TMPDIR` made every chooser test a sibling of every other — scanned by
            // each, and (worse) writable by each. Two runs in one process could then disagree about
            // what is beside them, which is a test failing for a reason no diff explains.
            //
            // **Measured 2026-10-24**: `making_a_project_reports_the_command_that_opens_it` failed
            // once in a full-workspace run and passed alone three times after, and `$TMPDIR` was
            // littered with sibling test projects. The parent is the test's own now, so what it sees
            // beside it is what it made.
            let dir = std::env::temp_dir()
                .join(format!("chooser-{name}-{}", std::process::id()))
                .join("project");
            let _ = std::fs::remove_dir_all(&dir);
            let base = dir.join(GRIMOIRE_DIR);
            std::fs::create_dir_all(&base).unwrap_or_else(|e| panic!("{}: {e}", base.display()));
            // A project that binds nothing — which is where an author starts, and the state
            // `bind_kit` gets them out of.
            // **The version comes from the constant, never a literal.** It was `1` written out
            // here, so bumping `KITS_VERSION` failed thirty chooser tests at once with a schema
            // refusal — the fixture disagreeing with the schema it is a fixture for.
            std::fs::write(
                base.join(bevy_grimoire_core::kits::KITS_FILE),
                format!(
                    "{{version: {}, bind: [], authoring: null, bash: []}}",
                    bevy_grimoire_core::kits::KITS_VERSION
                ),
            )
            .unwrap_or_else(|e| panic!("{}: {e}", base.display()));
            Root(dir)
        }

        /// Where this project's maps live. They left the kit directories on 2026-08-16.
        fn maps(&self) -> PathBuf {
            self.0.join(GRIMOIRE_DIR).join(MAPS_DIR)
        }

        /// Bind a directory that already exists, so `Project::open` would load it.
        fn bind(&self, name: &str) {
            let path = self.0.join(GRIMOIRE_DIR).join(bevy_grimoire_core::kits::KITS_FILE);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
            let mut k = bevy_grimoire_core::kits::Kits::parse(&text).unwrap_or_else(|e| panic!("{e}"));
            k.bind.push(bevy_grimoire_core::kits::Bind {
                namespace: name.to_owned(),
                dir: name.to_owned(),
            });
            if k.authoring.is_none() {
                k.authoring = Some(name.to_owned());
            }
            std::fs::write(&path, k.to_yaml().unwrap_or_else(|e| panic!("{e}")))
                .unwrap_or_else(|e| panic!("{e}"));
        }

        /// **Declare a bash**, appended in call order — which is the order `B` walks them in.
        /// Every namespace named must already be bound, or `Kits::validate` refuses the write here
        /// rather than at the next scan.
        fn bash(&self, name: &str, kits: &[&str]) {
            let path = self.0.join(GRIMOIRE_DIR).join(bevy_grimoire_core::kits::KITS_FILE);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
            let mut k = bevy_grimoire_core::kits::Kits::parse(&text).unwrap_or_else(|e| panic!("{e}"));
            k.bash.push(bevy_grimoire_core::kits::Bash {
                name: name.to_owned(),
                kits: kits.iter().map(|s| (*s).to_owned()).collect(),
                note: None,
            });
            let out = k.to_yaml().unwrap_or_else(|e| panic!("{e}"));
            bevy_grimoire_core::kits::Kits::parse(&out).unwrap_or_else(|e| panic!("{e}"));
            std::fs::write(&path, out).unwrap_or_else(|e| panic!("{e}"));
        }

        /// A kit directory holding `n` descriptors. `None` writes into the root kit itself.
        fn kit(&self, name: Option<&str>, n: usize) -> PathBuf {
            let dir = match name {
                Some(k) => self.0.join(GRIMOIRE_DIR).join(k),
                None => self.0.join(GRIMOIRE_DIR),
            };
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            let ids: Vec<String> = (0..n)
                .map(|i| format!("{{id: \"p{i}\", mesh: \"p{i}.glb\"}}"))
                .collect();
            let text = format!("{{version: 1, descriptors: [{}]}}", ids.join(", "));
            std::fs::write(dir.join(LIBRARY_FILE), text)
                .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            if let Some(k) = name {
                self.bind(k);
            }
            dir
        }

        /// **A kit whose pieces carry a `ns/` namespace** — the shape every shipped kit has, and
        /// the only shape the deletion guard can reason about. [`Root::kit`] writes flat ids,
        /// which is the other case and the reason both helpers exist.
        ///
        /// Two calls with one `ns` make a re-skin pair, exactly as `site` and `site_greybox` are.
        fn skin(&self, name: &str, ns: &str, pieces: &[&str]) -> PathBuf {
            let dir = self.0.join(GRIMOIRE_DIR).join(name);
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            let ids: Vec<String> = pieces
                .iter()
                .map(|p| format!("{{id: \"{ns}/{p}\", mesh: \"{p}.glb\"}}"))
                .collect();
            let text = format!("{{version: 1, descriptors: [{}]}}", ids.join(", "));
            std::fs::write(dir.join(LIBRARY_FILE), text)
                .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            self.bind(name);
            dir
        }

        /// A map in the project, placing one piece per id named. Built as a `Map` and serialized
        /// rather than hand-written, so a schema change breaks this loudly instead of quietly
        /// producing a file `Map::parse` rejects for an unrelated reason.
        fn map(&self, _kit: &Path, name: &str, places: &[&str]) {
            let map = Map {
                name: name.to_owned(),
                placements: places
                    .iter()
                    .enumerate()
                    .map(|(i, d)| Placed {
                        id: format!("piece@{i}"),
                        descriptor: (*d).to_owned(),
                        ..Placed::default()
                    })
                    .collect(),
                ..Map::default()
            };
            map.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
            let text = bevy_grimoire_core::yaml::to_string(&map)
                .unwrap_or_else(|p| panic!("{name}: {}", p.line()));
            let dir = self.maps();
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            std::fs::write(dir.join(bevy_grimoire_core::naming::map_file_name(name)), text)
                .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        }

        /// **The game's own kit file**, under [`GAME_KIT_DIR`] and outside `assets/grimoire`
        /// entirely — the dependent no scan of the editor's content could ever have found.
        fn game_kit(&self, file: &str, names: &[&str]) {
            let dir = self.0.join(GAME_KIT_DIR);
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            let body: Vec<String> = names
                .iter()
                .enumerate()
                .map(|(i, id)| format!("slot{i}: \"{id}\""))
                .collect();
            std::fs::write(dir.join(file), format!("{}\n", body.join("\n")))
                .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// **The mirror must not be on the layer the offscreen camera renders.**
    ///
    /// It was, and the capture came back a flat `000000` with nothing in the log: the camera that
    /// renders into the image was drawing the sprite that *shows* that image — one texture as
    /// colour attachment and sampled source in a single pass. Nothing in Bevy says so out loud, so
    /// the only way this stays fixed is a test that names the two layers.
    #[test]
    fn the_mirror_is_not_on_the_layer_it_mirrors() {
        assert_ne!(
            crate::surface::MIRROR_LAYER, 0,
            "layer 0 is where the UI camera draws; a mirror there renders its own target"
        );
    }

    /// **A directory is a kit only if it holds a `library.yaml`** — the rule `Project::open` enforces
    /// before it accepts a `--kit`. Quoted rather than re-decided, so the chooser cannot offer a kit
    /// the editor would then refuse by name.
    #[test]
    fn a_directory_without_a_library_is_not_a_kit() {
        let root = Root::new("not-a-kit");
        root.kit(Some("real"), 3);
        let stray = root.0.join(GRIMOIRE_DIR).join("notes");
        std::fs::create_dir_all(&stray).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(stray.join("readme.txt"), "hello").unwrap_or_else(|e| panic!("{e}"));

        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let labels: Vec<&str> = catalog.kits.iter().map(|k| k.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["real"],
            "a directory with no library.yaml is not a kit"
        );
    }

    /// **The order never moves.** See the module note: Sears & Shneiderman's split-menu benefit
    /// grows with menu length and skew, and their own guideline caps the high-frequency zone at four
    /// items — which for a handful of kits is the whole list. This is what stops a future "sort by
    /// recently opened" landing without the argument being had.
    #[test]
    fn the_catalog_order_never_moves() {
        let root = Root::new("order");
        for k in ["site_v2", "site", "site_greybox"] {
            root.kit(Some(k), 1);
        }
        let _kit = root.kit(Some("site"), 1);
        for m in ["zulu", "alpha", "mike"] {
            create_map(&root.maps(), m, (4.0, 3.0, 4.0), (0.0, 0.0, 0.0), None)
                .unwrap_or_else(|e| panic!("{e}"));
        }

        let first = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let labels: Vec<&str> = first.kits.iter().map(|k| k.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["site", "site_greybox", "site_v2"],
            "kits are alphabetical"
        );

        let maps: Vec<&str> = first
            .kits
            .iter()
            .find(|k| k.label == "site")
            .map(|_| first.maps.iter().map(|m| m.name.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        assert_eq!(maps, vec!["alpha", "mike", "zulu"], "maps are alphabetical");

        // The property, stated as a property: scanning again returns the identical order. (The
        // `projects` list is *not* compared — it names sibling directories, which parallel tests
        // may create between the two scans. Order within each scan is what never moves.)
        let again = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let again_kits: Vec<&str> = again.kits.iter().map(|k| k.label.as_str()).collect();
        assert_eq!(
            labels, again_kits,
            "a second scan must return the identical kit order"
        );
        let again_maps: Vec<&str> = again.maps.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            maps, again_maps,
            "a second scan must return the identical map order"
        );
    }

    /// The root kit is `Project::open(kit: None)`, which a subdirectory scan cannot produce — so it
    /// is carried explicitly, with `flag: None`, or that mode becomes unreachable from the chooser.
    #[test]
    fn the_root_kit_is_offered_with_no_flag() {
        let root = Root::new("root-kit");
        root.kit(None, 7);
        root.kit(Some("site"), 2);

        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let rooted = catalog
            .kits
            .iter()
            .find(|k| k.flag.is_none())
            .unwrap_or_else(|| panic!("the root kit is missing: {:?}", catalog.kits));
        assert_eq!(rooted.label, "grimoire");
        assert_eq!(
            rooted.pieces, 7,
            "the piece count is the fact this screen exists to show"
        );
        assert_eq!(
            catalog.kits.iter().filter(|k| k.flag.is_some()).count(),
            1,
            "and the subdirectory kit is there too"
        );
    }

    /// A new map lands where its name says, parses back, and keeps the bounds it was given — the
    /// setting that was previously only reachable by editing `map::default_bounds` in source.
    #[test]
    fn a_new_map_lands_named_and_keeps_its_bounds() {
        let root = Root::new("new-map");
        let _kit = root.kit(Some("site"), 1);

        let path = create_map(&root.maps(), "Porch A", (12.0, 5.0, 9.0), (1.0, 0.0, 2.0), None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(
            path.ends_with("porch_a.map.yaml"),
            "forced to snake_case: {}",
            path.display()
        );

        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
        let map = Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(map.name, "porch_a");
        assert_eq!(map.bounds, (12.0, 5.0, 9.0));
        assert_eq!(map.origin, (1.0, 0.0, 2.0));

        // And the catalog sees it, with the numbers the row shows.
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let entry = catalog
            .kits
            .iter()
            .find(|k| k.label == "site")
            .and_then(|_| catalog.maps.iter().find(|m| m.name == "porch_a"))
            .unwrap_or_else(|| panic!("the new map is not in the catalog"));
        assert!(
            matches!(&entry.summary, MapSummary::Read { placements: 0, stamps: 0, bounds: (12.0, 5.0, 9.0), .. }),
            "{:?}",
            entry.summary
        );
    }

    /// **An unnamed map is refused by name, never substituted.** `Map::default()` leaves the name
    /// empty deliberately — *"a substituted name is a name nobody chose, and the second one collides
    /// with the first"* — and a chooser that filled in `untitled_map` would reintroduce exactly what
    /// `bevy_grimoire_core` refuses.
    #[test]
    fn an_unnamed_map_is_refused_rather_than_defaulted() {
        let root = Root::new("unnamed");
        let kit = root.kit(Some("site"), 1);

        for raw in ["", "   ", "!!!"] {
            let e = create_map(&root.maps(), raw, (4.0, 3.0, 4.0), (0.0, 0.0, 0.0), None)
                .err()
                .unwrap_or_else(|| panic!("`{raw}` must be refused, not written"));
            assert!(e.contains("snake_case"), "the refusal names the rule: {e}");
        }
        assert!(
            std::fs::read_dir(&kit)
                .map(|d| d
                    .flatten()
                    .all(|f| !f.file_name().to_string_lossy().ends_with(".map.yaml")))
                .unwrap_or(false),
            "a refused name must leave no file behind"
        );
    }

    /// Taking the same name twice is refused rather than silently overwriting somebody's map.
    #[test]
    fn a_name_already_taken_is_refused() {
        let root = Root::new("taken");
        let _kit = root.kit(Some("site"), 1);
        create_map(&root.maps(), "hall", (4.0, 3.0, 4.0), (0.0, 0.0, 0.0), None)
            .unwrap_or_else(|e| panic!("{e}"));
        let e = create_map(&root.maps(), "hall", (8.0, 3.0, 8.0), (0.0, 0.0, 0.0), None)
            .err()
            .unwrap_or_else(|| panic!("the second `hall` must be refused"));
        assert!(e.contains("already exists"), "{e}");
    }

    /// **A new kit is one the editor will actually open**, which is the only thing that makes it a
    /// kit: `Catalog::scan` and `Project::open` both test for `library.yaml`, so writing the
    /// directory without it would produce something the chooser lists and the editor then refuses.
    #[test]
    fn a_new_kit_is_one_the_scanner_accepts() {
        let root = Root::new("new-kit");
        root.kit(Some("site"), 3);

        let dir = create_kit(&root.0, "Site V3").unwrap_or_else(|e| panic!("{e}"));
        assert!(
            dir.ends_with("site_v3"),
            "forced to snake_case: {}",
            dir.display()
        );
        assert!(dir.join(LIBRARY_FILE).is_file(), "a kit is its library");
        assert!(
            dir.join(bevy_grimoire_core::policy::POLICY_FILE).is_file(),
            "and its policy — `Project::open` reads both"
        );
        // No compositions.yaml: absence and empty are the same state to `layered_library`, so a file
        // saying nothing would be a file that means nothing.
        assert!(!dir.join("compositions.yaml").exists());

        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let made = catalog
            .kits
            .iter()
            .find(|k| k.label == "site_v3")
            .unwrap_or_else(|| panic!("the new kit did not scan"));
        assert_eq!(
            made.flag.as_deref(),
            Some("site_v3"),
            "reachable as --kit site_v3"
        );
    }

    /// **A new project opens and bootstraps** — the plan's Phase C verification 7. The skeleton is
    /// `assets/grimoire/{kits.yaml,vocab.yaml}` and nothing else; the copied vocabulary is
    /// byte-identical to the source's; a fresh scan sees zero kits and zero maps; and the first
    /// kit made lands in the new project — which used to fail with "kits.yaml does not exist" until
    /// `bind_kit` learned to say what the directory is instead.
    #[test]
    fn a_new_project_opens_and_bootstraps() {
        let root = Root::new("new-project");
        root.kit(Some("site"), 1);
        // A vocabulary with a comment, to pin the byte-copy.
        let vocab = root.0.join(GRIMOIRE_DIR).join("vocab.yaml");
        std::fs::write(
            &vocab,
            "# a comment that must survive\nkind: {tokens: []}\neffects: {tokens: []}\nlook: {tokens: []}\nsurfaces: {tokens: []}\n",
        )
        .unwrap_or_else(|e| panic!("{}: {e}", vocab.display()));

        // **The project is made INSIDE this test's own directory**, not beside it.
        //
        // `create_project` makes a *sibling* of the directory it is given, so passing the temp
        // root's parent put the project at `<temp>/porch_kit` — **a fixed path in a directory every
        // test in every binary shares**. Two tests running at once, or the same test in two runs,
        // then race on it: and the line that removed "a sibling the previous run left behind"
        // removed the directory the *other* run was writing.
        //
        // **The project's own name is unchanged** — `Porch Kit`, forced to `porch_kit` — and the
        // assertion below still checks exactly that. **Only where it is made has changed**, and
        // that is the whole of the fix: the directory is now the test's own, so nothing else can
        // be inside it and nothing else can delete it.
        let _ = std::fs::remove_dir_all(root.0.join("porch_kit"));
        let dir = create_project(&root.0, "Porch Kit", &vocab).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            dir.ends_with("porch_kit"),
            "forced to snake_case: {}",
            dir.display()
        );
        let grimoire = dir.join(GRIMOIRE_DIR);
        assert!(grimoire.join(bevy_grimoire_core::kits::KITS_FILE).is_file(), "kits.yaml");
        assert!(grimoire.join("vocab.yaml").is_file(), "vocab.yaml");
        assert!(
            std::fs::read(grimoire.join("vocab.yaml")).unwrap_or_else(|e| panic!("{e}"))
                == std::fs::read(&vocab).unwrap_or_else(|e| panic!("{e}")),
            "the copied vocabulary is byte-identical — comments included"
        );
        // Nothing else: no compositions.yaml, no maps/, no kit.
        assert!(!grimoire.join("compositions.yaml").exists());
        assert!(!grimoire.join("maps").exists());

        let catalog = Catalog::scan(&dir).unwrap_or_else(|e| panic!("{e}"));
        assert!(catalog.kits.is_empty(), "no kits yet");
        assert!(catalog.maps.is_empty(), "no maps yet");
        assert!(catalog.authoring.is_none(), "no authoring kit yet");

        // **The first kit made is the authoring kit.** This call failed before C2 — the new
        // project's `kits.yaml` was not there to bind into, and `bind_kit` said so with a raw io
        // error instead of an instruction.
        let kit_dir = create_kit(&dir, "props").unwrap_or_else(|e| panic!("{e}"));
        assert!(kit_dir.starts_with(&dir), "the kit is inside the project");
        let catalog = Catalog::scan(&dir).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            catalog.authoring.as_deref(),
            Some("props"),
            "bind_kit adopts the first kit"
        );
    }

    /// **Names cannot escape the parent** — the plan's Phase C verification 8. `to_snake_case`
    /// strips separators and dots, so every hostile name either refuses or lands strictly inside
    /// the parent; the returned path must start with the parent either way. This pins existing
    /// behaviour rather than adding a guard.
    #[test]
    fn a_project_name_cannot_escape_the_parent() {
        let root = Root::new("escape");
        let vocab = root.0.join(GRIMOIRE_DIR).join("vocab.yaml");
        std::fs::write(&vocab, "{}").unwrap_or_else(|e| panic!("{e}"));
        let parent = root.0.parent().unwrap_or_else(|| panic!("temp dir has a parent"));

        for hostile in ["../evil", "foo/bar", "..", "a/../../b"] {
            match create_project(parent, hostile, &vocab) {
                Err(e) => assert!(
                    e.contains("leaves nothing usable") || e.contains("already exists"),
                    "{hostile:?}: refused with a reason: {e}"
                ),
                Ok(dir) => {
                    assert!(
                        dir.starts_with(parent),
                        "{hostile:?} landed outside the parent: {}",
                        dir.display()
                    );
                    assert!(
                        !dir.to_string_lossy().contains(".."),
                        "{hostile:?} escaped: {}",
                        dir.display()
                    );
                }
            }
        }
    }

    /// **`set_authoring` re-points `kits.yaml`'s `authoring`, and refuses a name that is not bound.**
    ///
    /// **`set_authoring` re-points `kits.yaml`'s `authoring`, and refuses a name that is not bound.**
    ///
    /// The refusal names the bound kits, because "not bound" without a list is a search. The
    /// round-trip is through `Catalog::scan`, which is what the screen reads — so the row an author
    /// sees is the file an author wrote.
    #[test]
    fn authoring_round_trips_and_refuses_unbound_names() {
        let root = Root::new("authoring");
        root.kit(Some("a"), 1);
        root.kit(Some("b"), 1);

        set_authoring(&root.0, "b").unwrap_or_else(|e| panic!("{e}"));
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            catalog.authoring.as_deref(),
            Some("b"),
            "the scan reads what the write wrote"
        );

        let err = set_authoring(&root.0, "nope")
            .err()
            .unwrap_or_else(|| panic!("an unbound name must be refused"));
        assert!(err.contains("`nope`"), "names the offender: {err}");
        assert!(err.contains("`a`") && err.contains("`b`"), "and the bound kits: {err}");
        // The refusal wrote nothing.
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(catalog.authoring.as_deref(), Some("b"));
    }

    /// An unusable name is refused by name, and a taken one is refused rather than merged into
    /// somebody else's kit.
    #[test]
    fn a_kit_name_that_cannot_work_is_refused() {
        let root = Root::new("kit-names");
        root.kit(Some("site"), 1);

        for raw in ["", "   ", "!!!"] {
            let e = create_kit(&root.0, raw)
                .err()
                .unwrap_or_else(|| panic!("`{raw}` must be refused"));
            assert!(e.contains("snake_case"), "the refusal names the rule: {e}");
        }
        let e = create_kit(&root.0, "site")
            .err()
            .unwrap_or_else(|| panic!("a taken name must be refused"));
        assert!(e.contains("already exists"), "{e}");
        // And the kit that was there is untouched.
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            catalog
                .kits
                .iter()
                .find(|k| k.label == "site")
                .map(|k| k.pieces),
            Some(1),
            "refusing must not have emptied the existing kit"
        );
    }

    /// **Three names, then a count.** Listing all 51 dependents would be a screen of paths where
    /// the point is that there are 51 of them — and a refusal is read while reaching for the next
    /// key, which is the register the root-kit guard was cut down to.
    #[test]
    fn the_refusal_names_a_few_and_counts_the_rest() {
        let many: Vec<String> = (0..5).map(|i| format!("f{i}.map.yaml")).collect();
        let msg = strands("site", &many).unwrap_or_else(|| panic!("five dependents is a refusal"));
        assert!(msg.contains("5 files"), "{msg}");
        assert!(msg.contains("f0.map.yaml") && msg.contains("f2.map.yaml"), "{msg}");
        assert!(!msg.contains("f3.map.yaml"), "the fourth is counted, not named: {msg}");
        assert!(msg.contains("and 2 more"), "{msg}");

        // One reads as one, and none is not a refusal at all.
        let one = strands("site", &["a.map.yaml".to_owned()])
            .unwrap_or_else(|| panic!("one dependent is still a refusal"));
        assert!(one.contains("1 file still name"), "{one}");
        assert_eq!(strands("site", &[]), None, "nothing stranded is not a refusal");
    }

    /// An empty kit is a kit with no maps, not an error — and the screen turns that into an
    /// instruction rather than a report (`docs/ui.md` §1.4).
    #[test]
    fn an_empty_kit_reports_no_maps_rather_than_failing() {
        let root = Root::new("empty");
        root.kit(Some("site_v2"), 0);
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let kit = catalog
            .kits
            .iter()
            .find(|k| k.label == "site_v2")
            .unwrap_or_else(|| panic!("missing"));
        assert_eq!(kit.pieces, 0);
        assert!(catalog.maps.is_empty(), "and it brought no maps with it");
    }

    /// **A map that will not parse is a row, not an omission.** Dropping it would present a broken
    /// project as an empty one, and the author would go looking for a map the list had quietly eaten.
    #[test]
    fn an_unreadable_map_is_listed_with_its_reason() {
        let root = Root::new("broken");
        root.kit(Some("site"), 1);
        let maps = root.maps();
        std::fs::create_dir_all(&maps).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(maps.join("broken.map.yaml"), "(this is not a map)")
            .unwrap_or_else(|e| panic!("{e}"));

        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let entry = catalog
            .kits
            .iter()
            .find(|k| k.label == "site")
            .and_then(|_| catalog.maps.iter().find(|m| m.name == "broken"))
            .unwrap_or_else(|| panic!("the broken map was dropped from the list"));
        assert!(
            matches!(entry.summary, MapSummary::Unreadable(_)),
            "it must carry its reason: {:?}",
            entry.summary
        );
    }

    /// A root with no `assets/grimoire` is refused by name — the message tells the author what to do,
    /// per §1.4, rather than reporting an empty list that looks like a project with no kits.
    #[test]
    fn a_root_that_is_not_a_project_says_so() {
        let dir = std::env::temp_dir().join(format!("chooser-bare-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        let e = Catalog::scan(&dir)
            .err()
            .unwrap_or_else(|| panic!("must refuse"));
        assert!(e.contains("is not a project"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asking_to_delete_a_map_deletes_nothing_until_it_is_agreed_to() {
        let root = Root::new("ask-delete");
        let _kit = root.kit(Some("site"), 1);
        let path = create_map(&root.maps(), "hall", (4.0, 3.0, 4.0), (0.0, 0.0, 0.0), None).unwrap_or_else(|e| panic!("{e}"));
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));

        let ask = ask_delete_map(&catalog, "hall").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((ask.name.as_str(), ask.kit), ("hall", false));
        assert!(path.is_file(), "and the file is UNTOUCHED until it is answered");

        delete_entry(&root.0, &ask).unwrap_or_else(|e| panic!("{e}"));
        assert!(!path.exists(), "answering yes removes it");
        assert!(ask_delete_map(&catalog, "nothing").is_err(), "a map that is not there is not asked about");
    }

    #[test]
    fn the_question_deletes_what_it_named_even_if_the_list_moves() {
        let root = Root::new("moving-target");
        root.kit(Some("site"), 1);
        for m in ["alpha", "beta"] {
            create_map(&root.maps(), m, (4.0, 3.0, 4.0), (0.0, 0.0, 0.0), None).unwrap_or_else(|e| panic!("{e}"));
        }
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let ask = ask_delete_map(&catalog, "alpha").unwrap_or_else(|e| panic!("{e}"));
        // A new map lands first in the list, so a row index would now name another map.
        create_map(&root.maps(), "aardvark", (4.0, 3.0, 4.0), (0.0, 0.0, 0.0), None).unwrap_or_else(|e| panic!("{e}"));
        delete_entry(&root.0, &ask).unwrap_or_else(|e| panic!("{e}"));
        assert!(!root.maps().join("alpha.map.yaml").exists(), "the question named alpha, so alpha is what goes");
        assert!(root.maps().join("beta.map.yaml").is_file() && root.maps().join("aardvark.map.yaml").is_file(), "nothing else was in question");
    }

    #[test]
    fn the_default_kit_cannot_be_deleted() {
        let root = Root::new("root-kit-safe");
        root.kit(None, 3);
        root.kit(Some("site"), 1);
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let root_kit = catalog.kits.iter().find(|k| k.flag.is_none()).map(|k| k.label.clone()).unwrap_or_else(|| panic!("the root kit is always listed"));
        let e = ask_delete_kit(&root.0, &catalog, &root_kit).err().unwrap_or_else(|| panic!("must refuse the default kit"));
        assert!(e.contains("default kit"), "{e}");
    }

    #[test]
    fn a_kit_something_else_still_names_is_refused() {
        let root = Root::new("delete-strands");
        root.kit(None, 1);
        root.skin("site", "site", &["floor", "wall"]);
        let lab = root.skin("lab", "lab", &["bench"]);
        // The *lab's* map reaches across for a site piece — which is the whole point of making kits shareable, and the whole reason
        // deleting one stops being a local act.
        root.map(&lab, "lab_a", &["lab/bench", "site/floor"]);
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let e = ask_delete_kit(&root.0, &catalog, "site").err().unwrap_or_else(|| panic!("must refuse a kit whose pieces are still named"));
        assert!(e.contains("assets/grimoire/maps/lab_a.map.yaml"), "the refusal names the file, because \"something uses it\" is not information: {e}");
    }

    #[test]
    fn a_kit_another_one_re_skins_can_still_go() {
        let root = Root::new("delete-skin");
        root.kit(None, 1);
        root.skin("site", "site", &["floor", "wall"]);
        root.skin("site_greybox", "site", &["floor", "wall"]);
        let lab = root.skin("lab", "lab", &["bench"]);
        root.map(&lab, "lab_a", &["site/floor"]);
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        ask_delete_kit(&root.0, &catalog, "site").unwrap_or_else(|e| panic!("a skin with a twin strands nothing: {e}"));
    }

    #[test]
    fn the_game_kit_file_is_a_dependent_no_content_scan_would_find() {
        let root = Root::new("delete-game");
        root.kit(None, 1);
        root.skin("site", "site", &["floor"]);
        // Deliberately nothing under `assets/grimoire` refers to it. The editor's own world is clean.
        root.game_kit("kit_ozea.yaml", &["site/floor"]);
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let e = ask_delete_kit(&root.0, &catalog, "site").err().unwrap_or_else(|| panic!("the game is a dependent like any other"));
        assert!(e.contains("assets/site/kit_ozea.yaml"), "{e}");
    }

    #[test]
    fn deleting_a_kit_takes_the_directory_with_it_and_its_binding() {
        let root = Root::new("kit-delete");
        root.kit(None, 2);
        // `site` first, so it is the kit new work lands in; the one deleted is another.
        root.kit(Some("site"), 1);
        let doomed = root.kit(Some("scratch"), 1);
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let ask = ask_delete_kit(&root.0, &catalog, "scratch").unwrap_or_else(|e| panic!("{e}"));
        assert!(doomed.exists(), "asking removes nothing");
        delete_entry(&root.0, &ask).unwrap_or_else(|e| panic!("{e}"));
        assert!(!doomed.exists(), "the whole kit directory goes");
        let after = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        assert!(after.kit("scratch").is_none(), "and it is off the list");
        assert!(after.kit("site").is_some(), "while the others stand");
    }

    #[test]
    fn cycling_a_bash_writes_the_file_and_rounds_to_every_kit() {
        let root = Root::new("bash-cycle");
        root.skin("furniture", "furniture", &["bench"]);
        root.skin("lab", "lab", &["desk"]);
        root.bash("hub", &["furniture", "lab"]);
        root.bash("props", &["furniture"]);
        root.map(&root.0, "hall", &[]);
        let path = root.maps().join("hall.map.yaml");
        let on_disk = |path: &Path| Map::parse(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|e| panic!("{e}")).bash;
        assert_eq!(on_disk(&path), None, "a map starts offering every kit");

        // **File order, not alphabetical** — `hub` is declared first, so it is what the first press names.
        for want in [Some("hub"), Some("props"), None] {
            let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
            let landed = cycle_bash(&catalog, "hall").unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(landed.as_deref(), want);
            assert_eq!(on_disk(&path).as_deref(), want, "the file carries it");
            let again = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
            assert!(matches!(&again.map("hall").map(|m| &m.summary), Some(MapSummary::Read { bash, .. }) if bash.as_deref() == want), "and so does the scan");
        }
    }

    #[test]
    fn cycling_with_no_bashes_declared_says_where_to_declare_one() {
        let root = Root::new("bash-none");
        root.skin("furniture", "furniture", &["bench"]);
        root.map(&root.0, "hall", &[]);
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let e = cycle_bash(&catalog, "nowhere").err().unwrap_or_else(|| panic!("a map that is not there has nothing to give a bash to"));
        assert!(e.contains("no map here"), "{e}");
        let e = cycle_bash(&catalog, "hall").err().unwrap_or_else(|| panic!("a project declaring none must refuse"));
        assert!(e.contains("declares no bashes") && e.contains("kits.yaml"), "{e}");
        let map = Map::parse(&std::fs::read_to_string(root.maps().join("hall.map.yaml")).unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(map.bash, None, "and nothing was written");
    }

    #[test]
    fn counts_agree_with_their_nouns_and_a_size_reads_in_metres() {
        assert_eq!(plural(0, "map"), "maps", "zero is plural");
        assert_eq!(plural(1, "piece"), "piece");
        assert_eq!(plural(2, "piece"), "pieces");
        assert_eq!(size_in_metres((10.0, 4.0, 8.0)), "10 × 8 m, 4 m tall");
    }

    #[test]
    fn making_a_project_here_keeps_what_the_folder_held_and_refuses_one_that_is_already_a_project() {
        let dir = Root::new("project-here");
        // `Root` is already a project; this is about a folder that is not one yet.
        std::fs::remove_dir_all(dir.0.join(GRIMOIRE_DIR)).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.0.join("notes.txt"), "mine").unwrap_or_else(|e| panic!("{e}"));
        make_project_here(&dir.0, b"(vocab)").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(std::fs::read_to_string(dir.0.join("notes.txt")).unwrap_or_default(), "mine");
        let catalog = Catalog::scan(&dir.0).unwrap_or_else(|e| panic!("it scans as a project: {e}"));
        assert!(catalog.kits.is_empty() && catalog.maps.is_empty());
        let e = make_project_here(&dir.0, b"(vocab)").err().unwrap_or_else(|| panic!("a project is not made twice"));
        assert!(e.contains("already a project"), "{e}");
        let e = make_project_here(&dir.0.join("nowhere"), b"").err().unwrap_or_else(|| panic!("a path that is not a folder"));
        assert!(e.contains("not a folder"), "{e}");
    }

    /// **A kit's policy lists its entries in plain words and removes one, keeping the file's comments.** Exclusions come first and a
    /// patch is keyed by its place, so two patches sharing a key are still two rows.
    #[test]
    fn a_kits_policy_lists_and_removes_an_entry() {
        let root = Root::new("policy-rows");
        root.kit(Some("site"), 1);
        let path = root.0.join(GRIMOIRE_DIR).join("site").join(bevy_grimoire_core::policy::POLICY_FILE);
        std::fs::write(
            &path,
            "version: 2\n# A comment that must survive.\nnote: a kit with policy\nexclude: [characters]\npatches:\n- match: !Id wall\n  because: this game's walls are 2.4 m\n  patch:\n    align:\n      stretch_y: 1.2\n",
        )
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let catalog = Catalog::scan(&root.0).unwrap_or_else(|e| panic!("{e}"));
        let kit = catalog.kit("site").unwrap_or_else(|| panic!("the kit is listed"));

        let rows = policy_entries(kit).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0].row, PolicyRow::Exclude("characters".to_owned()));
        assert!(rows[0].shown.contains("characters"), "{}", rows[0].shown);
        assert_eq!(rows[1].row, PolicyRow::Patch(0));
        assert!(rows[1].shown.contains("wall") && rows[1].shown.contains("2.4 m"), "{}", rows[1].shown);

        remove_policy_entry(kit, &rows[0].row).unwrap_or_else(|e| panic!("{e}"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
        assert!(text.contains("# A comment that must survive"), "the comment stands:\n{text}");
        let left = policy_entries(kit).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(left.iter().map(|r| r.row.clone()).collect::<Vec<_>>(), vec![PolicyRow::Patch(0)], "only the exclusion went");

        remove_policy_entry(kit, &PolicyRow::Patch(0)).unwrap_or_else(|e| panic!("{e}"));
        assert!(policy_entries(kit).unwrap_or_else(|e| panic!("{e}")).is_empty(), "and then the patch");
    }
}
