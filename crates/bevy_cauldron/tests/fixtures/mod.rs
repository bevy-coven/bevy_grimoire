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

use bevy_grimoire_core::composition::{Body, Compositions, Envelope, Member};
use bevy_grimoire_core::descriptor::{
    Align, AxisOrigin, By, Confidence, Descriptor, Extent, LabelOrigin, Mount, Offers,
    SubCell, Subgrid,
};
use bevy_grimoire_core::kits::{Bash, Bind, Kits};
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{Map, Placed};
use bevy_grimoire_core::policy::Policy;
use bevy_grimoire_core::vocab::{Token, Vocabularies, Vocabulary};

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
    descriptors: Vec<Descriptor>,
    placements: Vec<Placed>,
    compositions: Vec<bevy_grimoire_core::composition::Composition>,
    /// **Kits beside the default one**, as `(directory name, descriptors)`. See [`Fixture::kit`].
    kits: Vec<(String, Vec<Descriptor>)>,
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
    /// **The project's rules**, written beside `kits.yaml` by [`Self::build`]. `None` writes no file,
    /// which is a project without rules. See [`Self::rules`].
    rules: Option<bevy_grimoire_core::rules::Rules>,
    /// A `rules.yaml` a test writes verbatim, because its subject is a file the loader must refuse.
    rules_text: Option<String>,
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

/// **One library entry, and the mesh it names**, written under `pack` inside `root`.
///
/// The file is named for the id's **last segment**, so `site/floor` writes `ozea/floor.glb` rather
/// than a `site/` subdirectory nobody asked for — the shape the shipped kits actually have, where
/// the namespace is in the id and never in the mesh path.
///
/// **A value, not a row of text.** The fixture used to format RON by hand, which meant every helper here
/// carried a `replace("footprint: Some((1.0, 1.0))", …)` that silently did nothing the day the shape moved.
/// A `Descriptor` with the field set cannot drift from the type, and the file is written by the one YAML
/// writer the editor itself uses.
///
/// Free rather than a method, because two callers want it: the root kit's builder pushes the entry onto its
/// own list, and [`Fixture::kit`] collects entries for a directory of its own.
fn descriptor_value(root: &Path, id: &str, pack: &str, y_offset: f32) -> Descriptor {
    let stem = id.rsplit('/').next().unwrap_or(id);
    let at = root.join("assets").join(pack);
    std::fs::create_dir_all(&at).unwrap_or_else(|e| panic!("cannot make {at:?}: {e}"));
    std::fs::write(at.join(format!("{stem}.glb")), glb(1.0, 0.5, 1.0))
        .unwrap_or_else(|e| panic!("cannot write {stem}.glb: {e}"));
    Descriptor {
        id: id.to_owned(),
        mesh: Some(format!("{pack}/{stem}.glb")),
        align: Align { y_offset: Some(y_offset), ..Align::default() },
        extent: Extent { footprint: Some((1.0, 1.0)), height: Some(0.5) },
        mount: Some(Mount::OnFloor),
        kind: vec!["prop".to_owned()],
        effects: vec!["inert".to_owned()],
        look: vec!["plain".to_owned()],
        note: Some("a fixture piece".to_owned()),
        ..Descriptor::default()
    }
}

/// **The fixture's vocabulary**: one token on each axis a fixture uses.
///
/// A value rather than a hand-written file, so a helper that declares another token edits the value it will
/// write instead of replacing a string that used to be in the file.
fn vocabulary() -> Vocabularies {
    let axis = |tokens: &[(&str, &str)]| Vocabulary {
        tokens: tokens
            .iter()
            .map(|(name, note)| Token { name: (*name).to_owned(), note: (*note).to_owned(), implies: Vec::new() })
            .collect(),
    };
    Vocabularies {
        version: bevy_grimoire_core::vocab::VOCAB_VERSION,
        kind: axis(&[("prop", "a thing")]),
        effects: axis(&[("inert", "does nothing")]),
        look: axis(&[("plain", "unremarkable")]),
        surfaces: axis(&[("worktop", "a top")]),
        capabilities: axis(&[]),
        edge: axis(&[("wall", "a solid run-face")]),
        slot: axis(&[]),
    }
}

