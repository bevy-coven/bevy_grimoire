//! **A project made of nothing** — for tests that are about the editor rather than about the corpus.
//!
//! `tests/headless.rs` used to boot the shipped `assets/` directory, and several of its assertions
//! read the candidate list, the pack set, or a named descriptor. That makes importing a kit a
//! breaking change to the test suite — and importing kits is the thing this editor exists to do. A
//! fixture writes exactly the project a test is about, so a test fails when the *editor* changes and
//! not when the art does.
//!
//! # What is real here, and why
//!
//! **The font, and only the font.** `harness::install_font` prefers the project's own
//! `assets/fonts/FiraMono-Regular.ttf` and falls back to the copy compiled into the crate. A fixture
//! still borrows the workspace's file, so the tests run the path a project with its own face runs
//! (`tests/font_fallback.rs` covers the other). It is infrastructure rather than corpus: no test
//! asserts anything about it, and no import can change it.
//!
//! The meshes are built here, in memory, by [`Fixture::pack`].
//!
//! # Asset-contract tests are the deliberate exception
//!
//! A test whose *purpose* is "does the shipped valkyrie still measure the way `rigs.ron` claims"
//! must read the shipped valkyrie; that is not corpus dependence, that is the assertion. Those say
//! so in their own doc comment and stay on the real project. Everything else belongs here.

#![allow(dead_code)] // Each test uses a different corner of the builder.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Once;

/// Remove the directories of exited test processes, once per process. A directory whose pid is
/// alive belongs to a run in flight and is left alone, as is anything that does not parse.
fn sweep_dead_fixtures() {
    static SWEPT: Once = Once::new();
    SWEPT.call_once(|| {
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
            return;
        };
        let mut alive: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(rest) = name
                .to_str()
                .and_then(|n| n.strip_prefix("grimoire-fixture-"))
            else {
                continue;
            };
            let Some(pid) = rest.split('-').next().and_then(|p| p.parse::<u32>().ok()) else {
                continue;
            };
            if pid == std::process::id() {
                continue;
            }
            if !*alive.entry(pid).or_insert_with(|| pid_is_alive(pid)) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    });
}

/// `kill -0` delivers no signal and only reports whether the pid exists. A spawn failure counts as
/// alive, so an unexpected environment deletes nothing.
fn pid_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

/// A throwaway project directory in the system temp dir, named `grimoire-fixture-<pid>-<name>`.
///
/// **Nothing deletes it when the test ends.** [`Fixture::build`] hands the project root back as a
/// plain `PathBuf`, so no value is left to drop it, and tests keep using that path after the
/// fixture is gone. The leak is bounded instead: the first [`Fixture::new`] in each test process
/// runs `sweep_dead_fixtures`, which removes the directories of processes that have exited. A
/// failed test's directory therefore survives until the next run, which is when you want to look
/// at it. Before this sweep every run left about 200 directories behind, several GB in all.
pub struct Fixture {
    dir: PathBuf,
    descriptors: Vec<String>,
    placements: Vec<String>,
    compositions: Vec<String>,
    /// **Kits beside the default one**, as `(directory name, library rows)`. See [`Fixture::kit`].
    kits: Vec<(String, Vec<String>)>,
    /// The ids each of those kits was asked for, so the binding can name the namespace they carry
    /// rather than guessing it from the directory.
    ///
    /// The default kit is in here too, under [`DEFAULT_KIT`]: a fixture may call
    /// `.descriptor("site/wall", ..)`, and binding that library as `furniture` is refused at load —
    /// correctly, since the directory says one thing and the ids say another.
    ids: Vec<(String, Vec<String>)>,
    /// **Named combinations of kits**, as `(bash name, namespaces)` — `kits.ron`'s `bash`. Every
    /// namespace named must be one this fixture binds, or the write is refused at load.
    bashes: Vec<(String, Vec<String>)>,
    /// **`rules.ron`'s text**, written beside `kits.ron` by [`Self::build`]. `None` writes no file,
    /// which is a project without rules. See [`Self::rules`].
    rules: Option<String>,
    /// The open map's edge in metres, both horizontal axes. See [`Self::side`].
    side: f32,
}

/// Where the workspace lives, for the one file that has to be borrowed.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|e| panic!("cannot resolve the workspace root: {e}"))
}

/// **A minimal binary glTF**, laid out the way `bevy_grimoire_core::glb` reads one: a JSON chunk then a BIN
/// chunk, each padded to four bytes.
///
/// The same construction `bevy_grimoire_core`'s own `glb` tests use to exercise the reader "without shipping
/// a fixture asset" — restated here rather than reached for, because that one is private to a
/// `#[cfg(test)]` module in another crate.
fn glb(width: f32, height: f32, depth: f32) -> Vec<u8> {
    let (hx, hz) = (width * 0.5, depth * 0.5);
    // **All eight corners.** `derive_front` compares centroids, so a two-vertex "box" is a diagonal
    // and is not symmetric — a mistake `bevy_grimoire_core`'s fixture made first and its own test caught.
    let mut bin = Vec::new();
    for x in [-hx, hx] {
        for y in [0.0f32, height] {
            for z in [-hz, hz] {
                for c in [x, y, z] {
                    bin.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
    }
    let json = format!(
        r#"{{"accessors":[{{"type":"VEC3","componentType":5126,"count":8,"bufferView":0,
            "min":[{:.4},0.0,{:.4}],"max":[{:.4},{:.4},{:.4}]}}],
            "bufferViews":[{{"byteOffset":0,"byteLength":{}}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}}}}]}}]}}"#,
        -hx,
        -hz,
        hx,
        height,
        hz,
        bin.len()
    );

    let mut j = json.into_bytes();
    while j.len() % 4 != 0 {
        j.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + j.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&j);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

