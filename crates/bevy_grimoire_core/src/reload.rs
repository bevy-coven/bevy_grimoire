//! **The reload contract: every kind of file the editor writes (and the hand-authored files it reads),
//! and what a running program does when one changes on disk.**
//!
//! Whether a running game or editor picks a file up, and how, used to be decided case by case: a
//! kind could ship with no reload path at all and nobody noticed (#198, series H1 to H7). This is the
//! one list. Each [`FileKind`] has a [`ReloadClass`] (what a change costs) and a [`Lifetime`] (what the
//! file belongs to), and two ratchet tests keep the list honest:
//!
//! - `every_written_kind_has_a_reload_class` (mapper): an operation declares the kinds it writes
//!   ([`FileKind::id`]), and one naming a kind this list does not have fails.
//! - `the_restart_list_only_shrinks` (here): every [`ReloadClass::Restart`] kind is on a checked-in
//!   list, so the debt can shrink but never silently grow.
//!
//! `docs/guide/reload-contract.md` renders [`table`], one row per kind, and a test keeps it current.
//!
//! # What a class means
//!
//! The class is the cost of a change **for the programs this repository ships** (the editor, and the
//! game-side `GrimoireLivePlugin`), the dearest of them when they differ. A kind nothing holds in
//! memory (read fresh at each use) is [`ReloadClass::Live`]: there is nothing to reload.
//!
//! # A request for a kind that was never loaded
//!
//! [`reload_for`] is the one policy. A change to a kind the program never loaded is the outcome
//! [`Reload::NotLoaded`], reported and nothing else: **never an implicit load**, because a load in the
//! middle of play is a hitch, and a program that did not ask for the file has no state to reload.

use std::fmt::Write as _;

/// **What a change to this kind costs a running program.** Ordered cheapest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReloadClass {
    /// Mutate in place: the new content replaces the old in memory, or nothing holds the old (the
    /// file is read fresh at each use).
    Live,
    /// Rebuild what was made from it: the entities, meshes or world derived from the file are
    /// dropped and made again, and the rest of the program carries on.
    Respawn,
    /// Cannot be hot: a running program that holds this kind keeps the old content until it is
    /// restarted. Every kind here is on the checked-in list the ratchet reads.
    Restart,
}

impl ReloadClass {
    /// The word the guide and the table use.
    pub fn word(self) -> &'static str {
        match self {
            ReloadClass::Live => "Live",
            ReloadClass::Respawn => "Respawn",
            ReloadClass::Restart => "Restart",
        }
    }
}

/// **What the file belongs to**, which decides how it reloads: a global file (a family settings file)
/// changes everything that reads it, a per-map one only its map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifetime {
    /// One per project, kit, species or crate: it outlives every map.
    Global,
    /// One per map, named after it: it is loaded with its map and goes with it.
    PerMap,
}

impl Lifetime {
    pub fn word(self) -> &'static str {
        match self {
            Lifetime::Global => "global",
            Lifetime::PerMap => "per map",
        }
    }
}

/// **Where a kind's path pattern starts.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    /// The project's directory (`assets/grimoire`, the one holding `kits.ron`). A kit's own files
    /// (its library, policy, terrain, water, forest, trees and `data.ron`) are under `<kit>/`, the
    /// authoring kit's directory.
    Grimoire,
    /// The game's root, the directory holding `assets/`.
    Root,
}

/// **One kind of file the editor writes.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileKind {
    /// The stable name an operation declares in `Operation::writes` and a test reads.
    pub id: &'static str,
    pub base: Base,
    /// Path under [`Self::base`]. `<name>` and `<kit>` each stand for one word of letters, digits and
    /// underscores (no `.` and no `/`); `<file>` for one file name of any characters. Empty for a kind
    /// with no fixed path: the files a game registers in `data.ron`. A kit's own files
    /// (`<kit>/library.ron`, `<kit>/terrain/...`) live in the authoring kit's directory.
    pub pattern: &'static str,
    /// The type it parses to.
    pub parses_to: &'static str,
    pub class: ReloadClass,
    pub lifetime: Lifetime,
    /// **Who writes it besides the operations the default build registers**, or empty when only those
    /// do: a person by hand, a screen, or an operation built only with a cargo feature. A kind no
    /// default-build operation writes must say who does, so the list cannot keep a kind nobody writes.
    pub other_writers: &'static str,
    /// How a change reaches a running program, and what is left unsaid by the class.
    pub note: &'static str,
}

