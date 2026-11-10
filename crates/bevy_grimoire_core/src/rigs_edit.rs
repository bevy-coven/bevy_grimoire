//! **Editing one rig inside `rigs.yaml` without destroying the rest of it.**
//!
//! `Rigs::to_yaml` would round-trip the *values* and delete every comment — and this manifest's
//! comments are load-bearing prose (the Valkyrie's LEFTWARD note among them), which is why the bench's
//! write-back goes through the surgery instead: [`crate::yaml_surgery`] edits one leaf's bytes, so
//! everything outside it is byte-identical by construction.
//!
//! **This is a thin policy layer, and it is smaller than it was.** It used to carry its own RON line
//! scanner — a byte span per rig, a line index per slot record, an insertion point for a new field — because
//! the RON writer found only the *first* `slots: [` in a document and this file has sixteen. YAML has paths:
//! a slot's duration is `rigs.alpha.slots[1].playback.duration`, so the surgery's own `set` and `add_entry`
//! address the one leaf the caller meant, with no bookkeeping to keep in step with an edit.

use crate::rigs::{Provenance, Rigs};
use crate::yaml_surgery::{self, Value};

/// One rig's block, as the whole file's text and the two paths an edit addresses.
///
/// A field a caller sets is `rigs.<name>.<field>`; a slot's is `rigs.<name>.slots[<i>].<field>`.
pub struct RigDoc {
    /// The complete document, as last edited.
    full: String,
    /// The rig's name, which is the key it stands under.
    rig: String,
    /// How many slots the rig has, so a slot number past its end is refused here rather than written
    /// somewhere else.
    slots: usize,
}

impl RigDoc {
    /// Open `rig_name`'s block inside the manifest text.
    ///
    /// Refuses a document that does not parse and validate — editing a broken file surgically produces a
    /// precisely edited broken file — and refuses a rig the manifest does not hold.
    pub fn open(text: &str, rig_name: &str) -> Result<RigDoc, String> {
        let parsed = Rigs::parse(text)?;
        let rig = parsed.get(rig_name).ok_or_else(|| format!("rigs.yaml has no rig named `{rig_name}`"))?;
        Ok(RigDoc { full: text.to_owned(), rig: rig_name.to_owned(), slots: rig.slots.len() })
    }

    /// Rewrite one field of one slot record — `duration`, `phase_offset`, `cycle_distance` — through the
    /// one surgery, which keeps the indentation, the sibling fields and every comment where they were.
    pub fn edit_slot_field(&mut self, slot: usize, field: &str, value: &str) -> Result<(), String> {
        if slot >= self.slots {
            return Err(format!("no slot {slot} in this rig"));
        }
        // **`playback` is a tagged enum**, so its fields sit one level deeper: `!Gait` then the numbers.
        // The path the surgery edits is the leaf's own, which is what `yaml::leaves` answers.
        let path = format!("rigs.{}.slots[{slot}].playback.{field}", self.rig);
        let number: Value = match field {
            "duration" | "phase_offset" | "cycle_distance" => Value::Float(
                value.parse::<f64>().map_err(|e| format!("`{value}` is not a number: {e}"))?,
            ),
            _ => Value::Str(value.to_owned()),
        };
        self.full = yaml_surgery::set(&self.full, &path, number, None).map_err(|p| p.line())?;
        Ok(())
    }

    /// Replace-or-insert a one-line rig-level field — the provenance stamp's door.
    ///
    /// **The value is the file's YAML, not a hand-built string**: [`provenance_yaml`] is
    /// `serde_yaml_ng`'s own output for the value, so the spelling that lands on disk is the spelling the
    /// reader takes back — which is what the stamp's round-trip test pins.
    pub fn set_rig_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        let parent = format!("rigs.{}", self.rig);
        // **A value may be a block** (the provenance stamp is a mapping of five fields), so the entry's first
        // line carries the key and every later line is the value's own, which `set_entry` writes under it.
        // **A block value starts on the next line**, which is what YAML requires of a mapping under a key: the
        // entry's first line is `field:` and the value's own lines follow, indented by `set_entry`.
        let entry = if value.contains('\n') {
            format!("{field}:\n{value}")
        } else {
            format!("{field}: {value}")
        };
        self.full = yaml_surgery::set_entry(&self.full, &parent, &entry).map_err(|p| p.line())?;
        Ok(())
    }

    /// The whole document as it now stands. Every byte the edits did not address is the byte it was.
    pub fn render(&self) -> String {
        self.full.clone()
    }
}

