//! **A reload validates before it applies, and a refusal names the file and the field** (#200, H3).
//!
//! For every kind of `bevy_grimoire_core::reload::KINDS` whose type this crate parses, three bad files
//! are refused: one that is not RON (corrupt), one whose field has the wrong type, one missing a field
//! the type requires. Each refusal names the file and, for the last two, the field. A kind with no
//! required field says so, and a file without it stays valid. The kinds this crate cannot parse are on
//! a checked-in list that may shrink and never silently grow.
//!
//! "Leaves the running state unchanged" is a property of the program that applies a file, not of a
//! pure check: the game's half is `crates/bevy_grimoire/tests/live_verdicts.rs`.

use std::path::Path;

use bevy_grimoire_core::destructible::{Bake, Chunk, DESTRUCTIBLE_VERSION, Marks};
use bevy_grimoire_core::map::{MAP_VERSION, Map};
use bevy_grimoire_core::notes::Notes;
use bevy_grimoire_core::plan::Plan;
use bevy_grimoire_core::references::References;
use bevy_grimoire_core::relations::Relations;
use bevy_grimoire_core::reload::{self, KINDS};
use bevy_grimoire_core::reload_check::{self, Check, Refusal};
use bevy_grimoire_core::standards::Standards;

/// One valid file of a kind, and the field to damage in it.
struct Sample {
    id: &'static str,
    /// The path the refusal must name.
    file: &'static str,
    text: String,
    /// A field of the root struct: damaged for the wrong-type case and removed for the missing one.
    field: &'static str,
    /// Whether the type requires `field`. A field with a default is not required, and a file without
    /// it is valid.
    required: bool,
}