/// One row of [`KINDS`], positional so the table reads as a table.
#[allow(clippy::too_many_arguments)]
const fn kind(
    id: &'static str,
    base: Base,
    pattern: &'static str,
    parses_to: &'static str,
    class: ReloadClass,
    lifetime: Lifetime,
    other_writers: &'static str,
    note: &'static str,
) -> FileKind {
    FileKind { id, base, pattern, parses_to, class, lifetime, other_writers, note }
}

use Base::{Grimoire, Root};
use Lifetime::{Global, PerMap};
use ReloadClass::{Live, Respawn, Restart};

/// **Every kind of file the editor writes, and the hand-authored files it reads.** One row each.
pub const KINDS: &[FileKind] = &[
    // ---- the project's own files ----
    kind("kits", Grimoire, "kits.ron", "Kits", Respawn, Global,
        "the project chooser (create, bind, unbind)",
        "The editor reopens the whole project; the game's `GrimoireLivePlugin` reloads it whole."),
    kind("vocab", Grimoire, "vocab.ron", "Vocabularies", Respawn, Global,
        "the project chooser (create), the vocabulary editor",
        "The editor re-resolves its libraries in place; the game reloads the project whole."),
    kind("library", Grimoire, "<kit>/library.ron", "Library", Respawn, Global,
        "the project chooser (create a kit)",
        "One per kit. The editor rereads the kit; the game rebuilds the maps that place a changed piece."),
    kind("policy", Grimoire, "<kit>/project.ron", "Policy", Respawn, Global,
        "the project chooser (create a kit, remove a policy), `policy::remove_patch`",
        "One per kit: its exclusions, patches and mounts. Reloads with the kit's library."),
    kind("rules", Grimoire, "rules.ron", "Rules", Respawn, Global, "",
        "The editor reopens the project; the game revalidates it against the library."),
    kind("compositions", Grimoire, "compositions.ron", "Compositions", Respawn, Global, "",
        "A stamp expands to rows, so the maps that stamp a changed composition are rebuilt."),
    kind("standards", Grimoire, "standards.ron", "Standards", Live, Global,
        "by hand",
        "Read fresh each time a kit is checked; the editor revalidates it when it changes."),
    kind("repetition", Grimoire, "repetition.ron", "RepetitionRules", Live, Global,
        "by hand",
        "Read fresh on each `repetition_vary` run; the editor revalidates it when it changes."),
    kind("checks", Grimoire, "checks.ron", "Checks", Live, Global,
        "by hand",
        "`Checks::for_project` reads it fresh on every run, so an edit takes effect at once."),
    kind("data_registry", Grimoire, "<kit>/data.ron", "DataFiles", Live, Global,
        "by hand",
        "The registry of a game's own tuning files, read at each `data_set`; nothing holds it."),
    kind("rigs", Grimoire, "rigs.ron", "Rigs", Live, Global,
        "the animation tab",
        "Read when the animation tab loads, and again at once when the file changes: the tab takes a good edit (`applied`), measures its rigs again and keeps the rig the author was on selected by name; a file that does not read is `refused` with the file and the field and the tab keeps what it holds; a file that is gone is `needs_restart`, because the tab has no manifest to take in its place."),
    kind("oplog", Grimoire, "oplog.jsonl", "JSON lines", Live, Global,
        "the dispatcher appends a line for every operation",
        "Appended by the dispatcher and read by the log tools, the undo walk and the steps feed; nothing holds it. The editor looks at it when its size or modification time moves and says in `grimoire_state.reloads` whether every line reads: a line that does not is named (`line 3.seq`), the readers skip it and an undo refuses until it is repaired."),
    kind("reference_images", Root, "references/<file>", "image bytes", Live, Global,
        "the editor, when a person drops an image on a map",
        "Copied into the project, where `reference_set` lists it and `grimoire_state.references` says whether it is there; nothing holds it. The editor looks at each picture the open map lists when its size or modification time moves and says in `grimoire_state.reloads` whether it is still the picture its name says: `refused` with the file and the header field (`signature`, `format`, `header`, `width` or `height`) when it is not, `applied` once it is again."),
    // ---- a map's own files ----
    kind("map", Grimoire, "maps/<name>.map.ron", "Map", Respawn, PerMap,
        "the project chooser (create, settings, rename)",
        "The editor adopts it into the open map, or keeps what it had when it does not parse; the game respawns the placements that changed."),
    kind("notes", Grimoire, "maps/<name>.notes.ron", "Notes", Live, PerMap, "",
        "The editor rereads it when its modification time moves; the game has no reader."),
    kind("plan", Grimoire, "maps/<name>.plan.ron", "Plan", Live, PerMap, "",
        "Read on demand by `plan_set` and the plan view."),
    kind("references", Grimoire, "maps/<name>.references.ron", "References", Live, PerMap, "",
        "Read on demand by `reference_set` and the notes."),
    kind("relations", Grimoire, "maps/<name>.relations.ron", "Relations", Live, PerMap,
        "by hand",
        "Read on demand by `relations_solve`."),
    kind("terrain", Grimoire, "<kit>/terrain/<name>.ron", "Heightfield", Respawn, PerMap,
        "`terrain_preset` (only in a build with the elements feature)",
        "The editor redraws the ground when the file's modification time moves. A file that does not parse is refused with its field named, and what is drawn stays; with nothing drawn yet, no ground is drawn."),
    kind("water_plan", Grimoire, "<kit>/water/<name>.ron", "WaterPlan", Live, PerMap, "",
        "The plan `water_run` reads; nothing draws it directly."),
    kind("water_field", Grimoire, "<kit>/water/<name>.field.ron", "WaterField", Respawn, PerMap, "",
        "The editor redraws the water surface when the file's modification time moves. A refused file keeps the water drawn before it."),
    kind("volume", Grimoire, "<kit>/volume/<name>.ron", "Volume", Respawn, PerMap, "",
        "The editor rebuilds the carved ground when the file's modification time moves. A refused file keeps the ground drawn before it."),
    kind("forest", Grimoire, "<kit>/forest/<name>.ron", "Forest", Respawn, PerMap, "",
        "The editor respawns the trees when the file's modification time moves."),
    kind("people", Grimoire, "<kit>/people/<name>.ron", "People", Live, PerMap, "",
        "Read at each `people_run`; nothing holds it."),
    kind("scripts", Grimoire, "<kit>/scripts/<name>.ron", "Scripts", Live, PerMap, "",
        "The map's triggers, read when they are run; nothing holds it."),
    // ---- shared by maps ----
    kind("trees", Grimoire, "<kit>/trees/<name>.tree.ron", "TreeParams", Respawn, Global, "",
        "A species a forest uses: the editor regrows every tree of it when the file's modification time moves, and a file that does not read leaves them as the last good one drew them."),
    // ---- the game root's files ----
    kind("family", Root, "family/<name>.ron", "a family crate's settings", Live, Global, "",
        "One file per family crate: the editor applies it in place, polled every 0.1 s. The game-side `GrimoireLivePlugin` does not watch `<root>/family/`."),
    kind("family_list", Root, "family.ron", "Family", Live, Global, "by hand",
        "Which family crates the project uses, read when the editor needs the list; nothing holds it."),
    kind("editor_settings", Grimoire, "editor.ron", "EditorSettings", Live, Global, "by hand",
        "The project layer of the editor's settings, laid over the user's `<config_dir>/Grimoire/settings.ron` key by key. The editor looks at it while it runs and applies it in place; a file that does not read keeps the last good one and is refused in `grimoire_state.reloads`. The game has no reader."),
    kind("agent", Root, "agent.ron", "AgentSettings", Live, Global, "by hand",
        "The note agent's model and prompt, read each time a note is sent; nothing holds it."),
    kind("play", Root, "play.ron", "PlayConfig", Live, Global, "by hand",
        "How the editor launches the game, read each time `play` runs; nothing holds it."),
    kind("destructible_marks", Root, "destructible.ron", "Marks", Live, Global,
        "`destructible_set` (only in a build with the carnage feature)",
        "Read at each `destructible_set` and `destructible_hit`; nothing holds it. A file that does not read refuses both, naming the file and the field, and is left as it is."),
    kind("destructible_bake", Root, "destructible/<name>.bake.ron", "a baked fracture", Live, Global,
        "`destructible_set` and `destructible_hit` (only in a build with the carnage feature)",
        "A cache the next hit rebuilds when it is absent or stale (another mesh or other cut settings). A file that does not read at all is refused, naming the file and the field, and is left as it is: remove it to bake afresh."),
    kind("game_data", Root, "", "the game's own RON", Live, Global, "",
        "Written by `data_set` into files the game owns, and followed by a running game that registers a reader for one (`bevy_grimoire::live::GrimoireLiveApp::reload_data_file`): about a second after the file's last write it is checked against its `data.ron` entry and handed to the game's own parse, and the game's verdict (`.live/game.ron`) says `applied`, or `refused` with the file and the leaf and the old value kept. The editor looks at each file `data.ron` lists when its size or modification time moves and says in `grimoire_state.reloads` whether it still reads: `refused` with the file and the leaf when it does not (a bounded leaf missing, not a number or outside its bounds, or a file cut off), `applied` when it does."),
];