/// **Where a fixture's unnamed descriptors live.**
///
/// The project root stopped being a kit on 2026-08-16, so `Fixture::descriptor` needs a kit to put
/// them in. `furniture` is what the shipped project calls the same set — 75 flat ids — so a fixture
/// and the real thing agree about where an unnamespaced library lives.
pub const DEFAULT_KIT: &str = "furniture";

/// The namespace a fixture kit is bound as: whatever its ids carry, or its own directory name when
/// they carry none.
///
/// **The same rule `kits::bound_library` verifies**, applied where the file is written so the two
/// cannot disagree — a fixture binding `greybox` for a library of `site/*` would be refused at open,
/// which is right for a real project and useless as a fixture.
fn namespace_of(dir: &str, ids: &[String]) -> String {
    ids.iter()
        .find_map(|id| id.split_once('/').map(|(ns, _)| ns.to_owned()))
        .unwrap_or_else(|| dir.to_owned())
}

/// **One library row, and the mesh it names**, written under `pack` inside `root`.
///
/// The file is named for the id's **last segment**, so `site/floor` writes `ozea/floor.glb` rather
/// than a `site/` subdirectory nobody asked for — the shape the shipped kits actually have, where
/// the namespace is in the id and never in the mesh path.
///
/// Free rather than a method, because two callers want it: the root kit's builder pushes the row
/// onto its own list, and [`Fixture::kit`] collects rows for a directory of its own.
fn descriptor_row(root: &Path, id: &str, pack: &str, y_offset: f32) -> String {
    let stem = id.rsplit('/').next().unwrap_or(id);
    let at = root.join("assets").join(pack);
    std::fs::create_dir_all(&at).unwrap_or_else(|e| panic!("cannot make {at:?}: {e}"));
    std::fs::write(at.join(format!("{stem}.glb")), glb(1.0, 0.5, 1.0))
        .unwrap_or_else(|e| panic!("cannot write {stem}.glb: {e}"));
    format!(
        r#"        (
            id: "{id}",
            mesh: Some("{pack}/{stem}.glb"),
            align: ( scale: None, stretch_y: None, y_offset: Some({y_offset:.4}), pivot: None, front: None ),
            extent: ( footprint: Some((1.0, 1.0)), height: Some(0.5) ),
            mount: Some(OnFloor),
            clearance: [],
            offers: ( surfaces: [], sockets: [] ),
            kind: ["prop"],
            effects: ["inert"],
            look: ["plain"],
            subgrid: None,
            note: Some("a fixture piece"),
            placement: ( rooms: [], group: None ),
        ),"#
    )
}

impl Fixture {
    /// An empty project: a vocabulary, an empty library, a policy, and the font.
    pub fn new(name: &str) -> Fixture {
        sweep_dead_fixtures();
        // A unique directory per test, since these run in parallel. The process id and the test's
        // own name are enough — no clock, so a rerun is reproducible.
        let dir =
            std::env::temp_dir().join(format!("grimoire-fixture-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let grimoire = dir.join("assets/grimoire");
        std::fs::create_dir_all(&grimoire).unwrap_or_else(|e| panic!("cannot make {grimoire:?}: {e}"));

        // The one borrowed file. See the module note.
        let fonts = dir.join("assets/fonts");
        std::fs::create_dir_all(&fonts).unwrap_or_else(|e| panic!("{e}"));
        let from = workspace().join("assets/fonts/FiraMono-Regular.ttf");
        std::fs::copy(&from, fonts.join("FiraMono-Regular.ttf"))
            .unwrap_or_else(|e| panic!("cannot copy the shipped font from {from:?}: {e}"));

        std::fs::write(
            grimoire.join("vocab.ron"),
            r#"(
    kind: (tokens: [( name: "prop", note: "a thing" )]),
    effects: (tokens: [( name: "inert", note: "does nothing" )]),
    look: (tokens: [( name: "plain", note: "unremarkable" )]),
    surfaces: (tokens: [( name: "worktop", note: "a top" )]),
    capabilities: (tokens: []),
    edge: (tokens: [( name: "wall", note: "a solid run-face" )]),
    slot: (tokens: []),
)"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        std::fs::write(
            grimoire.join("project.ron"),
            "(\n    version: 2,\n    note: None,\n    patches: [],\n)",
        )
        .unwrap_or_else(|e| panic!("{e}"));

        Fixture {
            dir,
            descriptors: Vec::new(),
            placements: Vec::new(),
            compositions: Vec::new(),
            kits: Vec::new(),
            ids: Vec::new(),
            bashes: Vec::new(),
            rules: None,
            side: 16.0,
        }
    }

    /// **A kit beside the root one**, at `assets/grimoire/<name>/` — a `library.ron` providing `ids`
    /// and the `project.ron` `policy::layered_library` requires of every kit.
    ///
    /// Every other helper here writes into the root kit, because until 2026-08-16 a fixture could
    /// only ever make one — so nothing in the suite had two, and multi-kit behaviour was pinned by
    /// a single asset-contract test reading the shipped corpus.
    ///
    /// **Ids are written verbatim, which is the point.** Calling this twice with the same `ids` and
    /// different `name`s builds a **re-skin pair**: two directories providing one namespace, which
    /// is what `site/` and `site_greybox/` are on disk and the case every question about deleting,
    /// binding or resolving a kit turns on.
    pub fn kit(mut self, name: &str, pack: &str, ids: &[&str]) -> Fixture {
        let dir = self.dir.join("assets/grimoire").join(name);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("cannot make {dir:?}: {e}"));
        let rows = ids
            .iter()
            .map(|id| descriptor_row(&self.dir, id, pack, 0.0))
            .collect();
        self.ids
            .push((name.to_owned(), ids.iter().map(|s| (*s).to_owned()).collect()));
        self.kits.push((name.to_owned(), rows));
        self
    }

