//! **Opening a project** — the vocabulary, the library, and the map being edited.
//!
//! A project is a directory with an `assets/` folder in it. `assets/grimoire/vocab.ron` says what
//! tokens exist, `assets/grimoire/library.ron` says what can be placed, and a descriptor's `mesh` path
//! resolves under `assets/` — which is also Bevy's asset root, so the editor and the game name the
//! same file the same way. Nothing here knows about any particular game.
//!
//! # It refuses to open rather than opening empty
//!
//! Every load failure is fatal and says which file and why. That is the same call
//! `site_editor::source_map` makes — *"a tool that writes level data while unsure which line it is
//! writing is worse than no tool"* — and the failure mode it avoids is the one that wastes an
//! afternoon: an editor that comes up with an empty palette looks exactly like an editor whose
//! project has no assets.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{Map, MAP_VERSION};
use bevy_grimoire_core::naming;
use bevy_grimoire_core::vocab::{Masks, Vocabularies};

/// Where a project's own files live, under its root. Inside `assets/` rather than beside it, so a
/// descriptor's `mesh` path means the same thing to this editor and to the game that loads the map.
const GRIMOIRE_DIR: &str = "assets/grimoire";
/// **Where the vocabulary lives** — public because the file watcher names it.
pub const VOCAB: &str = "assets/grimoire/vocab.ron";
/// **The library file's name** — public because the file watcher names it.
pub const LIBRARY_FILE: &str = "library.ron";
/// **Where maps live** — the project's own directory, not a kit's.
///
/// They sat beside the kit that drew them, on the argument that *"a map means nothing without the
/// library that draws it"*. That was true while a project was one kit. It stopped being true the
/// moment a map could stamp a tile seating two kits' pieces: the library that draws a map is now the
/// **merge**, so filing it under one of the kits picks an arbitrary winner.
///
/// A subdirectory rather than the project root, and that is load-bearing: `.gitignore` carries
/// `assets/grimoire/*.map.ron` to keep stray test maps out of git, and a map in `maps/` is not matched
/// by it.
const MAPS_DIR: &str = "maps";

/// **A value a [`Project`] clone shares instead of copies.**
///
/// The operation context owns a `Project`, so every click that dispatches an operation clones it —
/// and the library, its measurements, the kit layers and the masks are the heavy part of it. Behind
/// an `Arc`, a clone is a few reference-count bumps. Reads go through [`Deref`](std::ops::Deref)
/// unchanged; a write goes through [`DerefMut`](std::ops::DerefMut), which copies the value first
/// only when a clone is still holding it (`Arc::make_mut`), so an operation's scratch copy can never
/// change the live project and the live project pays for the copy only when it writes.
#[derive(Debug, PartialEq, Default)]
pub struct Shared<T>(std::sync::Arc<T>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(std::sync::Arc::clone(&self.0))
    }
}

impl<T> From<T> for Shared<T> {
    fn from(value: T) -> Self {
        Shared(std::sync::Arc::new(value))
    }
}

impl<T> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> std::ops::DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut T {
        std::sync::Arc::make_mut(&mut self.0)
    }
}