/// **The kind with this id.**
pub fn kind_by_id(id: &str) -> Option<&'static FileKind> {
    KINDS.iter().find(|k| k.id == id)
}

/// **Whether a path segment pattern matches one segment.** `<word>` stands for one run of letters,
/// digits and underscores: no `.` and no `/`, so `<name>.ron` never matches `town.field.ron`. `<file>`
/// is the one exception: any non-empty file name.
fn segment_matches(pattern: &str, segment: &str) -> bool {
    let Some(open) = pattern.find('<') else {
        return pattern == segment;
    };
    let Some(close) = pattern[open..].find('>').map(|c| c + open) else {
        return false;
    };
    let (head, tail) = (&pattern[..open], &pattern[close + 1..]);
    let Some(rest) = segment.strip_prefix(head).and_then(|s| s.strip_suffix(tail)) else {
        return false;
    };
    let any = &pattern[open..=close] == "<file>";
    !rest.is_empty() && (any || rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

impl FileKind {
    /// Whether `path`, written with `/` and relative to [`Self::base`], is a file of this kind.
    pub fn matches(&self, path: &str) -> bool {
        if self.pattern.is_empty() {
            return false;
        }
        let mut want = self.pattern.split('/');
        let mut have = path.split('/');
        loop {
            match (want.next(), have.next()) {
                (None, None) => return true,
                (Some(w), Some(h)) if segment_matches(w, h) => {}
                _ => return false,
            }
        }
    }
}

/// **The kind a changed file is**, given its path relative to `base`, or `None` for a file that is not
/// one of ours. Patterns are unambiguous (a test proves it), so at most one kind answers.
pub fn kind_of(base: Base, path: &str) -> Option<&'static FileKind> {
    KINDS.iter().find(|k| k.base == base && k.matches(path))
}

/// **What a running program does about a change to a kind.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reload {
    /// Replace the content in place.
    InPlace,
    /// Rebuild what was derived from the file.
    Respawn,
    /// The program holds the kind and cannot take the change: say so, and keep the old content.
    NeedsRestart,
    /// The program never loaded this kind, so there is nothing to reload. **Reported, never an
    /// implicit load.**
    NotLoaded,
}