fn fixture(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn samples() -> Vec<Sample> {
    let s = |id, file, text: String, field, required| Sample { id, file, text, field, required };
    vec![
        s("kits", "kits.yaml", fixture("kits.yaml"), "version", true),
        s("vocab", "vocab.yaml", fixture("vocab.yaml"), "kind", true),
        s("library", "site/library.yaml", fixture("site/library.yaml"), "version", true),
        s("policy", "site/project.yaml", fixture("site/project.yaml"), "version", true),
        s("rules", "rules.yaml", format!("version: {}\nblocks: []\n", bevy_grimoire_core::rules::RULES_VERSION), "version", true),
        s("compositions", "compositions.yaml", fixture("compositions.yaml"), "version", true),
        s("standards", "standards.yaml", Standards::default().to_yaml().unwrap(), "version", true),
        s("repetition", "repetition.yaml", "radius_m: 3.0\n".to_owned(), "radius_m", false),
        s("rigs", "rigs.yaml", fixture("rigs.yaml"), "version", true),
        s("map", "maps/yard.map.yaml", Map { version: MAP_VERSION, name: "yard".to_owned(), bounds: (10.0, 4.0, 10.0), ..Map::default() }.to_yaml().unwrap(), "version", true),
        s("notes", "maps/yard.notes.yaml", Notes::default().to_yaml().unwrap(), "version", true),
        s("plan", "maps/yard.plan.yaml", Plan::default().to_yaml().unwrap(), "version", true),
        s("references", "maps/yard.references.yaml", References::default().to_yaml().unwrap(), "version", true),
        s("relations", "maps/yard.relations.yaml", Relations::default().to_yaml().unwrap(), "version", true),
        s("terrain", "site/terrain/yard.yaml", "origin: [0.0, 0.0]\ncell_m: 1.0\nwidth: 2\ndepth: 2\nheights: [0.0, 0.0, 0.0, 0.0]\n".to_owned(), "cell_m", true),
        s("family_list", "family.yaml", format!("version: {}\ncrates: []\n", bevy_grimoire_core::family::FAMILY_VERSION), "version", true),
        s("agent", "agent.yaml", format!("version: {}\n", bevy_grimoire_core::note_agent::AGENT_VERSION), "version", true),
        s("play", "play.yaml", format!("version: {}\ncommand: [\"game\"]\n", bevy_grimoire_core::play::PLAY_VERSION), "version", true),
        s("water_plan", "site/water/yard.yaml", "springs: []\nsinks: []\nlakes: []\n".to_owned(), "springs", false),
        s("water_field", "site/water/yard.field.yaml", "origin: [0.0, 0.0]\ncell_m: 1.0\nwidth: 2\ndepth: 2\ndepth_m: [0.0, 0.0, 0.0, 0.0]\nsettled: true\nsteps: 3\n".to_owned(), "steps", true),
        s("volume", "site/volume/yard.yaml", "edits: []\n".to_owned(), "edits", false),
        s("data_registry", "site/data.yaml", format!("version: {}\nfiles: []\n", bevy_grimoire_core::data_files::REGISTRY_VERSION), "version", true),
        s("destructible_marks", "destructible.yaml", bevy_grimoire_core::yaml::to_string(&Marks { version: DESTRUCTIBLE_VERSION, pieces: vec!["site/cart".to_owned()] }).unwrap_or_else(|p| panic!("{}", p.line())), "version", true),
        s("destructible_bake", "destructible/site__cart.bake.yaml", bevy_grimoire_core::yaml::to_string(&Bake { version: DESTRUCTIBLE_VERSION, key: 7, chunks: vec![Chunk { points: vec![[0.0, 0.0, 0.0]], faces: vec![vec![0, 0, 0]] }] }).unwrap_or_else(|p| panic!("{}", p.line())), "key", true),
    ]
}

/// The byte range of `field: value` (and its trailing comma) in the root value of `text`. Skips
/// comments and strings, and only matches at the root's own depth, so a nested `version` is not it.
///
/// **Two spellings**, because a project file is YAML now and its sample may be either: a RON struct
/// member (`(version: 1, …`) at the struct's own depth, or a YAML key at the document's left margin
/// (`version: 1`), whose value runs to the end of its line. A YAML value is never followed by a comma, so
/// the third element is 0 there and the callers' trimming of one is a no-op.
fn field_span(text: &str, field: &str) -> (usize, usize, usize) {
    // **A YAML key at the document's left margin**, whose value runs to the end of its line. A nested `version`
    // is indented and so is never the root's, which is what this must not mistake.
    let mut at = 0usize;
    for line in text.split_inclusive('\n') {
        let opening = format!("{field}:");
        if let Some(rest) = line.strip_prefix(&opening) {
            let value = at + opening.len() + (rest.len() - rest.trim_start().len());
            let end = at + line.trim_end_matches('\n').len();
            return (at, value, end);
        }
        at += line.len();
    }
    panic!("no root field `{field}` in the sample");
}


/// **A damaged file**, which is what a half-finished write leaves: its root `version:` key is **unterminated**,
/// `version` with no `:`. That is a syntax fault in every spelling of the file — RON refuses it and so does
/// YAML — where cutting a document at a line boundary is no fault at all in YAML, and cutting inside a quoted
/// string leaves a string that reads.
///
/// A file whose first root key cannot be read is refused before anything else about it is decided, which is
/// exactly what this test is for: the verdict says `refused` and names the file.
fn damaged(text: &str) -> String {
    let mut at = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("version:") {
            let key_at = at + (line.len() - trimmed.len());
            // The colon goes, leaving `version 2` — a key with no separator.
            return format!("{}{}", &text[..key_at], &text[key_at + "version:".len()..]);
        }
        if !trimmed.is_empty() && !trimmed.starts_with('#') && at == 0 {
            // The first field is not `version`: break its colon anyway, so every sample is damaged the same way.
            if let Some(colon) = trimmed.find(':') {
                let at_key = at + (line.len() - trimmed.len()) + colon;
                return format!("{}{}", &text[..at_key], &text[at_key + 1..]);
            }
        }
        at += line.len();
    }
    text.to_owned()
}

fn wrong_type(text: &str, field: &str) -> String {
    let (_, mut value, mut end) = field_span(text, field);
    // **A key whose value starts on the next line** (`kind:` then `  tokens: []`) has nothing on its own line,
    // so the value's range is empty and lands one byte before itself. The replacement then fills the key's line,
    // which is the edit that makes the value the wrong type.
    if end < value {
        end = value;
    }
    let was = text[value..end].trim_end_matches(',').trim();
    // A number becomes a string; anything else becomes a number, which no struct, list or string takes.
    let wrong = if was.starts_with(|c: char| c.is_ascii_digit() || c == '-') { "\"wrong\"" } else { "7" };
    // **A YAML value's range ends where it starts** when the line carries nothing after the key, so the
    // replacement is an insert at `value` and the end is clamped to it: a slice that ends before it starts is
    // not a slice at all.
    let end = end.max(value);
    format!("{}{wrong}{}", &text[..value], &text[end - usize::from(text[..end].ends_with(','))..])
}