    /// **Write the project's binding by hand**, for the tests that are about binding itself.
    ///
    /// [`Self::build`] binds every kit as the namespace its ids carry, which is right for the
    /// ordinary case and impossible for a **re-skin pair**: two directories providing `site/*`
    /// cannot both be bound, because that is the ambiguity binding exists to resolve. A test about
    /// the pair binds one at a time, which is what a real project does.
    pub fn bind(root: &Path, binds: &[(&str, &str)], authoring: &str) {
        let list = binds
            .iter()
            .map(|(ns, dir)| format!("(namespace: \"{ns}\", dir: \"{dir}\")"))
            .collect::<Vec<_>>()
            .join(", ");
        let path = root.join("assets/grimoire/kits.ron");
        std::fs::write(
            &path,
            format!(
                "(version: {}, bind: [{list}], bash: [], authoring: Some(\"{authoring}\"))",
                bevy_grimoire_core::kits::KITS_VERSION
            ),
        )
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
    }

    /// **A named combination of kits**, declared in call order — which is the order the chooser's
    /// `B` walks them in. The namespaces are the ones the binding carries, not directory names.
    pub fn bash(mut self, name: &str, kits: &[&str]) -> Fixture {
        self.bashes.push((
            name.to_owned(),
            kits.iter().map(|s| (*s).to_owned()).collect(),
        ));
        self
    }

    /// **A descriptor of a stated footprint**, for a test about what the footprint *does*.
    ///
    /// [`Self::descriptor`] writes a 1 x 1 m piece, which is the ordinary case and exactly the size
    /// that cannot show an envelope growing. This states the number instead of measuring whatever
    /// the generated mesh happens to be.
    pub fn sized_descriptor(mut self, id: &str, pack: &str, w: f32, d: f32) -> Fixture {
        self = self.descriptor(id, pack);
        if let Some(last) = self.descriptors.last_mut() {
            let was = "footprint: Some((1.0, 1.0))";
            assert!(
                last.contains(was),
                "the fixture's descriptor shape changed under this helper"
            );
            *last = last.replace(was, &format!("footprint: Some(({w}, {d}))"));
        }
        self
    }

    /// **A descriptor of a stated height**, for a test about what stands in a frame. [`Self::descriptor`]
    /// writes a 0.5 m piece; this states the number, so a test can put a tall piece on the map.
    pub fn tall_descriptor(mut self, id: &str, pack: &str, h: f32) -> Fixture {
        self = self.descriptor(id, pack);
        if let Some(last) = self.descriptors.last_mut() {
            let was = "height: Some(0.5)";
            assert!(
                last.contains(was),
                "the fixture's descriptor shape changed under this helper"
            );
            *last = last.replace(was, &format!("height: Some({h})"));
        }
        self
    }