/// **The policy: what a change to `kind` means for a program that has (or has not) loaded it.**
pub fn reload_for(kind: &FileKind, loaded: bool) -> Reload {
    if !loaded {
        return Reload::NotLoaded;
    }
    match kind.class {
        Live => Reload::InPlace,
        Respawn => Reload::Respawn,
        Restart => Reload::NeedsRestart,
    }
}

/// **The kinds with this class**, for the ratchet and the guide.
pub fn kinds_of_class(class: ReloadClass) -> impl Iterator<Item = &'static FileKind> {
    KINDS.iter().filter(move |k| k.class == class)
}

/// **The guide's table**, one row per kind. `writers` names the operations that write a kind (the
/// registry's `Operation::writes`, which this crate cannot see), so the table is rendered from the
/// code and a test compares it with `docs/guide/reload-contract.md`.
pub fn table(writers: &dyn Fn(&str) -> Vec<String>) -> String {
    let mut out = String::new();
    out.push_str("| kind | path | parses to | class | lifetime | written by | how a change lands |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for k in KINDS {
        let base = match k.base {
            Base::Grimoire => "<grimoire>",
            Base::Root => "<root>",
        };
        let path = if k.pattern.is_empty() {
            "(any file registered in `data.ron`)".to_owned()
        } else {
            format!("`{base}/{}`", k.pattern)
        };
        let ops = writers(k.id);
        let mut by = if ops.is_empty() {
            String::new()
        } else {
            ops.iter().map(|o| format!("`{o}`")).collect::<Vec<_>>().join(", ")
        };
        if !k.other_writers.is_empty() {
            if !by.is_empty() {
                by.push_str("; also ");
            }
            by.push_str(k.other_writers);
        }
        let _ = writeln!(
            out,
            "| `{}` | {path} | {} | {} | {} | {} | {} |",
            k.id,
            k.parses_to,
            k.class.word(),
            k.lifetime.word(),
            by,
            k.note
        );
    }
    out
}