fn missing(text: &str, field: &str) -> String {
    let (start, _, _) = field_span(text, field);
    // **A field's removal takes its whole block with it.** The value may be on the key's line (`version: 2`) or
    // a block under it (`kind:` then `  tokens: []`), and deleting only the key's own line would leave the
    // block's lines behind — an indented run with no key, which is a *syntax* fault rather than a missing field,
    // so the test would measure the wrong thing. The block runs to the first line that starts at column 0 and
    // is neither blank nor a comment, which is the next root key.
    let mut end = start;
    let mut first = true;
    for line in text[start..].split_inclusive('\n') {
        if !first {
            let trimmed = line.trim();
            let at_column_0 = !line.starts_with(' ') && !line.starts_with('\t');
            if !trimmed.is_empty() && !trimmed.starts_with('#') && at_column_0 {
                break;
            }
        }
        first = false;
        end += line.len();
    }
    format!("{}{}", &text[..start], &text[end..])
}

fn refused(sample: &Sample, text: &str) -> Refusal {
    let kind = reload::kind_by_id(sample.id).unwrap();
    match reload_check::check(kind, sample.file, text) {
        Check::Refused(r) => r,
        other => panic!("`{}`: expected a refusal of\n{text}\nbut got {other:?}", sample.id),
    }
}

#[test]
fn a_valid_file_of_every_checked_kind_is_valid() {
    for sample in samples() {
        let kind = reload::kind_by_id(sample.id).unwrap();
        assert_eq!(reload_check::check(kind, sample.file, &sample.text), Check::Valid, "`{}`: {}", sample.id, sample.text);
        assert!(kind.matches(sample.file), "`{}` is not what `{}` names", sample.file, sample.id);
    }
}

#[test]
fn a_corrupt_file_is_refused_and_the_file_is_named() {
    for sample in samples() {
        let r = refused(&sample, &damaged(&sample.text));
        assert_eq!((r.kind, r.file.as_str()), (sample.id, sample.file));
        assert!(!r.reason.is_empty(), "`{}` refused with no reason", sample.id);
    }
}

#[test]
fn a_field_of_the_wrong_type_is_refused_and_named() {
    for sample in samples() {
        let r = refused(&sample, &wrong_type(&sample.text, sample.field));
        assert_eq!((r.kind, r.file.as_str()), (sample.id, sample.file));
        assert_eq!(r.field.as_deref(), Some(sample.field), "`{}`: {}", sample.id, r.reason);
        assert!(!r.reason.is_empty());
    }
}

#[test]
fn a_missing_required_field_is_refused_and_named_and_an_optional_one_is_not() {
    for sample in samples() {
        let text = missing(&sample.text, sample.field);
        let kind = reload::kind_by_id(sample.id).unwrap();
        if sample.required {
            let r = refused(&sample, &text);
            assert_eq!((r.kind, r.file.as_str()), (sample.id, sample.file));
            assert_eq!(r.field.as_deref(), Some(sample.field), "`{}`: {}", sample.id, r.reason);
            assert!(r.reason.contains(sample.field), "the reason should name the field: {}", r.reason);
        } else {
            assert_eq!(reload_check::check(kind, sample.file, &text), Check::Valid, "`{}` has no required field: {text}", sample.id);
        }
    }
}

#[test]
fn a_nested_field_is_named_by_its_path() {
    let sample = Sample { id: "terrain", file: "site/terrain/yard.yaml", text: String::new(), field: "heights", required: true };
    let text = "origin: [0.0, \"x\"]\ncell_m: 1.0\nwidth: 2\ndepth: 2\nheights: [0.0, 0.0, 0.0, 0.0]\n";
    assert_eq!(refused(&sample, text).field.as_deref(), Some("origin[1]"));
}