impl<T> Shared<T> {
    /// Whether `self` and `other` are the same allocation — what a clone preserves.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

/// The opened project.
/// **Cloned, because the operation context owns what it carries.** The registry's operations are
/// a `dyn`, and a `dyn` cannot have a lifetime on its methods — so the context holds the project
/// and the map by value. One copy of a project's settings is a price worth paying for a registry
/// that can hold more than one operation.
#[derive(Resource, Clone)]
pub struct Project {
    pub root: PathBuf,
    /// **The kit's directory** — `assets/grimoire`, or a named subdirectory of it.
    ///
    /// A *kit* is a library and its policy layer: `assets/grimoire/site/` holds the 45 architectural
    /// pieces (wall, corner, doorway, header, column, pipe) with their own patches sizing walls to a
    /// 2.40 m facility, and the default directory holds furniture. `policy::layered_library` already
    /// reads exactly one directory, so a kit is a path and nothing else.
    ///
    /// **The vocabulary is not per-kit** and stays at the root: tokens are what this *project* means,
    /// and a kit that could redefine them would be a second vocabulary to keep in step.
    ///
    /// Resolved once, here, so nothing downstream carries an `Option` or rebuilds this path. Maps are
    /// written beside their kit, because a map means nothing without the library that draws it.
    pub grimoire_dir: PathBuf,
    /// **The namespace this kit implements** — the `site` in `site/wall_corner`.
    ///
    /// Not the same question as [`Self::grimoire_dir`], and that is the whole point: `assets/grimoire/site/`
    /// and `assets/grimoire/site_greybox/` are two directories providing the **identical** 45 `site/*`
    /// ids, so a namespace is an *interface* and a directory is one *implementation* of it. A tile
    /// authored in either belongs to `site`, because that is what a map naming `site/floor` binds to.
    ///
    /// Read from the library when its ids carry a namespace, and from the directory name when they do
    /// not. Resolved in [`Self::open`], which is where the reasoning and the refusals live.
    pub namespace: String,
    /// Where [`Self::measured`] came from and where it is written back.
    pub library_path: PathBuf,
    pub vocab: Vocabularies,
    /// **`library.ron` as it sits on disk** — the measurements, with no policy layered on.
    ///
    /// **Every edit lands here and this is what gets written.** `write_library` used to serialize
    /// [`Self::library`] back over `library_path`, which meant toggling one lattice cell under
    /// `--kit site` wrote SCP-9191's stretched 2.40 m wall heights into the measurements file the kit
    /// exists to share — and the next load applied the patches again on top of them. A library is
    /// measurements; the architecture is `project.ron`'s, and it must not leak downward.
    pub measured: Shared<Library>,
    /// **Every composition this project can stamp**, validated against [`Self::library`] at open.
    ///
    /// Read from `compositions.ron` beside the library, or empty when the project has none — which is
    /// the same state as a file with none in it, not a degraded one.
    pub compositions: bevy_grimoire_core::composition::Compositions,
    /// The measurements with [`Self::policy`] applied — what the game would place, and what every
    /// reader here (the palette, the preview, the flood fill, the fault check) uses.
    ///
    /// Derived, never written. Rebuilt from `measured` after every edit by `write_library`.
    pub library: Shared<Library>,
    /// **The authoring kit's** policy — the patches and the exclusions it applies to its own pieces.
    ///
    /// Per-kit and not merged, because `Policy::apply` refuses a rule that matches nothing: a kit's
    /// patches name a kit's pieces, so layering one project-wide would refuse to open the moment two
    /// kits were bound.
    pub policy: bevy_grimoire_core::policy::Policy,
    /// **The grid every kit, tile and map in this project shares** — `kits.ron`'s
    /// [`bevy_grimoire_core::kits::Lattice`].
    ///
    /// Held here rather than on the map, which is where it lived for part of
    /// 2026-08-16. A map has exactly one lattice, but so does the project, and
    /// `composition::interface` takes the band count as an argument — so two maps at different
    /// counts gave the same tile two different adjacency contracts. It is also the only form the
    /// doors that author tiles can ask, since they have no map open at all.
    pub lattice: bevy_grimoire_core::kits::Lattice,
    /// **Every bound kit, layered**, in `kits.ron` order.
    ///
    /// Held so an edit to the authoring kit can rebuild [`Self::library`] without re-reading the
    /// other kits off disk — `commit_measured` validates before it writes, so the file it would
    /// re-read is still the old one at the moment the merge is needed.
    pub kits: Shared<Vec<bevy_grimoire_core::kits::KitLayer>>,
    /// **The named combinations a map may draw on** — `kits.ron`'s `bash`, in file order.
    ///
    /// Held beside [`Self::kits`] because it is read at the same two moments: `OpenMap::open`
    /// refuses a map naming one this project does not declare, and `palette_namespaces` resolves
    /// the name a map does carry. A bash names namespaces, and `Kits::validate` has already refused
    /// one naming a namespace nothing binds — so every name in here is loaded.
    pub bashes: Vec<bevy_grimoire_core::kits::Bash>,
    /// Per-descriptor token masks, in library order — resolved once at load so the palette and the
    /// placement rules never re-resolve the same strings.
    pub masks: Shared<Vec<Masks>>,
    /// **The project's own directory** — `assets/grimoire`, the one holding `vocab.ron`, `kits.ron`
    /// and `compositions.ron`. Not a kit; kits are its subdirectories.
    pub project_dir: PathBuf,
    /// **The block rules the project states** — `rules.ron` beside `kits.ron`, validated against
    /// the merged [`Self::library`] at open. Empty (no blocks) when the file is absent, which is the
    /// same state as a file that states none: a project without rules behaves as it always did.
    ///
    /// Held so `G` can lay a block the way the game will (`editor::generate_from`), instead of
    /// learning from the whole map at the editor's own 1 m step.
    pub rules: bevy_grimoire_core::rules::Rules,
    /// Where every map in this project lives — see [`MAPS_DIR`]. Held rather than re-derived, so
    /// a rename follows the name without re-deciding which directory a map belongs to.
    pub maps_dir: PathBuf,
    /// **The map that was open**, by name — the last one this project had open.
    ///
    /// **The Kit door has no map, and the crossing needs to know which one to come back to.**
    /// Without this, crossing from the Kit door to the Map door has nothing to open — and the
    /// refusal it gives is *true* and useless: the author was on a map a moment ago.
    ///
    /// So the name is **carried across the crossing**, the way the armed composition is: it is the
    /// author's intent about which map they are working on, and it outlives the door it was opened
    /// in. It is a name and not a path, so a map that has been renamed since is refused by name.
    pub open_map_name: Option<String>,
    /// **Descriptor ids whose resolved form changed since the Map last redrew**, and which therefore
    /// have placements standing on screen built from an older shape.
    ///
    /// Written at the one commit door (`tiles::commit_measured`) by **comparing** the rebuilt library
    /// against the one it replaces — never declared by a call site. Fifteen edit paths reach that
    /// door and a list each of them had to remember to append to is a list that goes stale on the
    /// sixteenth; a diff cannot miss one.
    ///
    /// Drained by `editor::redraw_edited`. Empty is the normal state, and checking it is one
    /// `is_empty` per frame.
    pub touched: Vec<String>,
    /// Triangles per library entry, in library order.
    ///
    /// Measured at open from each GLB's JSON chunk — accessor counts only, no vertex data — rather
    /// than stored in the descriptor. Triangle count is a fact about the *file*, so keeping it in the
    /// schema would mean a number that silently goes stale the first time an artist re-exports. This
    /// costs a few milliseconds and cannot be wrong.
    pub triangles: Shared<Vec<usize>>,
}

impl Project {
    /// Read a project, or say exactly what is wrong with it.
    pub fn open(root: &Path, kit: Option<&str>) -> Result<Project, String> {
        // **The binding is the project's, not a kit's.** `kits.ron` says which directory provides
        // which namespace — `site/` and `site_greybox/` both provide `site/*`, so this is a question
        // no directory can answer about itself. See `bevy_grimoire_core::kits`.
        let project_dir = root.join(GRIMOIRE_DIR);
        let kits_path = project_dir.join(bevy_grimoire_core::kits::KITS_FILE);
        let kits = bevy_grimoire_core::kits::Kits::parse(&read(&kits_path)?)
            .map_err(|e| format!("{}: {e}", kits_path.display()))?;

        // **`--kit` chooses where new work lands, not what the palette can show.**
        //
        // It used to select the *only* kit loaded, which is what made a tile authored in one kit
        // invisible to every map in another. Every bound kit is loaded now — that is the whole
        // feature — so this answers a smaller and more useful question: which kit an imported mesh
        // joins, and which namespace a new tile is named in.
        //
        // Forced into a plain directory name the same way the map name is, so `--kit ../../etc`
        // cannot walk out of the project.
        let authoring = match kit {
            Some(k) => {
                let forced = naming::to_snake_case(k);
                if forced.is_empty() {
                    return Err(format!(
                        "`{k}` leaves nothing usable as a kit name. A kit is a directory under \
                         `{GRIMOIRE_DIR}` — snake_case, like `site`."
                    ));
                }
                if !kits.bind.iter().any(|b| b.dir == forced) {
                    return Err(format!(
                        "no kit `{forced}` in this project. {} binds {}. Add it there, or open one \
                         of those.",
                        kits_path.display(),
                        kits.bind
                            .iter()
                            .map(|b| format!("`{}`", b.dir))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                forced
            }
            None => kits.authoring.clone().ok_or_else(|| {
                format!(
                    "{} binds no kit, so there is nothing to open. Make one from the menu — the \
                     `+ new kit` row — and it becomes where new work lands.",
                    kits_path.display()
                )
            })?,
        };

        let vocab_path = root.join(VOCAB);
        let vocab = Vocabularies::parse(&read(&vocab_path)?)
            .map_err(|e| format!("{}: {e}", vocab_path.display()))?;

        // Every bound kit, each layered with its **own** policy, merged into the one library a map
        // resolves against — plus the project's compositions, validated against that merge because
        // it is the only library that can answer for a tile seating two kits' pieces.
        let bound = bevy_grimoire_core::kits::bound_library(&project_dir, &kits)?;
        let bevy_grimoire_core::kits::Bound {
            kits: layers,
            mut library,
            compositions,
        } = bound;

        // **The derived half of `effects` is settled over the whole set, at open.**
        //
        // `implies` on a `kind` token (`vocab.ron`, resolved by `Vocabularies::implied_effects`)
        // says these tokens follow the kind and are not hand-authorable —
        // *"set the kind and the effect follows, which is what makes them trustworthy to read"*. That
        // was true of every row written after the rule landed and false of every row written before
        // it, because the only two places that settled were applying a suggestion and clicking a KIND
        // chip. Neither reaches a row already on disk, so a library carried both kinds of answer at
        // once: `lamp` had `["emit", "uses-electricity"]` and `desk_lamp_modern`, same kind, had
        // `["emit"]`. Reported at the keyboard 2026-08-18 as beds arriving without
        // `stamina-recharge`.
        //
        // Here rather than at the write, on the same argument `library.resolve` below makes: the
        // moment to reconcile the whole set is when the whole set is in hand.
        //
        // **This corrects the derived view, and only the derived view.** `measured` and each kit's
        // layer keep the lists as authored, so every rebuild of the merge starts from the unsettled
        // rows again — which is why `tiles::commit_measured` settles too, through the same
        // `settle_library`. It is not a write-back: the disk copy is corrected a row at a time, as
        // rows are edited, by the settle inside `labels::apply_fields` and the KIND chip.
        let settled = crate::labels::settle_library(&mut library, &vocab);
        if settled > 0 {
            info!("{settled} library row(s) had derived effects out of step with their kind");
        }

        // Present by construction: `Kits::validate` refuses an `authoring` that names no bind, and
        // the check above refuses a `--kit` that names none either.
        let layer = layers
            .iter()
            .find(|l| l.dir.file_name().is_some_and(|n| n == authoring.as_str()))
            .ok_or_else(|| {
                format!(
                    "{}: `{authoring}` is bound but was not loaded, which cannot happen — the \
                     binding and the loader disagree.",
                    kits_path.display()
                )
            })?;
        let grimoire_dir = layer.dir.clone();
        let measured = layer.measured.clone();
        let policy = layer.policy.clone();
        // **The namespace comes from the binding, which was verified against the library.**
        // `Library::namespace` is what does the verifying (`kits::bound_library`), so this is the
        // one answer rather than a second derivation of it — and it is the only form that can name
        // `site` for a directory called `site_greybox`.
        let namespace = layer.namespace.clone();
        let library_path = grimoire_dir.join(LIBRARY_FILE);

        // The two-sided pass over the whole set, at open. A prop that rests on a class nothing offers
        // can never be placed, and the moment to say so is now — not when an author wonders why a
        // shelf stays empty.
        let masks = library
            .resolve(&vocab)
            .map_err(|e| format!("{}: {e}", library_path.display()))?;

        // **And the holes, in the same breath.** A slot's `accepts` token is checked here rather than
        // in `composition::validate` because that runs inside the project loader, which reads the
        // library and the policy and never opens the vocabulary — so this is the first place both
        // halves exist at once. Same rule as every other token: an invented one is refused at open,
        // naming the composition, the member and the axis.
        vocab
            .check_slots(&compositions.compositions)
            .map_err(|e| {
                format!(
                    "{}: {e}",
                    project_dir
                        .join(bevy_grimoire_core::composition::Compositions::FILE)
                        .display()
                )
            })?;

        // **Rules are read last of the project files, against the merged library**, because the
        // descriptors they name (the border, a footprint's nine parts, a region marker) may live in
        // any bound kit. An absent file is no rules; a present one that does not parse or names
        // something the library lacks fails the open, like every other project file.
        let rules_path = project_dir.join(bevy_grimoire_core::rules::FILE);
        let rules = if rules_path.exists() {
            let parsed = bevy_grimoire_core::rules::Rules::parse(&read(&rules_path)?)
                .map_err(|e| format!("{}: {e}", rules_path.display()))?;
            parsed
                .validate_against(&library)
                .map_err(|e| format!("{}: {e}", rules_path.display()))?;
            parsed
        } else {
            bevy_grimoire_core::rules::Rules {
                version: bevy_grimoire_core::rules::RULES_VERSION,
                blocks: Vec::new(),
            }
        };

        let maps_dir = project_dir.join(MAPS_DIR);

        // Counts come from the one census, never from a `.len()` written here — see
        // `bevy_grimoire_core::census` for the drift that discipline exists to prevent.
        let catalog = bevy_grimoire_core::census::of_catalog(&library, &compositions.compositions);
        info!(
            "project: {} — {} descriptor(s), {} composition(s), {} bound kit(s)",
            root.display(),
            catalog.descriptors,
            catalog.compositions,
            layers.len(),
        );

        // **Pieces past the lattice ceiling are named once, here.** They can be placed and drawn
        // but not given edge tokens ([`Project::divisions_of`] refuses them), and an author who
        // cannot select a tree's edges should be able to find out why without reading the source.
        let oversize: Vec<&str> = library
            .descriptors
            .iter()
            .filter(|d| {
                bevy_grimoire_core::descriptor::divisions(d, kits.lattice.face_bands).is_ok_and(|div| {
                    bevy_grimoire_core::descriptor::Subgrid::volume(div)
                        > bevy_grimoire_core::library::MAX_LATTICE_CELLS
                })
            })
            .map(|d| d.id.as_str())
            .collect();
        if !oversize.is_empty() {
            info!(
                "project: {} piece(s) are past the {} cell lattice ceiling and cannot take edge \
                 tokens: {}",
                oversize.len(),
                bevy_grimoire_core::library::MAX_LATTICE_CELLS,
                oversize.join(", ")
            );
        }

        let triangles = library
            .descriptors
            .iter()
            .map(|d| triangles_of(root, d))
            .collect::<Vec<usize>>()
            .into();

        Ok(Project {
            root: root.to_path_buf(),
            grimoire_dir,
            namespace,
            lattice: kits.lattice,
            bashes: kits.bash,
            kits: layers.into(),
            library_path,
            vocab,
            measured: measured.into(),
            compositions,
            library: library.into(),
            rules,
            policy,
            masks: masks.into(),
            project_dir,
            maps_dir,
            // **No map yet.** The Map door sets it when it opens one, and the crossing carries it.
            open_map_name: None,
            touched: Vec::new(),
            triangles,
        })
    }

    /// **This piece's lattice divisions**, from its own size and the project's divisions-per-tile.
    ///
    /// The one place the editor derives them, so the grid an author clicks, the grid the gizmos
    /// draw, and the grid a write is range-checked against cannot come out different.
    pub fn divisions_of(
        &self,
        d: &bevy_grimoire_core::descriptor::Descriptor,
    ) -> Result<(u32, u32, u32), String> {
        let div = bevy_grimoire_core::descriptor::divisions(d, self.lattice.face_bands)?;
        let volume = bevy_grimoire_core::descriptor::Subgrid::volume(div);
        if volume > bevy_grimoire_core::library::MAX_LATTICE_CELLS {
            return Err(format!(
                "`{}` is too large for a lattice ({}x{}x{} = {volume} cells, past {}). It can be \
                 placed, but not given edge tokens.",
                d.id,
                div.0,
                div.1,
                div.2,
                bevy_grimoire_core::library::MAX_LATTICE_CELLS
            ));
        }
        Ok(div)
    }

    /// Re-read `assets/grimoire/vocab.ron` and re-resolve the masks. Adopts only on success.
    ///
    /// The editor holds a parsed copy of the vocabulary and the resolved masks, so a token appended
    /// on disk (by [`bevy_grimoire_core::vocab::append_token`]) is invisible until this runs. Appending
    /// never changes an existing bit, so already-resolved masks stay correct — the re-resolve is
    /// for the new token's own mask, and the swap is all-or-nothing so a bad file cannot leave the
    /// project half-updated.
    pub fn reload_vocab(&mut self) -> Result<(), String> {
        let path = self.root.join(VOCAB);
        let text = read(&path)?;
        let vocab = Vocabularies::parse(&text)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let masks = self
            .library
            .resolve(&vocab)
            .map_err(|e| format!("{}: {e}", self.library_path.display()))?;
        self.vocab = vocab;
        self.masks = masks.into();
        Ok(())
    }

    /// **The merged library, with the authoring kit's layer swapped for `layered`.**
    ///
    /// [`Self::library`] is every bound kit concatenated, so an edit to the kit being authored
    /// cannot simply replace it — doing that dropped every other kit's pieces out of the palette
    /// the moment a mesh was imported. Rebuilt from [`Self::kits`] rather than re-read from disk,
    /// because `commit_measured` validates *before* it writes and the file on disk is still the old
    /// one at the moment the merge is wanted.
    ///
    /// Validated on the way out, so a rename or an import that collides with another bound kit is
    /// refused here — at the commit door — rather than at the next open.
    pub fn merged_with(&self, layered: &Library) -> Result<Library, String> {
        let mut descriptors = Vec::new();
        for k in self.kits.iter() {
            if k.dir == self.grimoire_dir {
                descriptors.extend(layered.descriptors.iter().cloned());
            } else {
                descriptors.extend(k.library.descriptors.iter().cloned());
            }
        }
        let merged = Library {
            version: bevy_grimoire_core::library::LIBRARY_VERSION,
            note: self.library.note.clone(),
            descriptors,
        };
        merged.validate().map_err(|e| {
            format!("{e} — another bound kit already defines it, so this edit would make every \
                     reference to that id ambiguous.")
        })?;
        Ok(merged)
    }

    /// **Re-read one bound kit's files, and swap that kit's layer — not the whole project.**
    ///
    /// # Why this exists (E-003)
    ///
    /// [`crate::watch::adopt`] answered a save to a kit's `library.ron` or `project.ron` by
    /// re-opening the whole project: `kits.ron`, the vocabulary, **every** bound kit's library and
    /// policy, and a triangle count for every GLB in the merge. Measured on the shipped project
    /// (90 descriptors, 2 kits, six opens: 27, 58, 104, 185, 192, 315 ms), the triangle pass alone
    /// is 22–255 ms of that — up to a quarter second of the editor being wrong *per save*, on the
    /// file an author edits most.
    ///
    /// **A kit's own save can only change that kit.** The binding (`kits.ron`) is what decides
    /// which directories answer for which namespace, and it is a different file, watched
    /// separately; a change to it still goes through the whole-project open. So this re-reads one
    /// directory and re-does the *derived* work over the result.
    ///
    /// # It is the same answer the open gives, not a cheaper one
    ///
    /// Every check `Project::open` performs on the merge is performed here, in the same order and
    /// with the same functions — the namespace the binding declares, the duplicate-id refusal, the
    /// implied-effects settling, the lattice ceiling, the masks, the compositions against the merged
    /// library, and the rules against it. A refusal leaves `self` untouched, so the editor keeps
    /// the last good project, exactly as a refused open does.
    ///
    /// **The one thing it does not re-measure is another kit's triangles** — a `.glb` in a kit
    /// nobody edited has not moved, and the count is a fact about that file.
    ///
    /// Returns the descriptor ids that changed, by [`crate::tiles::changed_ids`] — the same
    /// comparison the in-editor commit door uses, so the Map redraws exactly the pieces whose
    /// shapes moved.
    pub fn reload_kit(&mut self, kit_dir: &Path) -> Result<Vec<String>, String> {
        let at = self
            .kits
            .iter()
            .position(|k| k.dir == kit_dir)
            .ok_or_else(|| {
                format!(
                    "{}: not a kit this project has bound, so there is nothing to re-read.",
                    kit_dir.display()
                )
            })?;
        let layered = bevy_grimoire_core::policy::layered_library(kit_dir)?;
        let bound = self.kits[at].namespace.clone();

        // **Declared, then verified** — the same rule as `kits::bound_library`, and the same
        // sentence: a directory that now defines a different namespace cannot answer for the one
        // `kits.ron` binds it as.
        match layered.measured.namespace()? {
            Some(ns) if ns != bound => {
                return Err(format!(
                    "{}: bound as `{bound}` but its pieces are `{ns}/*`. A directory is a skin and \
                     the namespace is the interface it implements — bind it as `{ns}`, or point \
                     `{bound}` at the directory that provides it.",
                    kit_dir.display()
                ));
            }
            _ => {}
        }

        // **The merge, from the layers the project already holds plus this one.** Every other kit
        // is what it was; only `at` moves.
        let mut descriptors = Vec::new();
        for (i, k) in self.kits.iter().enumerate() {
            if i == at {
                descriptors.extend(layered.library.descriptors.iter().cloned());
            } else {
                descriptors.extend(k.library.descriptors.iter().cloned());
            }
        }
        let mut library = Library {
            version: bevy_grimoire_core::library::LIBRARY_VERSION,
            note: self.library.note.clone(),
            descriptors,
        };
        library.validate().map_err(|e| {
            format!(
                "{e} — another bound kit already defines it, so every reference to that id is \
                 ambiguous."
            )
        })?;
        // Settled over the whole merged set, for the reason `commit_measured` gives: `measured`
        // and the layers never receive the correction, so an unsettled row here would read as a
        // change on the very first edit.
        crate::labels::settle_library(&mut library, &self.vocab);
        library.validate_lattices(self.lattice.face_bands)?;
        let masks = library
            .resolve(&self.vocab)
            .map_err(|e| format!("{}: {e}", kit_dir.join(LIBRARY_FILE).display()))?;

        // **The project-wide files, re-checked against the new merge.** A kit edit can take away a
        // piece a composition or a block rule names, and both are refused at open for exactly that.
        //
        // **`vocab.check_slots` is not re-run, and that is not a gap**: it answers a question about
        // the vocabulary and the compositions — neither of which a kit's own two files can change —
        // and it takes no library, so its answer cannot move here.
        bevy_grimoire_core::composition::validate(&self.compositions.compositions, &library).map_err(
            |e| format!("{}: {e}", self.project_dir.join(bevy_grimoire_core::composition::Compositions::FILE).display()),
        )?;
        self.rules
            .validate_against(&library)
            .map_err(|e| format!("{}: {e}", self.project_dir.join(bevy_grimoire_core::rules::FILE).display()))?;

        // **Triangles: this kit's rows from disk, every other kit's carried over.** The count is a
        // fact about a `.glb`, and no file outside this kit was edited. Carried by id, because the
        // merge has just changed order and a positional copy would land counts on the wrong rows.
        let old: std::collections::HashMap<&str, usize> = self
            .library
            .descriptors
            .iter()
            .map(|d| d.id.as_str())
            .zip(self.triangles.iter().copied())
            .collect();
        let owned: std::collections::HashSet<&str> = layered
            .library
            .descriptors
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        let root = &self.root;
        let triangles = library
            .descriptors
            .iter()
            .map(|d| {
                if !owned.contains(d.id.as_str())
                    && let Some(n) = old.get(d.id.as_str())
                {
                    return *n;
                }
                triangles_of(root, d)
            })
            .collect::<Vec<usize>>()
            .into();

        // **Nothing above this line has touched `self`** — every refusal returns before the swap.
        let touched = crate::tiles::changed_ids(&self.library, &library);
        let authoring = self.kits[at].dir == self.grimoire_dir;
        let kit = &mut self.kits[at];
        kit.measured = layered.measured;
        kit.library = layered.library;
        kit.policy = layered.policy;
        if authoring {
            // The three fields that only ever describe the authoring kit's layer.
            self.measured = self.kits[at].measured.clone().into();
            self.policy = self.kits[at].policy.clone();
        }
        self.library = library.into();
        self.masks = masks.into();
        self.triangles = triangles;
        Ok(touched)
    }

    /// **Which kit defines this piece** — by the library it is in, never by reading its id.
    ///
    /// The first version split the id on `/` and took the prefix. That is wrong for every kit that
    /// ships: the furniture library's 75 ids are flat (`lamp_tall`, not `furniture/lamp_tall`), so
    /// every one of them belonged to no kit, the palette filter matched nothing, and the whole
    /// selection was inert — ticking changed a label and offered exactly the same rows. Found by
    /// driving the shipped project rather than a fixture, which is the only place the flat ids are.
    ///
    /// `KitLayer::namespace` is the **bound** namespace, so it answers for a flat library too, and
    /// it survives a re-skin: `site_greybox` bound as `site` reports `site`, which is what a map
    /// naming `site/floor` means.
    pub fn kit_of(&self, id: &str) -> Option<&str> {
        self.kits
            .iter()
            .find(|k| k.library.get(id).is_some())
            .map(|k| k.namespace.as_str())
    }

    /// **A piece another kit owns**, by that kit's namespace. `None` when this kit owns it, or
    /// when no bound kit has it. The refusals use it to say whose piece it is.
    pub fn other_kit_of(&self, id: &str) -> Option<&str> {
        self.kit_of(id).filter(|ns| *ns != self.namespace)
    }

    /// **Every map in this project that places `id`**, by name, in a stable order.
    ///
    /// # It used to ask the open map, which was the wrong map
    ///
    /// Removing a piece from a kit is refused while something still places it, and that guard read
    /// `project.map` — *the one map the author happened to have open*. So removing a piece used by
    /// **another** map in the same project was allowed, and the damage showed up later as that map
    /// refusing to resolve, with nothing pointing at the edit that caused it.
    ///
    /// The doors made the narrow version impossible rather than merely wrong: the Kits door has no
    /// open map to ask. Asking the project is the answer that was always correct, and it is
    /// affordable because removal is a keypress, not a frame.
    ///
    /// A map that will not parse is **named as unreadable rather than skipped**. Skipping it would
    /// mean a broken file silently withdrew its vote, which is how a guard quietly stops guarding.
    pub fn maps_that_place(&self, id: &str) -> Result<Vec<String>, String> {
        let dir = match std::fs::read_dir(&self.maps_dir) {
            Ok(d) => d,
            // No `maps/` yet is a project nobody has made a map in — no votes, not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("{}: {e}", self.maps_dir.display())),
        };
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in dir {
            let path = entry.map_err(|e| format!("{}: {e}", self.maps_dir.display()))?.path();
            if path.to_string_lossy().ends_with(".map.ron") {
                paths.push(path);
            }
        }
        // Sorted, so the message an author reads names them in the same order every time.
        paths.sort();

        let mut out = Vec::new();
        for path in paths {
            let text = read(&path)?;
            let map = match Map::parse(&text) {
                Ok(m) => m,
                Err(e) => {
                    return Err(format!(
                        "{} will not parse, so it cannot be asked whether it places `{id}`: {e}",
                        path.display()
                    ));
                }
            };
            let placed = map.placements.iter().any(|p| p.descriptor == id);
            // A stamp reaches descriptors through its composition's members, so a map that only
            // stamps still votes — otherwise a piece seated inside every tile would read as unused.
            let stamped = map.stamps.iter().any(|s| {
                self.compositions
                    .compositions
                    .iter()
                    .find(|c| c.id == s.of)
                    .is_some_and(|c| {
                        c.members.iter().any(|m| match &m.body {
                            bevy_grimoire_core::composition::Body::Descriptor { id: d, .. } => d == id,
                            bevy_grimoire_core::composition::Body::Composition { .. }
                            | bevy_grimoire_core::composition::Body::Slot { .. } => false,
                        })
                    })
            });
            if placed || stamped {
                out.push(map.name);
            }
        }
        Ok(out)
    }

    /// Re-measure the per-entry triangle counts after the library changes.
    ///
    /// Called when an import lands, so the palette's cost column covers the new piece rather than
    /// silently reading zero for it.
    pub fn remeasure_triangles(&mut self) {
        let root = &self.root;
        self.triangles = self
            .library
            .descriptors
            .iter()
            .map(|d| triangles_of(root, d))
            .collect::<Vec<usize>>()
            .into();
    }

    /// **The one door to `compositions.ron`.** Insert or replace by id, sort, validate the whole set,
    /// write atomically, and adopt only on success.
    ///
    /// # Why it lives here and not on a tab
    ///
    /// The invariant FVS-R-15 bought is **one writer** — it was spent making the Compose tab read-only
    /// while the Map authored, and `tests/compose_is_read_only.rs` held the line by naming the tabs.
    /// Naming tabs was always a proxy: what mattered is that one function opens the file. Now that
    /// tiles are assembled on the Tiles tab *and* captured from a box on the Map, two tabs legitimately
    /// author compositions, and a per-tab rule would either forbid the feature or be quietly widened
    /// until it forbade nothing.
    ///
    /// So the rule moves to where it was always aimed: **`project.rs` is the only module that names
    /// the file**, and every author-facing verb comes through here. That is the same shape
    /// `tiles::commit_measured` has for `library.ron`, and the ratchet now asserts it directly.
    ///
    /// # Validated as a set, before anything is written
    ///
    /// `composition::validate` is whole-set because containment is a set property — a nested group
    /// that stops existing breaks its parent, not itself. Validating the *proposal* rather than the
    /// live set is what makes a refusal leave the project exactly as it was.
    pub fn commit_composition(
        &mut self,
        comp: bevy_grimoire_core::composition::Composition,
    ) -> Result<(), String> {
        let proposed = self.proposed_compositions(comp)?;

        // **The project's collection, not the authoring kit's.** A tile may seat `site/wall` beside
        // `lab/bench`, so it belongs to neither kit — filing it under whichever one happened to be
        // open is what made a tile authored in one kit invisible to every map in another.
        let path = self
            .project_dir
            .join(bevy_grimoire_core::composition::Compositions::FILE);
        let text = proposed.to_ron()?;
        bevy_grimoire_core::ron_surgery::save_atomic(&path, &text)
            .map_err(|e| format!("NOT WRITTEN: {e}"))?;
        self.compositions = proposed;
        Ok(())
    }

    /// **What [`Project::commit_composition`] would refuse, without writing**: the collection with `comp`
    /// in it, in canonical order, validated whole. A preview calls it so it refuses what the commit will.
    pub fn check_composition(
        &self,
        comp: &bevy_grimoire_core::composition::Composition,
    ) -> Result<(), String> {
        self.proposed_compositions(comp.clone()).map(|_| ())
    }

    fn proposed_compositions(
        &self,
        comp: bevy_grimoire_core::composition::Composition,
    ) -> Result<bevy_grimoire_core::composition::Compositions, String> {
        let mut proposed = self.compositions.clone();
        match proposed.compositions.iter().position(|c| c.id == comp.id) {
            Some(i) => proposed.compositions[i] = comp,
            None => proposed.compositions.push(comp),
        }
        // Canonical order, so one set has one encoding and a diff shows what changed rather than
        // where it was inserted.
        proposed.compositions.sort_by(|a, b| a.id.cmp(&b.id));
        bevy_grimoire_core::composition::validate(&proposed.compositions, &self.library)?;
        Ok(proposed)
    }

    /// **Take a composition out of the project's collection** — the inverse of
    /// [`Project::commit_composition`] for an id that did not exist before.
    ///
    /// Refuses while any map in the project names the id between quotes (a conservative text test: a
    /// map that stamped it would stop loading), and when another composition nests it. Nothing is
    /// written on a refusal.
    pub fn remove_composition(&mut self, id: &str) -> Result<(), String> {
        let mut proposed = self.compositions.clone();
        let before = proposed.compositions.len();
        proposed.compositions.retain(|c| c.id != id);
        if proposed.compositions.len() == before {
            return Err(format!("the project has no composition `{id}`"));
        }
        let quoted = format!("\"{id}\"");
        if let Ok(entries) = self.maps_dir.read_dir() {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                    continue;
                }
                if std::fs::read_to_string(&path).is_ok_and(|t| t.contains(&quoted)) {
                    return Err(format!(
                        "{} still names `{id}`, so it cannot be removed. Remove its stamps first.",
                        path.display()
                    ));
                }
            }
        }
        bevy_grimoire_core::composition::validate(&proposed.compositions, &self.library)?;
        let path = self
            .project_dir
            .join(bevy_grimoire_core::composition::Compositions::FILE);
        let text = proposed.to_ron()?;
        bevy_grimoire_core::ron_surgery::save_atomic(&path, &text)
            .map_err(|e| format!("NOT WRITTEN: {e}"))?;
        self.compositions = proposed;
        Ok(())
    }

    /// **Rename a composition, and every map that stamps it.**
    ///
    /// # Why it lives here
    ///
    /// **Because this is the only module allowed to name the compositions file** — see
    /// `tests/compose_is_read_only.rs`: *"project.rs is the only module that names the file, and
    /// every tab reaches it there."* The COMPOSE tab asks; it does not write.
    ///
    /// # All of it, or none of it
    ///
    /// The core's writer returns **every** new text, and this writes them **only if every one
    /// succeeded** — the composition's file and every map that stamped it. A rename that updated the
    /// composition and left one map naming the old id would be a **partial rename**, and a partial
    /// rename is a project that will not open.
    ///
    /// **And a refusal changes nothing**: the old name stands, the old files stand, and the reason
    /// is what the caller is given.
    pub fn rename_composition(
        &mut self,
        from: &str,
        to: &str,
        // The map that is open, so the editor shows what the file now says.
        open: Option<&mut OpenMap>,
    ) -> Result<(), String> {
        let path = self
            .project_dir
            .join(bevy_grimoire_core::composition::Compositions::FILE);
        let text = read(&path)?;

        // **Every map in the project.** The ones that stamp the composition are edited; the ones that
        // do not are left byte for byte, which the core's writer does by itself.
        let map_texts: Vec<(String, String)> = self
            .maps_dir
            .read_dir()
            .map_err(|e| format!("cannot read the maps: {e}"))?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                    return None;
                }
                let text = std::fs::read_to_string(&path).ok()?;
                Some((path.display().to_string(), text))
            })
            .collect();