/// **One setting that always needs a type change and a rebuild** (#202, H5).
///
/// The hot-reload series moves what can be data out of Rust types, so a change to it reloads. What
/// cannot move is listed here, once, with the reason, so "why does this need a rebuild" has an answer
/// in one place and a setting nobody can reload is a fact on a list and not a surprise. These are not
/// file kinds, so [`ReloadClass::Restart`] does not hold them: a `Restart` kind is debt that a reload
/// path can pay, and none of these has a file to reload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rebuild {
    /// A stable word for the entry.
    pub id: &'static str,
    /// What changes when it changes, and where it lives.
    pub what: &'static str,
    /// **Why a file cannot hold it.** The reason, not the symptom.
    pub reason: &'static str,
}

/// **What needs a rebuild, and why.** Short on purpose: an entry that gains a file form leaves this list
/// and joins [`KINDS`] in the commit that gives it a reload path.
pub const REBUILD: &[Rebuild] = &[
    Rebuild {
        id: "family_crates",
        what: "Which family crates a build of the editor can host: one cargo feature each.",
        reason: "A family crate is compiled in, as an optional dependency pinned by git revision. `family.ron` says which a project uses and reloads live, but a build without the crate cannot run it: the editor names the entry as a problem until it is built with the feature.",
    },
    Rebuild {
        id: "operations",
        what: "The editor's verbs: the registered operations, each a Rust type with its parameters, preview, undo and the kinds it writes.",
        reason: "An operation is code. A new verb, or a changed one, is a type change; the MCP tool list and the parity test are read from the registry, so they follow the next build and no earlier.",
    },
    Rebuild {
        id: "file_kinds",
        what: "The set of file kinds the editor reads and writes, and the Rust type each parses to (`reload::KINDS`).",
        reason: "A kind needs a type that reads it, a class and a reader that applies it. A file cannot teach a running program to read a file of a new shape.",
    },
    Rebuild {
        id: "format_shapes",
        what: "The fields of every saved format, and its version number.",
        reason: "A field is a field of a Rust type. Adding one with a default keeps old files reading (`tests/old_data` freezes them), and a change that cannot is a new version with a migration. Either way the type changes and the build with it.",
    },
    Rebuild {
        id: "algorithms",
        what: "The solvers: the constraint solver, wave function collapse, the relation solver and the sight and reach checks.",
        reason: "An algorithm is code. Its tuning (a radius, a weight, a seed) is data where a file already carries it, such as `repetition.ron` and `standards.ron`; the search itself is not.",
    },
    Rebuild {
        id: "editor_surface",
        what: "The editor's own panels, colours and key chords.",
        reason: "The panels are spawned by systems, the colours are constants and the chords are a table in code; no file reads them today. They could take a file form, and until one does a change is a rebuild.",
    },
    Rebuild {
        id: "game_systems",
        what: "A game's own systems, components and schedule, and the host that runs its scripts.",
        reason: "A trigger's script reloads (`scripts`), but the interpreter that runs it, the components it acts on and the order systems run in are Rust. Behaviour that can be a script or a row of data is moved there under #202.",
    },
];