/// **A fragment written in a project file's own spelling, read back as its type.**
///
/// The label helpers take their record in the reader's words — `(kind: Some(Human), look: Some(Model))` —
/// because that is what makes a test about *who wrote what* readable. Reading it back through the same reader
/// a file goes through is what keeps it honest: a fragment that does not read panics with the field it was
/// wrong at, rather than producing a descriptor with a silently empty record.
fn parse_fragment<T: serde::de::DeserializeOwned>(fragment: &str, what: &str) -> T {
    bevy_grimoire_core::yaml::from_str(fragment, what)
        .unwrap_or_else(|p| panic!("the fixture's {what} does not read: {}", p.line()))
}

/// **A `LabelOrigin` from its two fragments**: the per-axis author (a value) and the record's remaining
/// fields (a project file's lines).
fn parse_record(by: AxisOrigin, rest: &str) -> LabelOrigin {
    let text = format!("by: {by:?}\n{rest}\n");
    let _ = &text;
    // The fields are read one at a time, because the fragments are the reader's spelling and a record built
    // from a `format!` is a file that has to pass the reader twice.
    let model = field_of(rest, "model").map(|v| v.to_owned());
    let model_at = field_of(rest, "model_at").map(|v| v.to_owned());
    let confidence = field_of(rest, "confidence").map(confidence_of);
    let at = field_of(rest, "at").unwrap_or_default().to_owned();
    LabelOrigin { by, model, model_at, at, confidence }
}

/// **A quoted string field out of a fragment**, or `None` for a field that is absent or `None`.
fn field_of<'a>(rest: &'a str, name: &str) -> Option<&'a str> {
    let at = rest.find(&format!("{name}: "))? + name.len() + 2;
    let tail = rest[at..].trim_start();
    if tail.starts_with("None") {
        return None;
    }
    let inner = tail.strip_prefix("Some(")?;
    let body = inner.split(')').next()?;
    let quoted = body.trim();
    Some(quoted.strip_prefix('"')?.split('"').next()?)
}

/// **A `Confidence` from the variant name a test writes**: `High`, `Medium` or `Low`.
fn confidence_of(name: &str) -> Confidence {
    match name.trim() {
        "High" => Confidence::High,
        "Medium" => Confidence::Medium,
        "Low" => Confidence::Low,
        other => panic!("`{other}` is not a confidence (High, Medium or Low)"),
    }
}