/// A refusal the type's own validation raises after the file parsed has no field to name and says why:
/// water that does not fill its shape, an edit with no size, a registry from another version or one
/// that names an id twice. The running state is as safe against these as against a typo.
#[test]
fn a_file_that_parses_but_is_not_valid_is_refused_and_says_why() {
    use bevy_grimoire_core::volume::{Edit, Mode, Shape, Volume};
    let carve = Edit { shape: Shape::Sphere { center: [0.0; 3], radius_m: 0.0 }, mode: Mode::Carve, smooth_m: 0.0 };
    let cases = [
        ("water_field", "site/water/yard.field.yaml", "origin: [0.0, 0.0]\ncell_m: 1.0\nwidth: 2\ndepth: 2\ndepth_m: [0.0, 0.0, 0.0]\nsettled: true\nsteps: 3\n".to_owned(), "3 depths for 2x2"),
        ("water_field", "site/water/yard.field.yaml", "origin: [0.0, 0.0]\ncell_m: 0.0\nwidth: 2\ndepth: 2\ndepth_m: [0.0, 0.0, 0.0, 0.0]\nsettled: true\nsteps: 3\n".to_owned(), "cell_m"),
        ("volume", "site/volume/yard.yaml", bevy_grimoire_core::yaml::to_string(&Volume { edits: vec![carve], ..Volume::default() }).unwrap_or_else(|p| panic!("{}", p.line())), "above zero"),
        ("data_registry", "site/data.yaml", "version: 99\nfiles: []\n".to_owned(), "version 99"),
        (
            "data_registry",
            "site/data.yaml",
            format!("version: {}\nfiles:\n- id: a\n  path: a.yaml\n- id: a\n  path: b.yaml\n", bevy_grimoire_core::data_files::REGISTRY_VERSION),
            "`a` is registered twice",
        ),
    ];
    for (id, file, text, why) in cases {
        let kind = reload::kind_by_id(id).unwrap();
        match reload_check::check(kind, file, &text) {
            Check::Refused(r) => {
                assert_eq!((r.kind, r.file.as_str()), (id, file));
                assert!(r.reason.contains(why), "`{id}` should say `{why}`: {}", r.reason);
            }
            other => panic!("`{id}`: expected a refusal of {text} but got {other:?}"),
        }
    }
}

/// Every kind with a parser here has its three cases above; this keeps the table and `check` in step.
#[test]
fn the_samples_are_exactly_the_checked_kinds() {
    let mut sampled: Vec<&str> = samples().iter().map(|s| s.id).collect();
    sampled.sort_unstable();
    let mut checked: Vec<&str> =
        KINDS.iter().filter(|k| !reload_check::unchecked().any(|u| u.id == k.id)).map(|k| k.id).collect();
    checked.sort_unstable();
    assert_eq!(sampled, checked, "a kind `check` parses needs a sample here, and a sample needs a parser");
}

/// **The kinds this crate cannot check**, checked in. A kind comes off when its type gains a check
/// here or in the layer that owns it; none joins without a commit that says why it cannot be read.
const UNCHECKED: &[&str] = &[
    "checks",
    "oplog",
    "reference_images",
    "forest",
    "people",
    "scripts",
    "trees",
    "family",
    "game_data",
    "editor_settings",
    // A JSON Schema is generated on write and read by an outside editor's extension; this crate's own
    // reader never opens one, so there is nothing here to check it against.
    "schema",
];

/// **A literal, not `UNCHECKED.len()`**: a number that is its own measure can never fail.
const UNCHECKED_MAX: usize = 11;

#[test]
fn the_unchecked_list_only_shrinks() {
    assert!(UNCHECKED.len() == UNCHECKED_MAX, "the list has {} kinds and the ratchet says {UNCHECKED_MAX}: lower both together", UNCHECKED.len());
    for k in reload_check::unchecked() {
        assert!(UNCHECKED.contains(&k.id), "`{}` has no check here and is not on the checked-in list in tests/reload_verdicts.rs", k.id);
    }
    for id in UNCHECKED {
        assert!(reload_check::unchecked().any(|k| k.id == *id), "`{id}` is on the list and is now checked: take it off and lower UNCHECKED_MAX");
    }
}