/// **The guide's second table**, one row per entry of [`REBUILD`], rendered like [`table`] so a test can
/// compare it with `docs/guide/reload-contract.md`.
pub fn rebuild_table() -> String {
    let mut out = String::new();
    out.push_str("| setting | what | why a file cannot hold it |\n");
    out.push_str("|---|---|---|\n");
    for r in REBUILD {
        let _ = writeln!(out, "| `{}` | {} | {} |", r.id, r.what, r.reason);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_pattern_matches_one_word() {
        assert!(segment_matches("<name>.map.ron", "town.map.ron"));
        assert!(!segment_matches("<name>.ron", "town.field.ron"));
        assert!(!segment_matches("<name>.map.ron", ".map.ron"));
        assert!(segment_matches("kits.ron", "kits.ron"));
        assert!(!segment_matches("kits.ron", "kits.ron.tmp"));
    }

    #[test]
    fn a_changed_path_is_classified_by_its_pattern() {
        let map = kind_of(Base::Grimoire, "maps/town.map.ron").expect("a map is a kind");
        assert_eq!((map.id, map.class, map.lifetime), ("map", Respawn, PerMap));
        assert_eq!(kind_of(Base::Grimoire, "site/water/lake.field.ron").map(|k| k.id), Some("water_field"));
        assert_eq!(kind_of(Base::Grimoire, "site/water/lake.ron").map(|k| k.id), Some("water_plan"));
        assert_eq!(kind_of(Base::Grimoire, "site/library.ron").map(|k| k.id), Some("library"));
        assert_eq!(kind_of(Base::Root, "references/Sketch v2.JPG").map(|k| k.id), Some("reference_images"));
        assert_eq!(kind_of(Base::Root, "family/weather.ron").map(|k| k.id), Some("family"));
        assert_eq!(kind_of(Base::Grimoire, "maps/town.map.ron.tmp"), None);
        assert_eq!(kind_of(Base::Grimoire, ".live/game.ron"), None);
    }

    #[test]
    fn a_request_for_a_kind_never_loaded_is_reported_not_loaded() {
        for k in KINDS {
            assert_eq!(reload_for(k, false), Reload::NotLoaded, "{} must not load itself", k.id);
        }
        let by = |id| reload_for(kind_by_id(id).unwrap(), true);
        assert_eq!(by("family"), Reload::InPlace);
        assert_eq!(by("map"), Reload::Respawn);
        assert_eq!(by("rigs"), Reload::InPlace);
        assert_eq!(by("game_data"), Reload::InPlace);
        // No kind is `Restart` any more (#263), so the mapping is pinned on a row of its own.
        let held = kind("held", Grimoire, "held.ron", "Held", Restart, Global, "", "");
        assert_eq!(reload_for(&held, true), Reload::NeedsRestart);
        assert_eq!(reload_for(&held, false), Reload::NotLoaded);
    }
}