    /// **A flat piece with NO mesh**, drawn as a footprint marker — the shape of the shipped
    /// `town/road` and `town/field`: `mesh: None`, a stated footprint, a 0.05 m height, and no
    /// `y_offset`. Two of these placed over one another are the case where their top faces are
    /// coplanar and the editor has to decide which one draws.
    pub fn flat_descriptor(mut self, id: &str, w: f32, d: f32) -> Fixture {
        self.descriptors.push(format!(
            r#"        (
            id: "{id}",
            mesh: None,
            align: ( scale: None, stretch_y: None, y_offset: None, pivot: None, front: None ),
            extent: ( footprint: Some(({w}, {d})), height: Some(0.05) ),
            mount: Some(OnFloor),
            clearance: [],
            offers: ( surfaces: [], sockets: [] ),
            kind: ["prop"],
            effects: ["inert"],
            look: ["plain"],
            subgrid: None,
            note: Some("a flat fixture piece"),
            placement: ( rooms: [], group: None ),
        ),"#
        ));
        match self.ids.iter_mut().find(|(k, _)| k == DEFAULT_KIT) {
            Some((_, v)) => v.push(id.to_owned()),
            None => self.ids.push((DEFAULT_KIT.to_owned(), vec![id.to_owned()])),
        }
        self
    }

    /// **A piece with a HAND-AUTHORED lattice**, one edge token per cell down its run face.
    ///
    /// Every other descriptor this fixture writes carries `subgrid: None`, which is right for the
    /// ordinary case and is why *"an authored lattice survives the disk round-trip"* had no fixture
    /// to be tested against — it read the shipped `site/wall` instead, and went red the day that kit
    /// stopped shipping.
    ///
    /// The round-trip is real here despite the fixture writing the file: this emits RON text, and
    /// what is under test is `policy::layered_library` + `Project::open` reading it back. The two
    /// are not the same code.
    ///
    /// `run` cells wide **along the run axis**, so the piece is sized to exactly that many divisions
    /// across — `grid::cells` is what decides, and a lattice with a cell outside its own divisions
    /// is refused at load. The claim is one-axis on purpose: every cell written here sits at
    /// `(x, 0, 0)`, and that fills the *whole* face only because the fixture leaves
    /// `Lattice::face_bands` at 1. Set that and these cells are still legal, but they are one band
    /// of several rather than the face.
    pub fn authored_lattice(mut self, id: &str, pack: &str, token: &str, run: u32) -> Fixture {
        // **Zero is the caller's bug, and it surfaces four layers away.** `run: 0` writes
        // `footprint: Some((0, 1))` and an empty cell list, and the project then fails to open with
        // a load error about a descriptor — pointing at the RON this helper generated rather than at
        // the test that asked for nothing.
        assert!(
            run > 0,
            "authored_lattice was asked for a run of 0 cells: that writes a piece 0 m wide and a \
             lattice with nothing in it, and the refusal then arrives from `Project::open` instead \
             of from here. Name the number of cells the test is about."
        );
        self = self.sized_descriptor(id, pack, run as f32 * bevy_grimoire_core::grid::SNAP, 1.0);
        if let Some(last) = self.descriptors.last_mut() {
            let was = "subgrid: None";
            assert!(
                last.contains(was),
                "the fixture's descriptor shape changed under this helper"
            );
            let cells: Vec<String> = (0..run)
                .map(|x| {
                    format!(
                        "( at: ({x}, 0, 0), solid: true, edge: Some(\"{token}\") )"
                    )
                })
                .collect();
            *last = last.replace(was, &format!("subgrid: Some(( cells: [{}] ))", cells.join(", ")));
        }
        self
    }

    /// **A piece that must sit on something** — `mount: OnSurface(class)`.
    ///
    /// The shape every fixture, lamp and screen in a real kit has, and the one a tile refuses when
    /// nothing under it offers the class.
    pub fn mounted_descriptor(mut self, id: &str, pack: &str, class: &str) -> Fixture {
        self = self.descriptor(id, pack);
        if let Some(last) = self.descriptors.last_mut() {
            let was = "mount: Some(OnFloor)";
            assert!(
                last.contains(was),
                "the fixture's descriptor shape changed under this helper"
            );
            *last = last.replace(
                was,
                &format!("mount: Some(OnSurface( class: \"{class}\" ))"),
            );
        }
        self
    }

    /// **A piece that may sit on a support surface or on the floor** — issue #1's
    /// `mount: Some(OnSurfaceOrFloor( class: "..." ))`.
    ///
    /// The third member of the surface family: `surface_descriptor` offers a top,
    /// `mounted_descriptor` may only rest on one, and this may do either. A test that drops it in two
    /// places is the only way to prove the two seatings are one mount rather than two descriptors.
    pub fn two_way_descriptor(mut self, id: &str, pack: &str, class: &str) -> Fixture {
        self = self.descriptor(id, pack);
        if let Some(last) = self.descriptors.last_mut() {
            let was = "mount: Some(OnFloor)";
            assert!(
                last.contains(was),
                "the fixture's descriptor shape changed under this helper"
            );
            *last = last.replace(
                was,
                &format!("mount: Some(OnSurfaceOrFloor( class: \"{class}\" ))"),
            );
        }
        self
    }

    /// **A piece that offers a surface** — a desk, a table, a shelf.
    ///
    /// The other half of the pair: without one in the library, a refusal about a missing host has
    /// nothing true to point at.
    pub fn surface_descriptor(mut self, id: &str, pack: &str, class: &str) -> Fixture {
        self = self.descriptor(id, pack);
        if let Some(last) = self.descriptors.last_mut() {
            let was = "offers: ( surfaces: [], sockets: [] )";
            assert!(
                last.contains(was),
                "the fixture's descriptor shape changed under this helper"
            );
            *last = last.replace(
                was,
                &format!("offers: ( surfaces: [\"{class}\"], sockets: [] )"),
            );
        }
        self
    }

    /// **A `slot` token, so a tile can declare a hole.**
    ///
    /// `new` writes an empty slot axis, which is the honest default: a project that has not grown
    /// one refuses `Shift+Enter` by name rather than inventing a token. A test about holes needs
    /// one, and rewriting the file is how any other axis would be set too.
    /// **Declare edge tokens**, so a test can exercise both sides of the derivation's commit door.
    ///
    /// The fixture ships one token, `wall`, which is deliberately *not* what
    /// `adjacency::derive_edges` names — so the refusal branch is the default and a test has to opt
    /// in to the accepting one.
    pub fn edge_tokens(self, names: &[&str]) -> Fixture {
        let at = self.dir.join("assets/grimoire/vocab.ron");
        let was =
            std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("cannot read {at:?}: {e}"));
        let one = r#"edge: (tokens: [( name: "wall", note: "a solid run-face" )]),"#;
        assert!(
            was.contains(one),
            "the fixture's edge axis must be the shipped one, or this is a no-op"
        );
        let mut rows = vec![r#"( name: "wall", note: "a solid run-face" )"#.to_owned()];
        for n in names {
            rows.push(format!(r#"( name: "{n}", note: "derived from the mesh" )"#));
        }
        let full = format!("edge: (tokens: [{}]),", rows.join(", "));
        std::fs::write(&at, was.replace(one, &full))
            .unwrap_or_else(|e| panic!("cannot write {at:?}: {e}"));
        self
    }

    /// **Declare `look` tokens**, so a test can drive the tag block with the words a shipped guide
    /// actually names.
    ///
    /// The fixture ships one, `plain`, which is enough for every test that only needs *a* token on
    /// this axis. `the_label_a_mesh_script_s_objective_steps_can_actually_be_walked` needs the two
    /// the script names, because a drive test that substituted its own words would prove the block
    /// works and not that the script can be followed — and the script's payload is exactly what was
    /// wrong with it.
    pub fn look_tokens(self, names: &[&str]) -> Fixture {
        let at = self.dir.join("assets/grimoire/vocab.ron");
        let was =
            std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("cannot read {at:?}: {e}"));
        let one = r#"look: (tokens: [( name: "plain", note: "unremarkable" )]),"#;
        assert!(
            was.contains(one),
            "the fixture's look axis must be the shipped one, or this is a no-op"
        );
        let mut rows = vec![r#"( name: "plain", note: "unremarkable" )"#.to_owned()];
        for n in names {
            rows.push(format!(r#"( name: "{n}", note: "how it reads" )"#));
        }
        let full = format!("look: (tokens: [{}]),", rows.join(", "));
        std::fs::write(&at, was.replace(one, &full))
            .unwrap_or_else(|e| panic!("cannot write {at:?}: {e}"));
        self
    }

    /// **Declare `kind` tokens**, so a test can move a piece's kind from one chip to another.
    ///
    /// The fixture ships `prop` alone, which is enough to observe *a* kind chip but not enough to
    /// watch a click move the light from one chip to the next — the question B-047 asks.
    pub fn kind_tokens(self, names: &[&str]) -> Fixture {
        let at = self.dir.join("assets/grimoire/vocab.ron");
        let was =
            std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("cannot read {at:?}: {e}"));
        let one = r#"kind: (tokens: [( name: "prop", note: "a thing" )]),"#;
        assert!(
            was.contains(one),
            "the fixture's kind axis must be the shipped one, or this is a no-op"
        );
        let mut rows = vec![r#"( name: "prop", note: "a thing" )"#.to_owned()];
        for n in names {
            rows.push(format!(r#"( name: "{n}", note: "a kind" )"#));
        }
        let full = format!("kind: (tokens: [{}]),", rows.join(", "));
        std::fs::write(&at, was.replace(one, &full))
            .unwrap_or_else(|e| panic!("cannot write {at:?}: {e}"));
        self
    }

    /// **A `kind` token that brings an `effects` token with it** — the *derived* half of `effects`.
    ///
    /// `Vocabularies::implied_effects` resolves the implication, `Vocabularies::masks` folds it into
    /// the mask, and `labels::settle_library` writes it into the stored list at open. The fixture's
    /// own vocabulary declares no implication at all, which is right for every other test here and
    /// is exactly why the settling had nothing to be driven against — the one test that reached it
    /// read the shipped corpus.
    ///
    /// The effect token is declared on the `effects` axis in the same breath, because a vocabulary
    /// whose `kind` token implies a word the `effects` axis does not hold **does not load**:
    /// `Vocabularies::validate_tables` calls `validate_implications`, and that is the door. Writing
    /// the implication alone would fail at `Project::open` with a refusal about the vocabulary
    /// rather than at the assertion the test is about.
    ///
    /// # It used to name the wrong door
    ///
    /// This said `implied_effects` refuses it. It did once — the check lived inside the resolver and
    /// so ran once per descriptor, which meant an invented implied token loaded clean for ever and
    /// exploded the first time an author used that kind. The refusal moved to parse, and
    /// `implied_effects` is infallible now; a sentence still pointing at the resolver would send the
    /// next reader looking for a `Result` that no longer exists.
    pub fn kind_implies(self, effect: &str) -> Fixture {
        let at = self.dir.join("assets/grimoire/vocab.ron");
        let was =
            std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("cannot read {at:?}: {e}"));
        let kind = r#"kind: (tokens: [( name: "prop", note: "a thing" )]),"#;
        let effects = r#"effects: (tokens: [( name: "inert", note: "does nothing" )]),"#;
        assert!(
            was.contains(kind) && was.contains(effects),
            "the fixture's kind and effects axes must be the shipped ones, or this is a no-op"
        );
        let text = was
            .replace(
                kind,
                &format!(
                    r#"kind: (tokens: [( name: "prop", note: "a thing", implies: ["{effect}"] )]),"#
                ),
            )
            .replace(
                effects,
                &format!(
                    r#"effects: (tokens: [( name: "inert", note: "does nothing" ), ( name: "{effect}", note: "follows the kind" )]),"#
                ),
            );
        std::fs::write(&at, text).unwrap_or_else(|e| panic!("cannot write {at:?}: {e}"));
        self
    }

    pub fn slot_token(self, name: &str) -> Fixture {
        let at = self.dir.join("assets/grimoire/vocab.ron");
        let was =
            std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("cannot read {at:?}: {e}"));
        let empty = "slot: (tokens: []),";
        assert!(
            was.contains(empty),
            "the fixture's slot axis must start empty, or this is a no-op"
        );
        let full = format!("slot: (tokens: [( name: \"{name}\", note: \"a hole\" )]),");
        std::fs::write(&at, was.replace(empty, &full))
            .unwrap_or_else(|e| panic!("cannot write {at:?}: {e}"));
        self
    }

    /// **A pack of meshes on disk**, none of them in the library — i.e. import candidates.
    pub fn pack(self, pack: &str, meshes: &[&str]) -> Fixture {
        let at = self.dir.join("assets").join(pack);
        std::fs::create_dir_all(&at).unwrap_or_else(|e| panic!("{e}"));
        for m in meshes {
            std::fs::write(at.join(format!("{m}.glb")), glb(0.6, 0.5, 0.6))
                .unwrap_or_else(|e| panic!("{e}"));
        }
        self
    }

    /// **A piece that still owes judgement** — no `effects`, no `look`, no note.
    ///
    /// The `Fixture` default is fully judged, because most tests are about placing and composing and
    /// an unjudged piece is invisible to the Tiles palette. This is the other entity: what the VLM
    /// batch is FOR, and what `the_tiles_palette_lists_only_judged_meshes` contrasts against.
    pub fn unjudged_descriptor(mut self, id: &str, pack: &str) -> Fixture {
        self = self.descriptor(id, pack);
        if let Some(last) = self.descriptors.last_mut() {
            *last = last
                .replace(r#"effects: ["inert"],"#, "effects: [],")
                .replace(r#"look: ["plain"],"#, "look: [],")
                .replace(r#"note: Some("a fixture piece"),"#, "note: None,");
        }
        self
    }

    /// A library entry naming a mesh in `pack`. The mesh is written too, so the entry resolves.
    pub fn descriptor(self, id: &str, pack: &str) -> Fixture {
        self.sunk_descriptor(id, pack, 0.0)
    }

    /// The same, recessed into its own floor by `y_offset` metres — `bevy_grimoire_core::stack::datum`'s
    /// ordinary case, and the thing a backdrop has to sit under.
    pub fn sunk_descriptor(mut self, id: &str, pack: &str, y_offset: f32) -> Fixture {
        self.descriptors
            .push(descriptor_row(&self.dir, id, pack, y_offset));
        match self.ids.iter_mut().find(|(k, _)| k == DEFAULT_KIT) {
            Some((_, v)) => v.push(id.to_owned()),
            None => self
                .ids
                .push((DEFAULT_KIT.to_owned(), vec![id.to_owned()])),
        }
        self
    }

    /// A fully judged descriptor whose six fields were all written by `model` at `confidence`, a
    /// Rust variant name (`High`, `Medium` or `Low`). For tests that read the labels row.
    pub fn labelled_descriptor(self, id: &str, pack: &str, model: &str, confidence: &str) -> Fixture {
        self.labelled_by(
            id,
            pack,
            "(kind: Some(Model), effects: Some(Model), look: Some(Model))",
            &format!(r#"model: Some("{model}"), at: "2026-10-20", confidence: Some({confidence})"#),
        )
    }

    /// A library entry whose `labels` record is written verbatim: `by` is the per-axis author
    /// (RON for an `AxisOrigin`) and `rest` is the record's remaining fields. For the tests that
    /// need a human-confirmed axis or a record with no model, which `labelled_descriptor` cannot say.
    pub fn labelled_by(self, id: &str, pack: &str, by: &str, rest: &str) -> Fixture {
        let mut f = self.descriptor(id, pack);
        if let Some(last) = f.descriptors.last_mut() {
            *last = last.replace(
                "placement: ( rooms: [], group: None ),",
                &format!(
                    "placement: ( rooms: [], group: None ),\n            labels: Some((by: {by}, {rest})),"
                ),
            );
        }
        f
    }

    /// Place a descriptor already added by [`Fixture::descriptor`], minting `<id>@<n>`.
    pub fn place(self, id: &str, at: (f32, f32)) -> Fixture {
        let n = self.placements.len();
        self.place_as(&format!("{id}@{n}"), id, at)
    }

    /// Place one under an id you choose — for the tests that are about id minting.
    ///
    /// # A placement naming an id nothing carries is refused here
    ///
    /// `redraw_placements` resolves a row through `library.get(..)` and `continue`s when that is
    /// `None`, so a map placing `alpha/floor` in a project whose library carries `floor` draws
    /// **nothing at all** — silently, with no refusal anywhere. Seven badge fixtures were built that
    /// way (`Fixture::descriptor("floor", "alpha")` mints the id `floor`; `alpha` is only the mesh
    /// folder), and every geometry test standing on them was measuring an empty map while claiming
    /// to measure a populated one.
    ///
    /// So the fixture refuses instead. It is the one place that knows both halves — the ids declared
    /// and the ids placed — and a panic naming the id beats a green suite measuring nothing. Kit ids
    /// count too: [`Fixture::kit`] rows are as placeable as the root kit's.
    pub fn place_as(mut self, row: &str, id: &str, at: (f32, f32)) -> Fixture {
        let known: Vec<&str> = self
            .ids
            .iter()
            .flat_map(|(_, v)| v.iter().map(String::as_str))
            .collect();
        assert!(
            known.contains(&id),
            "this fixture places `{id}`, which no descriptor it carries is called. It carries \
             {known:?} — a placement naming an id nothing defines is dropped by \
             `redraw_placements` without a word, so the map draws empty and every assertion about \
             what is on it passes over nothing. Note that `descriptor(\"floor\", \"alpha\")` mints \
             the id `floor`: the pack is the mesh folder, never a namespace."
        );
        self.placements.push(format!(
            r#"        (
            id: "{row}",
            descriptor: "{id}",
            at: ({:.1}, {:.1}),
            yaw: 0.0,
            tip: (0, 0),
            lift: 0.0,
            on: None,
            owned: false,
            owned_because: None,
            note: None,
            patch: None,
        ),"#,
            at.0, at.1
        ));
        self
    }

    /// [`Fixture::place`] with an authored `paint`: the order the author set on the placement, which
    /// outranks the automatic draw order where two flat pieces overlap.
    pub fn place_painted(self, id: &str, at: (f32, f32), paint: i8) -> Fixture {
        let mut f = self.place(id, at);
        if let Some(last) = f.placements.last_mut() {
            *last = last.replacen(
                "lift: 0.0,",
                &format!("lift: 0.0,\n            paint: {paint},"),
                1,
            );
        }
        f
    }

    /// [`Fixture::place`] standing on `host`, a placement already added: the rider a stack is made of.
    /// A removal of the host has to take it too, or the map is left with a rider on nothing.
    pub fn place_on(self, id: &str, host: &str, at: (f32, f32)) -> Fixture {
        let mut f = self.place(id, at);
        if let Some(last) = f.placements.last_mut() {
            *last = last.replacen("on: None,", &format!("on: Some(\"{host}\"),"), 1);
        }
        f
    }

    /// **A reusable group of two members**, both descriptors already added.
    ///
    /// `Anchored` — it claims no tile, so it has no boundary for anything to abut and needs no
    /// derived edge interface. That is the simplest thing a stamp can be, which is what a test about
    /// stamping wants.
    /// A **bounded** group — one that claims a tile, and so the only kind with a lattice to seat in.
    ///
    /// A separate constructor rather than an option on the one below, because `Anchored` and
    /// `Bounded` are two different things a group can be and not two settings of one: an anchored
    /// group presents no interface and has no envelope to be seated inside, which is exactly what
    /// `compose::seated` refuses on.
    pub fn bounded_composition(
        self,
        id: &str,
        size: (f32, f32, f32),
        members: &[(&str, &str, (f32, f32))],
    ) -> Fixture {
        let envelope = format!(
            "Bounded( size: ({:.1}, {:.1}, {:.1}) )",
            size.0, size.1, size.2
        );
        self.composition_with(id, &envelope, members)
    }

    pub fn composition(self, id: &str, members: &[(&str, &str, (f32, f32))]) -> Fixture {
        self.composition_with(id, "Anchored", members)
    }

    fn composition_with(
        mut self,
        id: &str,
        envelope: &str,
        members: &[(&str, &str, (f32, f32))],
    ) -> Fixture {
        // **Sorted by member id**, which the schema requires rather than prefers: one group must
        // have one encoding, or two authors building the same thing produce diffs that differ
        // without meaning to. `Composition::validate_shape` refuses otherwise, and names the order.
        let mut members = members.to_vec();
        members.sort_by(|a, b| a.0.cmp(b.0));
        let rows: Vec<String> = members
            .iter()
            .map(|(member, descriptor, at)| {
                format!(
                    r#"                (
                    id: "{member}",
                    body: Descriptor( id: "{descriptor}", tip: (0, 0), on: None, patch: None ),
                    at: ({:.1}, {:.1}),
                    yaw: 0.0,
                    lift: 0.0,
                    of_fingerprint: None,
                    note: None,
                ),"#,
                    at.0, at.1
                )
            })
            .collect();
        self.compositions.push(format!(
            r#"        (
            id: "{id}",
            envelope: {envelope},
            note: None,
            members: [
{}
            ],
            locations: [],
        ),"#,
            rows.join("\n")
        ));
        self
    }

    /// **The project's `rules.ron`**, as the text a project would hold — written verbatim by
    /// [`Self::build`], so a test can also hand it something the loader must refuse.
    pub fn rules(mut self, text: &str) -> Fixture {
        self.rules = Some(text.to_owned());
        self
    }

    /// **The open map's edge**, in metres on both horizontal axes. Sixteen by default; a rule's
    /// example has to be exactly `size * cell`, so a test about rules states its own.
    pub fn side(mut self, metres: f32) -> Fixture {
        self.side = metres;
        self
    }

    /// **A pinned placement** — [`Self::place`] with `owned` set and the reason a pin must carry.
    pub fn place_owned(mut self, id: &str, at: (f32, f32)) -> Fixture {
        self = self.place(id, at);
        if let Some(last) = self.placements.last_mut() {
            assert!(
                last.contains("owned: false,") && last.contains("owned_because: None,"),
                "the fixture's placement shape changed under this helper"
            );
            *last = last
                .replace("owned: false,", "owned: true,")
                .replace("owned_because: None,", "owned_because: Some(\"pinned by the test\"),");
        }
        self
    }

    /// **Another map beside the one [`Self::build`] writes**, at `maps/<name>.map.ron` under `root` —
    /// for the rule whose example is a different map from the one open. Rows are `(descriptor, at,
    /// owned)`, named `<descriptor>@<n>`. The map is not checked against the library, because the
    /// reader of an example (`rules::learn`) never asks.
    pub fn write_map(root: &Path, name: &str, side: f32, rows: &[(&str, (f32, f32), bool)]) {
        let placements: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(n, (id, at, owned))| {
                let because = if *owned {
                    "Some(\"pinned by the test\")"
                } else {
                    "None"
                };
                format!(
                    "        (id: \"{id}@{n}\", descriptor: \"{id}\", at: ({:.1}, {:.1}), yaw: 0.0, \
                     tip: (0, 0), lift: 0.0, on: None, owned: {owned}, owned_because: {because}, \
                     note: None, patch: None),",
                    at.0, at.1
                )
            })
            .collect();
        let path = root
            .join("assets/grimoire/maps")
            .join(format!("{name}.map.ron"));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        }
        std::fs::write(
            &path,
            format!(
                "(\n    version: 3,\n    name: \"{name}\",\n    origin: (0.0, 0.0, 0.0),\n    \
                 bounds: ({side:?}, 3.0, {side:?}),\n    bash: None,\n    placements: [\n{}\n    ],\n    \
                 stamps: [],\n    locations: [],\n)",
                placements.join("\n")
            ),
        )
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
    }

    /// Write the library and the map, and hand back the root [`crate::harness::build_headless`] opens.
    pub fn build(self, map: &str) -> PathBuf {
        let grimoire = self.dir.join("assets/grimoire");

        // **The project root is not a kit.** `vocab.ron`, `kits.ron`, `compositions.ron` and
        // `maps/` are the project's; every library lives in a kit directory beside them. This used
        // to write a `library.ron` here, which is exactly the conflation the 2026-08-16 split undid.
        //
        // The unnamed descriptors go into `furniture` — the same name the shipped project uses for
        // the same set, so a fixture and the real thing agree about where flat ids live.
        let mut binds = vec![DEFAULT_KIT.to_owned()];
        binds.extend(self.kits.iter().map(|(n, _)| n.clone()));
        for (name, rows) in std::iter::once(&(DEFAULT_KIT.to_owned(), self.descriptors.clone()))
            .chain(self.kits.iter())
        {
            let dir = grimoire.join(name);
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
            std::fs::write(
                dir.join("library.ron"),
                format!(
                    "(\n    version: 1,\n    note: None,\n    descriptors: [\n{}\n    ],\n)",
                    rows.join("\n")
                ),
            )
            .unwrap_or_else(|e| panic!("{e}"));
            // *"A project states its policy, even when its policy is nothing"* — a kit missing
            // `project.ron` is not a kit with defaults, it is a kit the editor refuses.
            std::fs::write(
                dir.join("project.ron"),
                "(\n    version: 2,\n    note: None,\n    patches: [],\n)",
            )
            .unwrap_or_else(|e| panic!("{e}"));
        }

        // **Bound as their own names.** A fixture kit built with `Fixture::kit` may carry `site/*`
        // ids and be called something else — that is the re-skin shape — so the binding is verified
        // against the library at load and a mismatch is a refusal, not a silent re-point. Tests
        // that want the pair write their own `kits.ron` on top.
        let bind = binds
            .iter()
            .map(|n| {
                let ids = self
                    .ids
                    .iter()
                    .find(|(k, _)| k == n)
                    .map(|(_, v)| v.as_slice())
                    .unwrap_or(&[]);
                format!("(namespace: \"{}\", dir: \"{n}\")", namespace_of(n, ids))
            })
            .collect::<Vec<_>>()
            .join(", ");
        // **The declared combinations, in call order.** Empty is the ordinary fixture: every map
        // offers every bound kit, which is what `Map::bash: None` means.
        let bash = self
            .bashes
            .iter()
            .map(|(name, kits)| {
                let members = kits
                    .iter()
                    .map(|k| format!("\"{k}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("(name: \"{name}\", kits: [{members}])")
            })
            .collect::<Vec<_>>()
            .join(", ");
        std::fs::write(
            grimoire.join("kits.ron"),
            format!(
                "(version: {}, bind: [{bind}], bash: [{bash}], authoring: Some(\"{DEFAULT_KIT}\"))",
                bevy_grimoire_core::kits::KITS_VERSION
            ),
        )
        .unwrap_or_else(|e| panic!("{e}"));

        // **Absent is the ordinary project.** Only a test that is about rules writes the file.
        if let Some(text) = &self.rules {
            std::fs::write(grimoire.join("rules.ron"), text).unwrap_or_else(|e| panic!("{e}"));
        }
        let side = self.side;

        // **One collection, at the project root.** Its absence and an empty one mean the same
        // thing, so a project that stamps nothing writes no file.
        if !self.compositions.is_empty() {
            std::fs::write(
                grimoire.join("compositions.ron"),
                format!(
                    "(\n    version: 1,\n    note: None,\n    compositions: [\n{}\n    ],\n)",
                    self.compositions.join("\n")
                ),
            )
            .unwrap_or_else(|e| panic!("{e}"));
        }

        let maps = grimoire.join("maps");
        std::fs::create_dir_all(&maps).unwrap_or_else(|e| panic!("{maps:?}: {e}"));
        std::fs::write(
            maps.join(format!("{map}.map.ron")),
            format!(
                "(\n    version: 3,\n    name: \"{map}\",\n    origin: (0.0, 0.0, 0.0),\n    \
                 bounds: ({side:?}, 3.0, {side:?}),\n    bash: None,\n    placements: [\n{}\n    ],\n    \
                 stamps: [],\n    locations: [],\n)",
                self.placements.join("\n")
            ),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        self.dir
    }
}
