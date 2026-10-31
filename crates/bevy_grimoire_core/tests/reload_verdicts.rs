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
        s("kits", "kits.ron", fixture("kits.ron"), "version", true),
        s("vocab", "vocab.ron", fixture("vocab.ron"), "kind", true),
        s("library", "site/library.ron", fixture("site/library.ron"), "version", true),
        s("policy", "site/project.ron", fixture("site/project.ron"), "version", true),
        s("rules", "rules.ron", format!("(version: {}, blocks: [])", bevy_grimoire_core::rules::RULES_VERSION), "version", true),
        s("compositions", "compositions.ron", fixture("compositions.ron"), "version", true),
        s("standards", "standards.ron", Standards::default().to_ron().unwrap(), "version", true),
        s("repetition", "repetition.ron", "(radius_m: 3.0)".to_owned(), "radius_m", false),
        s("rigs", "rigs.ron", fixture("rigs.ron"), "version", true),
        s("map", "maps/yard.map.ron", Map { version: MAP_VERSION, name: "yard".to_owned(), bounds: (10.0, 4.0, 10.0), ..Map::default() }.to_ron().unwrap(), "version", true),
        s("notes", "maps/yard.notes.ron", Notes::default().to_ron().unwrap(), "version", true),
        s("plan", "maps/yard.plan.ron", Plan::default().to_ron().unwrap(), "version", true),
        s("references", "maps/yard.references.ron", References::default().to_ron().unwrap(), "version", true),
        s("relations", "maps/yard.relations.ron", Relations::default().to_ron().unwrap(), "version", true),
        s("terrain", "site/terrain/yard.ron", "(origin: (0.0, 0.0), cell_m: 1.0, width: 2, depth: 2, heights: [0.0, 0.0, 0.0, 0.0])".to_owned(), "cell_m", true),
        s("family_list", "family.ron", format!("(version: {}, crates: [])", bevy_grimoire_core::family::FAMILY_VERSION), "version", true),
        s("agent", "agent.ron", format!("(version: {})", bevy_grimoire_core::note_agent::AGENT_VERSION), "version", true),
        s("play", "play.ron", format!("(version: {}, command: [\"game\"])", bevy_grimoire_core::play::PLAY_VERSION), "version", true),
    ]
}

/// The byte range of `field: value` (and its trailing comma) in the root struct of `text`. Skips
/// comments and strings, and only matches at the root's own depth, so a nested `version` is not it.
fn field_span(text: &str, field: &str) -> (usize, usize, usize) {
    let b = text.as_bytes();
    let (mut depth, mut i) = (0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ if depth == 1
                && text[i..].starts_with(field)
                && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'))
                && text[i + field.len()..].trim_start().starts_with(':') =>
            {
                let colon = i + field.len() + text[i + field.len()..].find(':').unwrap();
                let value = colon + 1 + text[colon + 1..].len() - text[colon + 1..].trim_start().len();
                let (mut j, mut d) = (value, 0i32);
                while j < b.len() {
                    match b[j] {
                        b'"' => {
                            j += 1;
                            while j < b.len() && b[j] != b'"' {
                                j += if b[j] == b'\\' { 2 } else { 1 };
                            }
                        }
                        b'(' | b'[' | b'{' => d += 1,
                        b')' | b']' | b'}' if d == 0 => break,
                        b')' | b']' | b'}' => d -= 1,
                        b',' if d == 0 => break,
                        _ => {}
                    }
                    j += 1;
                }
                let end = if b.get(j) == Some(&b',') { j + 1 } else { j };
                return (i, value, end.min(b.len()));
            }
            _ => {}
        }
        i += 1;
    }
    panic!("no root field `{field}` in the sample");
}

fn corrupt(text: &str) -> String {
    let half = (0..=text.len() / 2).rev().find(|&i| text.is_char_boundary(i)).unwrap();
    text[..half].to_owned()
}

fn wrong_type(text: &str, field: &str) -> String {
    let (_, value, end) = field_span(text, field);
    let was = text[value..end].trim_end_matches(',').trim();
    // A number becomes a string; anything else becomes a number, which no struct, list or string takes.
    let wrong = if was.starts_with(|c: char| c.is_ascii_digit() || c == '-') { "\"wrong\"" } else { "7" };
    format!("{}{wrong}{}", &text[..value], &text[end - usize::from(text[..end].ends_with(',')) ..])
}

fn missing(text: &str, field: &str) -> String {
    let (start, _, end) = field_span(text, field);
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
        let r = refused(&sample, &corrupt(&sample.text));
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
    let sample = Sample { id: "terrain", file: "site/terrain/yard.ron", text: String::new(), field: "heights", required: true };
    let text = "(origin: (0.0, \"x\"), cell_m: 1.0, width: 2, depth: 2, heights: [0.0, 0.0, 0.0, 0.0])";
    assert_eq!(refused(&sample, text).field.as_deref(), Some("origin[1]"));
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
    "data_registry",
    "oplog",
    "reference_images",
    "water_plan",
    "water_field",
    "volume",
    "forest",
    "people",
    "scripts",
    "trees",
    "family",
    "destructible_marks",
    "destructible_bake",
    "game_data",
];

/// **A literal, not `UNCHECKED.len()`**: a number that is its own measure can never fail.
const UNCHECKED_MAX: usize = 15;

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