/// **A [`Provenance`] as the YAML `set_rig_field` writes**, indented to sit under its key.
///
/// `serde_yaml_ng`'s own output for the value, so the spelling that lands on disk is the spelling the reader
/// takes back — the stamp's round-trip test is what pins that, and a hand-built string would be a second
/// spelling to keep in step with the type.
pub fn provenance_yaml(p: &Provenance) -> String {
    // **Unindented on purpose**: [`crate::yaml_surgery::set_entry`] indents what it inserts to the mapping's
    // own column, so a caller that padded here would have its padding added to.
    crate::yaml::to_string(p).unwrap_or_else(|e| e.line()).trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rigs::{BENCH_TOOL_VERSION, fingerprint_string};

    /// A two-rig miniature in the real manifest's shape: comments above and inside, a note that
    /// contains a field-name-looking string, and one-line slot records.
    const DOC: &str = r#"# Header prose that must survive.
version: 3
rigs:
  # The first rig.
  alpha:
    mesh: "a/a.glb"
    scale: 1.0
    slots:
      - clip: 0
        playback: !Free
          speed: 1.0
        note: "idle"
  beta:
    mesh: "b/b.glb"
    scale: 2.0
    slots:
      - clip: 0
        playback: !Gait
          duration: 1.417
          phase_offset: 0.0
          cycle_distance: 1.388
        note: "walk — the reference"  # trailing
      - clip: 1
        playback: !Gait
          duration: 0.75
          phase_offset: -0.016
          cycle_distance: 2.135
        note: "run"
"#;

    #[test]
    fn a_no_op_open_and_render_is_byte_identical() {
        let doc = RigDoc::open(DOC, "beta").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(doc.render(), DOC);
    }

    #[test]
    fn the_quoted_key_finds_the_second_rig_not_the_first() {
        let mut doc = RigDoc::open(DOC, "beta").unwrap_or_else(|e| panic!("{e}"));
        doc.edit_slot_field(0, "duration", "1.402").unwrap_or_else(|e| panic!("{e}"));
        let out = doc.render();
        assert!(out.contains("duration: 1.402"), "{out}");
        // Alpha's block and every comment are untouched.
        assert!(out.contains("mesh: \"a/a.glb\"") || out.contains("mesh: a/a.glb"));
        assert!(out.contains("# The first rig."));
        assert!(out.contains("# Header prose that must survive."));
        assert!(out.contains("# trailing"), "the record's trailing comment survives");
        assert!(out.contains("walk — the reference"), "the note survives");
    }

    #[test]
    fn editing_one_field_changes_exactly_one_line() {
        let mut doc = RigDoc::open(DOC, "beta").unwrap_or_else(|e| panic!("{e}"));
        doc.edit_slot_field(1, "cycle_distance", "2.106").unwrap_or_else(|e| panic!("{e}"));
        let before: Vec<&str> = DOC.lines().collect();
        let out = doc.render();
        let after: Vec<&str> = out.lines().collect();
        assert_eq!(before.len(), after.len());
        let changed: Vec<usize> = (0..before.len()).filter(|&i| before[i] != after[i]).collect();
        assert_eq!(changed.len(), 1, "{changed:?}");
        assert!(after[changed[0]].contains("cycle_distance: 2.106"));
    }

    #[test]
    fn a_provenance_stamp_inserts_once_then_rewrites_in_place() {
        let stamp = Provenance {
            glb_fnv1a: fingerprint_string(0x1234_5678_9abc_def0),
            clips: 2,
            clip_names: vec!["walk".to_owned(), String::new()],
            tool: BENCH_TOOL_VERSION,
            date: "2026-08-06".to_owned(),
        };
        let mut doc = RigDoc::open(DOC, "beta").unwrap_or_else(|e| panic!("{e}"));
        doc.set_rig_field("provenance", &provenance_yaml(&stamp)).unwrap_or_else(|e| panic!("{e}"));
        let once = doc.render();
        assert!(
            once.lines().count() > DOC.lines().count(),
            "the first stamp adds the provenance entry"
        );
        // It parses back to the same value — the spelling pin.
        let parsed = Rigs::parse(&once).unwrap_or_else(|e| panic!("{e}"));
        let read = parsed.get("beta").and_then(|r| r.provenance.as_ref());
        assert_eq!(read, Some(&stamp));

        // A second adopt rewrites the same line rather than stacking stamps, and slot edits still
        // land after the insertion shifted the bookmarks.
        let mut doc = RigDoc::open(&once, "beta").unwrap_or_else(|e| panic!("{e}"));
        let mut newer = stamp.clone();
        newer.date = "2026-08-07".to_owned();
        doc.set_rig_field("provenance", &provenance_yaml(&newer)).unwrap_or_else(|e| panic!("{e}"));
        doc.edit_slot_field(0, "duration", "1.4").unwrap_or_else(|e| panic!("{e}"));
        let twice = doc.render();
        assert_eq!(
            twice.lines().count(),
            once.lines().count(),
            "a second adopt rewrites the entry rather than stacking another"
        );
        assert!(twice.contains("2026-08-07") && !twice.contains("2026-08-06"));
        assert!(twice.contains("duration: 1.4"), "{twice}");
    }

    #[test]
    fn the_shipped_manifest_opens_and_renders_byte_identical_for_every_rig() {
        // The strongest comment-preservation assertion available, and it is read-only: every rig's
        // block round-trips the REAL file untouched.
        let text = std::fs::read_to_string("../../assets/grimoire/rigs.yaml")
            .unwrap_or_else(|e| panic!("rigs.yaml: {e}"));
        let rigs = Rigs::parse(&text).unwrap_or_else(|e| panic!("{e}"));
        for name in rigs.rigs.keys() {
            let doc = RigDoc::open(&text, name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(doc.render(), text, "rig `{name}` did not round-trip");
        }
    }

    #[test]
    fn a_document_that_does_not_parse_is_refused_before_any_edit() {
        let e = RigDoc::open("(version: 2, rigs: {)", "alpha").err();
        assert!(e.is_some());
    }
}