        let renamed =
            bevy_grimoire_core::composition::rename_composition(&text, &map_texts, from, to)?;

        // **The composition's file.**
        bevy_grimoire_core::ron_surgery::save_atomic(&path, &renamed.compositions)
            .map_err(|e| format!("NOT RENAMED: cannot write {}: {e}", path.display()))?;

        // **And every map that named it.** A map the rename did not touch is not written at all —
        // writing it back would be a second chance to fail.
        for (name, new_text) in &renamed.maps {
            let unchanged = map_texts
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, t)| t == new_text)
                .unwrap_or(true);
            if unchanged {
                continue;
            }
            bevy_grimoire_core::ron_surgery::save_atomic(std::path::Path::new(name), new_text)
                .map_err(|e| {
                    format!(
                        "NOT RENAMED: the composition is renamed but {name} could not be written: \
                         {e}. The project needs that file fixed before it will open."
                    )
                })?;
        }

        // **And the project, in memory** — the editor must show what the file now says.
        let set = bevy_grimoire_core::composition::Compositions::parse(&renamed.compositions)?;
        self.compositions = set;
        if let Some(open) = open {
            let path = open.map_path.display().to_string();
            if let Some((_, new_text)) = renamed.maps.iter().find(|(n, _)| *n == path)
                && let Ok(map) = bevy_grimoire_core::map::Map::parse(new_text)
            {
                open.map = map;
            }
        }
        Ok(())
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// **Which `.glb` files have been opened for a triangle count** — E-003's second measure.
///
/// The triangle pass is the largest single item in a project open (22–255 ms of a 27–315 ms
/// open on the shipped project, 90 descriptors), and it is a fact about a *file*:
/// a `.glb` nobody edited has not moved. A save to one kit re-opening the whole project
/// re-measures every other kit's meshes for nothing, and this is how that is measured.
///
/// **The process, and not the thread**, for the reason `policy::layered_for_test` gives: the work
/// runs inside `watch::adopt_changes`, which Bevy's default `MultiThreadedExecutor` may run on a
/// task-pool thread. `tiles::library_writes_for_test` can be a `thread_local` because the editor's
/// own commits run on the caller's thread; a watcher adoption does not.
static MEASURED: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// **The `.glb` files opened for a triangle count since the last reset.**
pub fn triangles_measured_for_test() -> Vec<PathBuf> {
    MEASURED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// **Start the count over**, so an editor's own open is not charged to the change under test.
pub fn forget_triangles_measured_for_test() {
    MEASURED.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// **One descriptor's triangle count**, read from its `.glb`'s JSON chunk.
///
/// The one place the count is taken, so a caller cannot measure a different thing — and so the
/// E-003 measure has exactly one increment to watch. `0` for a piece with no mesh, and for a mesh
/// that will not open: the palette's cost column reading zero is honest about a file that is not
/// there, and refusing to open a project over one unreadable mesh would be worse.
fn triangles_of(root: &Path, d: &bevy_grimoire_core::descriptor::Descriptor) -> usize {
    let Some(path) = d
        .mesh
        .as_deref()
        .map(|m| root.join("assets").join(m))
    else {
        return 0;
    };
    MEASURED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(path.clone());
    bevy_grimoire_core::glb::Glb::open(&path)
        .ok()
        .map_or(0, |g| bevy_grimoire_core::import::triangles(&g))
}

/// **The map being edited** — the Maps door's subject, and nothing else's.
///
/// # Why this is not on [`Project`]
///
/// It was, until the editor split into one door per entity. Four of the five doors — Kits, Tiles,
/// Compose and Rigs — author things that outlive any particular map, and a `Project` that carried
/// one forced each of them to name a map they would never read or write. Any name picked there is
/// arbitrary, and an arbitrary value that nothing reads is exactly the kind of thing that is still
/// sitting there when somebody later writes code that *does* read it.
///
/// The measurement that made the cut cheap: of ~270 map-field accesses in this crate, **234 were in
/// `editor.rs`** — the module that *is* the Maps door — and `dirty = true` was written in that one
/// file and nowhere else. The Meshes and Tiles doors commit straight to disk through
/// `tiles::commit_measured` and [`Project::commit_composition`], so the map is the only thing this
/// editor ever holds unsaved.
/// **Cloned, for the same reason as [`Project`]** — the operation context owns the map it edits.
#[derive(Resource, Clone)]
pub struct OpenMap {
    /// The map itself.
    pub map: Map,
    /// Where it will be written.
    pub map_path: PathBuf,
    /// The map as it is on disk (or as it was when opened). [`OpenMap::dirty`] compares against it.
    ///
    /// **Dirty is derived, never flagged.** A boolean set by every edit site said "unsaved" after a
    /// piece was picked up and put back, and after place-then-undo, because nothing ever cleared it
    /// when the map went back to what the disk holds. `Map` is `PartialEq`, so the question "do I
    /// differ from disk" is answerable directly, and no edit site has to remember to ask it.
    saved: Map,
}

impl OpenMap {
    /// An open map with `map` as both its content and its on-disk state: clean.
    pub fn new(map: Map, map_path: PathBuf) -> OpenMap {
        OpenMap { saved: map.clone(), map, map_path }
    }

    /// Whether the map differs from what is on disk.
    pub fn dirty(&self) -> bool {
        self.map != self.saved
    }

    /// **What the editor last saved or read** — the map as the file has it.
    ///
    /// The file watcher's whole question is *"does the file say something the editor does not?"*,
    /// and there are two "the editor" it can mean: `map`, which holds the person's unsaved edits,
    /// and this, which holds what the file said the last time the two agreed. **The second one is
    /// the one a re-read may be compared against.** Comparing against `map` made every save
    /// followed by an unsaved edit look like an outside change, and the poll then replaced the
    /// person's edit with the file — see `watch::adopt_map`.
    pub fn saved(&self) -> &Map {
        &self.saved
    }

    /// **Say that what is on disk is what the editor holds.**
    ///
    /// **Used by the file watcher**, which re-reads the map from disk and puts it in the editor.
    /// **Without it the editor would think the map was unsaved** — the dirty flag compares the
    /// map with what it last saved, and a re-read from disk did not go through the save — **and
    /// the author would be told to save a map that was already saved.**
    pub fn mark_saved(&mut self) {
        self.saved = self.map.clone();
    }

    /// Open a map by **name**, or start one under that name.
    ///
    /// The name is forced into snake_case rather than checked, so there is no path through this
    /// program on which a map has a name the filesystem and the schema disagree about.
    pub fn open(project: &Project, map_name: &str) -> Result<OpenMap, String> {
        let name = naming::to_snake_case(map_name);
        if name.is_empty() {
            return Err(format!(
                "`{map_name}` leaves nothing usable as a name. Names are snake_case — lowercase \
                 letters, digits and single underscores, starting with a letter."
            ));
        }
        let map_path = project.maps_dir.join(naming::map_file_name(&name));
        // A map that does not exist yet is a new map, not an error: this is how an author starts one.
        // A map that exists and does not parse IS an error — silently replacing it with an empty one
        // would destroy their work on the first save.
        let map = if map_path.is_file() {
            let loaded: Map = Map::parse(&read(&map_path)?)
                .map_err(|e| format!("{}: {e}", map_path.display()))?;
            // A map's name and its file have to agree, or a rename leaves two files answering to one
            // name and nobody can say which is the level.
            if loaded.name != name {
                return Err(format!(
                    "{} calls itself `{}`. A map's name IS its file — rename the file to \
                     `{}` or open it under its own name.",
                    map_path.display(),
                    loaded.name,
                    naming::map_file_name(&loaded.name)
                ));
            }
            loaded
        } else {
            Map {
                version: MAP_VERSION,
                name: name.clone(),
                note: Some(format!(
                    "Authored in bevy_cauldron. Library: {}.",
                    project.library_path.display()
                )),
                ..Map::default()
            }
        };

        // **A map names a bash the project declares, or it does not open.** The name is not checked
        // by `Map::validate` — a map validates in isolation and cannot see `kits.ron` — so this is
        // the one door where both halves are in hand. Refused by name rather than resolved to
        // every-kit, because a silent fallback is a palette nobody chose.
        if let Some(name) = map.bash.as_deref()
            && !project.bashes.iter().any(|b| b.name == name)
        {
            let declared = if project.bashes.is_empty() {
                "none".to_owned()
            } else {
                project
                    .bashes
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(format!(
                "{}: names bash `{name}`, which `kits.ron` does not declare. Declared: {declared}.",
                map_path.display()
            ));
        }

        // **Counted by the census**, like every other count — the ratchet
        // `tests/census_is_the_one_counter.rs` enforces, and the reason two panels cannot
        // disagree about the same number.
        //
        // The map's own rows and the rows its stamps bring are separate numbers, and the log
        // says both: a map with 728 hand-placed rows and one stamp of an 18-member group is a
        // 746-row level, and a log that said "728" would be describing half of it.
        let counted = bevy_grimoire_core::census::of_map(
            &map,
            &project.compositions.compositions,
            &project.library,
        );
        match counted {
            Ok(counted) => info!(
                "map: {} — {} placement(s), {} stamp(s) standing for {} row(s)",
                map_path.display(),
                counted.placements,
                counted.stamps,
                counted.stamped
            ),
            // A map whose stamps do not expand is refused by the save gate, but opening it is
            // still legal — the author may be about to fix the group. The log says so rather
            // than printing a count that would be a guess.
            Err(e) => info!(
                "map: {} — and its stamps do not expand: {e}",
                map_path.display()
            ),
        }

        Ok(OpenMap::new(map, map_path))
    }

    /// Write the map. Atomic, so a crash mid-write cannot leave half a level on disk.
    ///
    /// Serialized by an ordinary serializer with no comment-preserving pass, because an grimoire map
    /// keeps its prose in `note:` fields — see `bevy_grimoire_core::map`. That is the whole reason the
    /// decision was made that way.
    pub fn save(&mut self, project: &Project) -> Result<(), String> {
        self.map.validate()?;
        // **The save gate has to ask the question the game will ask.** `validate` checks the map's
        // own shape and stops there; it does not expand stamps, so a map whose group no longer
        // resolves passed this door and was written with a cheerful "saved" — and then failed at
        // `FVS_EMERGE_MAP` load with "the map has holes". The editor saying saved while the game says
        // broken is the exact drift the shared `bevy_grimoire_core` validation exists to prevent.
        //
        // **And now the question is literally the same one.** The game expands a map's stamps when
        // it loads it, so the gate expands them the same way — through `Map::expanded`, the same
        // function `project_data::load` calls. Before this, the gate checked that the stamps
        // *could* expand while the game did not expand them at all, so the gate was on the wrong
        // side of the file: it proved the file was loadable, not that the game would load it.
        if !self.map.stamps.is_empty() || !self.map.runs.is_empty() {
            self.map
                .expanded(&project.compositions.compositions, &project.library)
                .map_err(|e| format!("not saved — the game could not load this: {e}"))?;
        }
        // Follow a rename. The path is derived rather than remembered, so the file a map is in is
        // always the file its name says it is.
        //
        // In the project's `maps/`, not beside a kit. It used to be beside the kit it was authored
        // with, on the argument that a map written where its library is not opens against the wrong
        // tiles — which was right while a project was one kit and is now the opposite of right: the
        // library that draws a map is the merge of every bound kit, so filing it under one of them
        // picks a winner arbitrarily.
        let path = project.maps_dir.join(naming::map_file_name(&self.map.name));
        // One placement a line (`Map::to_ron`), so an edit of one row is a diff of one line.
        let text = self.map.to_ron()?;

        // **A rename is a move, and a move is not a copy.**
        //
        // This is where the map's name and the file's name come apart, and where they were
        // allowed to: the old file was never removed, so a rename left **two files answering to one
        // map** — and the stale one *"refuses to open with `<path>` calls itself `<name>`"*, a
        // message about a schema mismatch, for a rename that half-happened. The old file's removal
        // belongs here, at the save, and not on the keystroke that typed the name: the editor's own
        // reason for not removing it there was *"deleting it here would destroy a file on a
        // keystroke"*, which is true and is why the removal waits for the write.
        let renamed = path != self.map_path;

        if renamed {
            // **The refusal, and it is the kernel's.** `save_atomic_no_replace` will not replace an
            // existing target, and it will not *check* first either — POSIX gives atomicity to the
            // single call, so a check-then-write would leave a window in which the target appears
            // and is then removed. A map is a file the author spent hours on, and "silently
            // replaced" is not a thing that may happen to one.
            //
            // **And it names the map that is in the way**, because the author needs to know *which*
            // name to change.
            if path.exists() {
                // **The name that is taken, and the file that holds it.** The author needs to know
                // *which* name to change, and the map that is in the way is a file they can open.
                //
                // **And it is the new name twice, on purpose**: the name the map is *becoming* is
                // the one that is taken, and the map it is *becoming it from* is the file the
                // author is editing. Saying it once would leave the author with two names and no
                // way to tell which is which.
                let was = self
                    .map_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| self.map.name.clone());
                return Err(format!(
                    "NOT SAVED: `{}` is already a map in this project — {} — so the map now \
                     called `{}` cannot take that name. Rename this map to something else.",
                    self.map.name,
                    path.display(),
                    was
                ));
            }
            bevy_grimoire_core::ron_surgery::save_atomic_no_replace(&path, &text)?;
            // **And now the old file goes — or the rename is taken back.** A rename is all or nothing:
            // a refused call changes nothing, so a rename whose old file cannot be removed has not
            // happened. The new file is ours (save_atomic_no_replace just created it), so removing it
            // touches nothing the author had.
            if let Err(e) = std::fs::remove_file(&self.map_path) {
                let old = self.map_path.clone();
                return match std::fs::remove_file(&path) {
                    Ok(()) => Err(format!(
                        "the old file {} could not be removed, so the map keeps its name: {e}. \
                         Nothing was changed.",
                        old.display()
                    )),
                    // **The take-back failed too, so the map is in two files and must say so.** The
                    // map follows the new file, because that is where the write landed, and the
                    // refusal tells the author which file to delete by hand.
                    Err(back) => {
                        self.map_path = path.clone();
                        Err(format!(
                            "saved as `{}`, but the old file {} could not be removed: {e}, and the new \
                             file {} could not be taken back: {back}. Delete the old file by hand — \
                             until then the list shows this map twice.",
                            self.map.name,
                            old.display(),
                            path.display()
                        ))
                    }
                };
            }
        } else {
            bevy_grimoire_core::ron_surgery::save_atomic(&path, &text)?;
        }
        self.map_path = path;
        self.saved = self.map.clone();
        Ok(())
    }


    /// **The namespaces this map's content already names** — derived every time, never stored.
    ///
    /// Placements name a descriptor directly; a stamp names a composition, whose members name
    /// descriptors, so both are walked. `bevy_grimoire_core::census` makes the same argument for counts and
    /// this is the same discipline: a set that could be cached is a set that can be wrong, and this
    /// one decides whether a checkbox is allowed to be unticked.
    pub fn namespaces_in_use(&self, project: &Project) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let mut add = |id: &str| {
            if let Some(k) = project.kit_of(id) {
                out.insert(k.to_owned());
            }
        };
        for p in &self.map.placements {
            add(&p.descriptor);
        }
        for stamp in &self.map.stamps {
            let Some(c) = project
                .compositions
                .compositions
                .iter()
                .find(|c| c.id == stamp.of)
            else {
                continue;
            };
            for m in &c.members {
                match &m.body {
                    bevy_grimoire_core::composition::Body::Descriptor { id, .. }
                    | bevy_grimoire_core::composition::Body::Composition { id } => add(id.as_str()),
                    bevy_grimoire_core::composition::Body::Slot { .. } => {}
                }
            }
        }
        out
    }


    /// **What the palette offers**: the bash the map names, plus whatever the map already uses.
    ///
    /// [`bevy_grimoire_core::map::Map::bash`] `None` means *all bound kits* — the state a new map starts
    /// in, and the state a project declaring no bashes leaves every map in.
    ///
    /// **The union with [`Self::namespaces_in_use`] is what makes this safe to edit.** Naming a
    /// bash that leaves out a kit whose pieces are already on the map would otherwise hide the rows
    /// that describe them: the map would still load and still draw, and the author would be unable
    /// to find, re-place or match the pieces in front of them. Folding the in-use set back in means
    /// a bash cannot strand a placement, which is the whole reason it is allowed to be a filter.
    pub fn palette_namespaces(&self, project: &Project) -> std::collections::BTreeSet<String> {
        let Some(name) = self.map.bash.as_deref() else {
            return project.kits.iter().map(|k| k.namespace.clone()).collect();
        };
        // Present by construction — `OpenMap::open` refuses a map naming a bash the project does
        // not declare, so an open map's name always resolves.
        let mut out: std::collections::BTreeSet<String> = project
            .bashes
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.kits.iter().cloned().collect())
            .unwrap_or_default();
        out.extend(self.namespaces_in_use(project));
        out
    }

}