/// **Write a fixture file through the one YAML writer**, which is the writer the editor itself uses. A
/// fixture written any other way is a second spelling, and a project file has one.
fn write_yaml<T: serde::Serialize>(path: &Path, value: &T) {
    let text = bevy_grimoire_core::yaml::to_string(value)
        .unwrap_or_else(|p| panic!("{}: {}", path.display(), p.line()));
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
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

        write_yaml(&grimoire.join("vocab.yaml"), &vocabulary());
        write_yaml(&grimoire.join("project.yaml"), &Policy::default());

        Fixture {
            dir,
            descriptors: Vec::new(),
            placements: Vec::new(),
            compositions: Vec::new(),
            kits: Vec::new(),
            ids: Vec::new(),
            bashes: Vec::new(),
            rules: None,
            rules_text: None,
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
            .map(|id| descriptor_value(&self.dir, id, pack, 0.0))
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
        let kits = Kits {
            version: bevy_grimoire_core::kits::KITS_VERSION,
            note: None,
            lattice: Default::default(),
            bind: binds
                .iter()
                .map(|(ns, dir)| Bind { namespace: (*ns).to_owned(), dir: (*dir).to_owned() })
                .collect(),
            bash: Vec::new(),
            authoring: Some(authoring.to_owned()),
        };
        write_yaml(&root.join("assets/grimoire/kits.yaml"), &kits);
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
            last.extent.footprint = Some((w, d));
        }
        self
    }

    /// **A descriptor of a stated height**, for a test about what stands in a frame.
    pub fn tall_descriptor(mut self, id: &str, pack: &str, h: f32) -> Fixture {
        self = self.descriptor(id, pack);
        if let Some(last) = self.descriptors.last_mut() {
            last.extent.height = Some(h);
        }
        self
    }

    /// **A flat piece with NO mesh**, drawn as a footprint marker — the shape of the shipped
    /// `town/road` and `town/field`: `mesh: None`, a stated footprint, a 0.05 m height, and no
    /// `y_offset`. Two of these placed over one another are the case where their top faces are
    /// coplanar and the editor has to decide which one draws.
    pub fn flat_descriptor(mut self, id: &str, w: f32, d: f32) -> Fixture {
        self.descriptors.push(Descriptor {
            id: id.to_owned(),
            mesh: None,
            extent: Extent { footprint: Some((w, d)), height: Some(0.05) },
            mount: Some(Mount::OnFloor),
            kind: vec!["prop".to_owned()],
            effects: vec!["inert".to_owned()],
            look: vec!["plain".to_owned()],
            note: Some("a flat fixture piece".to_owned()),
            ..Descriptor::default()
        });
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
    /// to be tested against. The round trip is real here despite the fixture writing the file: this
    /// emits a **value**, and what is under test is `policy::layered_library` + `Project::open`
    /// reading the YAML back. The two are not the same code.
    ///
    /// `run` cells wide **along the run axis**, so the piece is sized to exactly that many divisions
    /// across — `grid::cells` is what decides, and a lattice with a cell outside its own divisions
    /// is refused at load.
    pub fn authored_lattice(mut self, id: &str, pack: &str, token: &str, run: u32) -> Fixture {
        // **Zero is the caller's bug, and it surfaces four layers away.** `run: 0` writes a piece 0 m
        // wide and a lattice with nothing in it, and the project then fails to open with a load error
        // about a descriptor — pointing at the fixture rather than at the test that asked for nothing.
        assert!(
            run > 0,
            "authored_lattice was asked for a run of 0 cells: that writes a piece 0 m wide and a \
             lattice with nothing in it, and the refusal then arrives from `Project::open` instead \
             of from here. Name the number of cells the test is about."
        );
        self = self.sized_descriptor(id, pack, run as f32 * bevy_grimoire_core::grid::SNAP, 1.0);
        if let Some(last) = self.descriptors.last_mut() {
            last.subgrid = Some(Subgrid {
                cells: (0..run)
                    .map(|x| SubCell { at: (x, 0, 0), solid: true, edge: Some(token.to_owned()) })
                    .collect(),
            });
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
            last.mount = Some(Mount::OnSurface { class: class.to_owned() });
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
            last.mount = Some(Mount::OnSurfaceOrFloor { class: class.to_owned() });
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
            last.offers = Offers { surfaces: vec![class.to_owned()], ..Offers::default() };
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
        let mut vocab = vocabulary();
        for n in names {
            vocab.edge.tokens.push(Token { name: (*n).to_owned(), note: "derived from the mesh".to_owned(), implies: Vec::new() });
        }
        self.rewrite_vocabulary(&vocab)
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
        let mut vocab = vocabulary();
        for n in names {
            vocab.look.tokens.push(Token { name: (*n).to_owned(), note: "how it reads".to_owned(), implies: Vec::new() });
        }
        self.rewrite_vocabulary(&vocab)
    }

    /// **Declare `kind` tokens**, so a test can move a piece's kind from one chip to another.
    ///
    /// The fixture ships `prop` alone, which is enough to observe *a* kind chip but not enough to
    /// watch a click move the light from one chip to the next — the question B-047 asks.
    pub fn kind_tokens(self, names: &[&str]) -> Fixture {
        let mut vocab = vocabulary();
        for n in names {
            vocab.kind.tokens.push(Token { name: (*n).to_owned(), note: "a kind".to_owned(), implies: Vec::new() });
        }
        self.rewrite_vocabulary(&vocab)
    }

    /// **The fixture's vocabulary with `edit` applied**, written through the one YAML writer.
    ///
    /// One door for every axis helper: each used to read the file's text, replace a literal string and write
    /// it back, which is three places to keep in step with the spelling and silently a no-op the day the
    /// spelling moved. This edits the value the builder already holds.
    fn rewrite_vocabulary(self, vocab: &Vocabularies) -> Fixture {
        write_yaml(&self.dir.join("assets/grimoire/vocab.yaml"), vocab);
        self
    }

    /// **A `slot` token, so a tile can declare a hole.**
    ///
    /// `new` writes an empty slot axis, which is the honest default: a project that has not grown one
    /// refuses `Shift+Enter` by name rather than inventing a token.
    pub fn slot_token(self, name: &str) -> Fixture {
        let mut vocab = vocabulary();
        vocab.slot.tokens.push(Token { name: name.to_owned(), note: "a hole".to_owned(), implies: Vec::new() });
        self.rewrite_vocabulary(&vocab)
    }

    /// **A `kind` token that brings an `effects` token with it** — the *derived* half of `effects`.
    ///
    /// `Vocabularies::implied_effects` resolves the implication, `Vocabularies::masks` folds it into the mask,
    /// and `labels::settle_library` writes it into the stored list at open. The effect token is declared on
    /// the `effects` axis in the same breath, because a vocabulary whose `kind` token implies a word the
    /// `effects` axis does not hold **does not load** (`validate_implications`), and writing the implication
    /// alone would fail at `Project::open` with a refusal about the vocabulary rather than at the assertion
    /// the test is about.
    pub fn kind_implies(self, effect: &str) -> Fixture {
        let mut vocab = vocabulary();
        if let Some(prop) = vocab.kind.tokens.first_mut() {
            prop.implies = vec![effect.to_owned()];
        }
        vocab.effects.tokens.push(Token { name: effect.to_owned(), note: "follows the kind".to_owned(), implies: Vec::new() });
        self.rewrite_vocabulary(&vocab)
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
            last.effects.clear();
            last.look.clear();
            last.note = None;
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
        self.descriptors.push(descriptor_value(&self.dir, id, pack, y_offset));
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
        let by = AxisOrigin { kind: Some(By::Model), effects: Some(By::Model), look: Some(By::Model), ..AxisOrigin::default() };
        let labels = LabelOrigin {
            by,
            model: Some(model.to_owned()),
            model_at: None,
            at: "2026-10-20".to_owned(),
            confidence: Some(confidence_of(confidence)),
        };
        self.with_labels(id, pack, labels)
    }

    /// **A library entry whose `labels` record a test states in the file's own words.**
    ///
    /// `by` is an `AxisOrigin` as a project file writes one (`(kind: Some(Human), look: Some(Model))`) and
    /// `rest` is the record's remaining fields (`model: Some("gemma-x"), at: "2026-10-21", confidence:
    /// Some(Medium)`), for the tests that need a human-confirmed axis, a record with no model, or a model
    /// stored by its full path — none of which a single "written by the model" constructor can say.
    ///
    /// **The two fragments are parsed, not formatted into a file.** They are written in the reader's own
    /// spelling because that is what makes the test readable, so they are read back by the same reader a
    /// project file goes through — and a fragment that does not read is a panic here rather than a descriptor
    /// with a silently empty record.
    pub fn labelled_by(self, id: &str, pack: &str, by: &str, rest: &str) -> Fixture {
        let by: AxisOrigin = parse_fragment(by, "the per-axis author");
        let labels = parse_record(by, rest);
        self.with_labels(id, pack, labels)
    }

    /// **The piece with `labels` set**, which is the one place a record reaches a descriptor.
    fn with_labels(self, id: &str, pack: &str, labels: LabelOrigin) -> Fixture {
        let mut f = self.descriptor(id, pack);
        if let Some(last) = f.descriptors.last_mut() {
            last.labels = Some(labels);
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
        self.placements.push(Placed {
            id: row.to_owned(),
            descriptor: id.to_owned(),
            at,
            ..Placed::default()
        });
        self
    }

    /// [`Fixture::place`] with an authored `paint`: the order the author set on the placement, which
    /// outranks the automatic draw order where two flat pieces overlap.
    pub fn place_painted(self, id: &str, at: (f32, f32), paint: i8) -> Fixture {
        let mut f = self.place(id, at);
        if let Some(last) = f.placements.last_mut() {
            last.paint = paint;
        }
        f
    }

    /// [`Fixture::place`] standing on `host`, a placement already added: the rider a stack is made of.
    /// A removal of the host has to take it too, or the map is left with a rider on nothing.
    pub fn place_on(self, id: &str, host: &str, at: (f32, f32)) -> Fixture {
        let mut f = self.place(id, at);
        if let Some(last) = f.placements.last_mut() {
            last.on = Some(host.to_owned());
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
        self.composition_with(id, Envelope::Bounded { size }, members)
    }

    pub fn composition(self, id: &str, members: &[(&str, &str, (f32, f32))]) -> Fixture {
        self.composition_with(id, Envelope::Anchored, members)
    }

    fn composition_with(
        mut self,
        id: &str,
        envelope: Envelope,
        members: &[(&str, &str, (f32, f32))],
    ) -> Fixture {
        // **Sorted by member id**, which the schema requires rather than prefers: one group must have one
        // encoding, or two authors building the same thing produce diffs that differ without meaning to.
        // `Composition::validate_shape` refuses otherwise, and names the order.
        let mut members = members.to_vec();
        members.sort_by(|a, b| a.0.cmp(b.0));
        self.compositions.push(bevy_grimoire_core::composition::Composition {
            id: id.to_owned(),
            envelope,
            members: members
                .iter()
                .map(|(member, descriptor, at)| Member {
                    id: (*member).to_owned(),
                    body: Body::Descriptor { id: (*descriptor).to_owned(), tip: (0, 0), on: None, patch: None },
                    at: *at,
                    yaw: 0.0,
                    lift: 0.0,
                    paint: 0,
                    of_fingerprint: None,
                    note: None,
                })
                .collect(),
            note: None,
            locations: Vec::new(),
        });
        self
    }

    /// **The project's `rules.ron`**, as the text a project would hold — written verbatim by
    /// [`Self::build`], so a test can also hand it something the loader must refuse.
    /// **The project's `rules.yaml` from a value**, which is the ordinary case: a test that wants a rule
    /// states it, and the builder writes the file through the one YAML writer.
    pub fn rules(mut self, rules: bevy_grimoire_core::rules::Rules) -> Fixture {
        self.rules = Some(rules);
        self
    }

    /// **The project's `rules.yaml` from text**, for a test whose subject *is* a file the loader must
    /// refuse. The text is written verbatim, so it is not written through a writer that would have to read
    /// it first — which is the whole point of a malformed fixture.
    pub fn rules_text(mut self, text: &str) -> Fixture {
        self.rules_text = Some(text.to_owned());
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
            last.owned = true;
            last.owned_because = Some("pinned by the test".to_owned());
        }
        self
    }

    /// **Another map beside the one [`Self::build`] writes**, at `maps/<name>.map.ron` under `root` —
    /// for the rule whose example is a different map from the one open. Rows are `(descriptor, at,
    /// owned)`, named `<descriptor>@<n>`. The map is not checked against the library, because the
    /// reader of an example (`rules::learn`) never asks.
    pub fn write_map(root: &Path, name: &str, side: f32, rows: &[(&str, (f32, f32), bool)]) {
        // The map is not checked against the library, because the reader of an example (`rules::learn`)
        // never asks: it reads rows and their positions.
        let map = Map {
            version: bevy_grimoire_core::map::MAP_VERSION,
            name: name.to_owned(),
            origin: (0.0, 0.0, 0.0),
            bounds: (side, 3.0, side),
            placements: rows
                .iter()
                .enumerate()
                .map(|(n, (id, at, owned))| Placed {
                    id: format!("{id}@{n}"),
                    descriptor: (*id).to_owned(),
                    at: *at,
                    owned: *owned,
                    owned_because: (*owned).then(|| "pinned by the test".to_owned()),
                    ..Placed::default()
                })
                .collect(),
            ..Map::default()
        };
        let path = root.join("assets/grimoire/maps").join(format!("{name}.map.yaml"));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        }
        write_yaml(&path, &map);
    }

    /// Write the library and the map, and hand back the root [`crate::harness::build_headless`] opens.
    ///
    /// **Every file is written through the one YAML writer, from a value.** The builder used to format RON
    /// text by hand — nine `format!` calls, each a second spelling of a schema the crate already states — and
    /// every helper above reached into that text with `replace(…)`. A value cannot drift from the type, and it
    /// is what makes this builder the gate the cutover needed rather than the thing blocking it.
    pub fn build(self, map: &str) -> PathBuf {
        let grimoire = self.dir.join("assets/grimoire");

        // **The project root is not a kit.** `vocab.yaml`, `kits.yaml`, `compositions.yaml` and `maps/` are
        // the project's; every library lives in a kit directory beside them. The unnamed descriptors go into
        // `furniture` — the same name the shipped project uses for the same set.
        let mut binds = vec![DEFAULT_KIT.to_owned()];
        binds.extend(self.kits.iter().map(|(n, _)| n.clone()));
        for (name, descriptors) in std::iter::once(&(DEFAULT_KIT.to_owned(), self.descriptors.clone()))
            .chain(self.kits.iter())
        {
            let dir = grimoire.join(name);
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
            write_yaml(
                &dir.join("library.yaml"),
                &Library {
                    version: bevy_grimoire_core::library::LIBRARY_VERSION,
                    note: None,
                    descriptors: descriptors.clone(),
                },
            );
            // *"A project states its policy, even when its policy is nothing"* — a kit missing
            // `project.yaml` is not a kit with defaults, it is a kit the editor refuses.
            write_yaml(&dir.join("project.yaml"), &Policy::default());
        }

        // **Bound as their own names.** A fixture kit built with `Fixture::kit` may carry `site/*` ids and be
        // called something else — that is the re-skin shape — so the binding is verified against the library
        // at load and a mismatch is a refusal, not a silent re-point. Tests that want the pair write their
        // own `kits.yaml` on top.
        let bind = binds
            .iter()
            .map(|n| {
                let ids = self.ids.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_slice()).unwrap_or(&[]);
                Bind { namespace: namespace_of(n, ids), dir: n.clone() }
            })
            .collect();
        // **The declared combinations, in call order.** Empty is the ordinary fixture: every map offers every
        // bound kit, which is what `Map::bash: None` means.
        let bash = self
            .bashes
            .iter()
            .map(|(name, kits)| Bash { name: name.clone(), kits: kits.clone(), note: None })
            .collect();
        write_yaml(
            &grimoire.join("kits.yaml"),
            &Kits {
                version: bevy_grimoire_core::kits::KITS_VERSION,
                note: None,
                lattice: Default::default(),
                bind,
                bash,
                authoring: Some(DEFAULT_KIT.to_owned()),
            },
        );

        // **Absent is the ordinary project.** Only a test that is about rules writes the file.
        if let Some(rules) = &self.rules {
            write_yaml(&grimoire.join("rules.yaml"), rules);
        }
        if let Some(text) = &self.rules_text {
            std::fs::write(grimoire.join("rules.yaml"), text).unwrap_or_else(|e| panic!("{e}"));
        }

        // **One collection, at the project root.** Its absence and an empty one mean the same thing, so a
        // project that stamps nothing writes no file.
        if !self.compositions.is_empty() {
            write_yaml(
                &grimoire.join("compositions.yaml"),
                &Compositions {
                    version: bevy_grimoire_core::composition::COMPOSITIONS_VERSION,
                    note: None,
                    compositions: self.compositions.clone(),
                },
            );
        }

        let maps = grimoire.join("maps");
        std::fs::create_dir_all(&maps).unwrap_or_else(|e| panic!("{maps:?}: {e}"));
        write_yaml(
            &maps.join(format!("{map}.map.yaml")),
            &Map {
                version: bevy_grimoire_core::map::MAP_VERSION,
                name: map.to_owned(),
                origin: (0.0, 0.0, 0.0),
                bounds: (self.side, 3.0, self.side),
                placements: self.placements.clone(),
                ..Map::default()
            },
        );
        self.dir
    }
}

/// **A person's trust store of the test's own, trusting `root` as a whole project.** The editor starts a
/// program a project names only once a person has allowed it; a test that is about *starting* the program
/// says so with this, and points `Ctx::trust_file` or `EditorState::trust.file` at it. It sits beside the
/// project so no test reads or writes the real one in the user's data directory.
pub fn trusted_store(root: &Path) -> PathBuf {
    let file = root.join("trusted-by-the-test.yaml");
    let mut store = bevy_grimoire_core::trust::TrustStore::default();
    store.trust_project(root).unwrap_or_else(|e| panic!("{e}"));
    store.save(&file).unwrap_or_else(|e| panic!("{e}"));
    file
}
