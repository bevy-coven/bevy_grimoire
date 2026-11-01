//! **One shape for a message that says something went wrong** (UI-05, `docs/ui/vision.md` §2 and §7).
//!
//! A refused operation reached a person as a toast, a journal row and a status line, and reached an
//! agent as a string; a refused file reached the journal as another string and the reload panel as
//! a third. Each surface joined the same facts differently, so a field named in one was missing from
//! the next. A [`Problem`] is the facts, once: what happened, where, why, and what to do about it.
//! Every surface is a view of it: [`Problem::line`] is the sentence a person reads, [`Problem::to_json`]
//! is what an agent receives, and neither can drop a part the other has.
//!
//! Engine-free: nothing here knows what a toast or a tool call is.

use std::fmt;

use serde::Serialize;

/// **Where a problem is**: a file, a field in it, an entity on a map. Any may be absent; one with
/// none of them is not a location, and a [`Problem`] holds `None` instead.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Location {
    /// Relative to the project directory, `/`-separated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The path to the field inside the file: `version`, `placements[3].yaw`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// A placement, stamp, piece or person id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
}

impl Location {
    pub fn file(file: impl Into<String>) -> Location {
        Location { file: Some(file.into()), ..Location::default() }
    }

    pub fn entity(entity: impl Into<String>) -> Location {
        Location { entity: Some(entity.into()), ..Location::default() }
    }

    /// The field inside the file, when the file got that far.
    pub fn field(mut self, field: Option<String>) -> Location {
        self.field = field;
        self
    }
}

impl fmt::Display for Location {
    /// `maps/yard.map.ron (placements[3].yaw)`, or `` `crate_b` `` for an entity.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        if let Some(file) = &self.file {
            f.write_str(file)?;
            first = false;
        }
        if let Some(field) = &self.field {
            if !first {
                f.write_str(" ")?;
            }
            if self.file.is_some() {
                write!(f, "({field})")?;
            } else {
                f.write_str(field)?;
            }
            first = false;
        }
        if let Some(entity) = &self.entity {
            if !first {
                f.write_str(" ")?;
            }
            write!(f, "`{entity}`")?;
        }
        Ok(())
    }
}

/// **What a problem carries besides what happened**: the parts a caller adds to a sentence it
/// already has. Every part is optional, and the default says none of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detail {
    pub where_: Option<Location>,
    pub why: Option<String>,
    pub fix: Option<String>,
    pub docs: Option<&'static str>,
}

impl Detail {
    pub fn at(mut self, location: Location) -> Detail {
        self.where_ = Some(location);
        self
    }

    pub fn because(mut self, why: impl Into<String>) -> Detail {
        self.why = Some(why.into());
        self
    }

    pub fn fix(mut self, fix: impl Into<String>) -> Detail {
        self.fix = Some(fix.into());
        self
    }

    pub fn docs(mut self, docs: &'static str) -> Detail {
        self.docs = Some(docs);
        self
    }

    /// Nothing but the sentence.
    pub fn is_empty(&self) -> bool {
        *self == Detail::default()
    }
}

/// **Something the editor could not do, or did only part of.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Problem {
    /// What happened, as a sentence a person can read alone.
    pub what: String,
    /// Where: the file, the field, the entity.
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub where_: Option<Location>,
    /// Why, when the cause is not already in `what`: a parser's own error, a rule's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// What to do about it, as an instruction. `None` when nothing can be said that is true of every
    /// case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// The page of the guide that explains it, relative to the repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<&'static str>,
}

impl Problem {
    /// A problem that is only its sentence.
    pub fn new(what: impl Into<String>) -> Problem {
        Problem { what: what.into(), where_: None, why: None, fix: None, docs: None }
    }

    /// `what` with every part `detail` has.
    pub fn with(what: impl Into<String>, detail: Detail) -> Problem {
        Problem { what: what.into(), where_: detail.where_, why: detail.why, fix: detail.fix, docs: detail.docs }
    }

    /// Everything but the sentence, as a caller built it.
    pub fn detail(&self) -> Detail {
        Detail { where_: self.where_.clone(), why: self.why.clone(), fix: self.fix.clone(), docs: self.docs }
    }

    /// **The sentence a person reads**: the toast, a journal row, the status line. A problem with
    /// nothing but `what` reads as `what`, so a message that has not gained its parts yet is
    /// unchanged; each part it has is added in a fixed order and is never left out:
    /// `rigs.ron did not read — rigs.ron (rigs[2]): 1:47: Expected float. Fix: save a rigs.ron that reads.`
    pub fn line(&self) -> String {
        let mut line = self.what.clone();
        if let Some(location) = &self.where_ {
            line.push_str(" — ");
            line.push_str(&location.to_string());
            if let Some(why) = &self.why {
                line.push_str(": ");
                line.push_str(why);
            }
        } else if let Some(why) = &self.why {
            line.push_str(" — ");
            line.push_str(why);
        }
        if let Some(fix) = &self.fix {
            if !line.ends_with(['.', '!', '?']) {
                line.push('.');
            }
            line.push_str(" Fix: ");
            line.push_str(fix);
        }
        line
    }

    /// **What an agent receives**: the same parts, by name, with the ones a problem lacks left out.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("a problem is strings and options of strings")
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.line())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_with_only_a_sentence_reads_as_that_sentence() {
        assert_eq!(Problem::new("NOT WRITTEN: read-only file system").line(), "NOT WRITTEN: read-only file system");
        assert_eq!(Problem::new("x").to_json(), serde_json::json!({"what": "x"}));
    }

    #[test]
    fn every_part_a_problem_has_is_in_its_line_and_in_its_json() {
        let p = Problem::with(
            "rigs.ron did not read",
            Detail::default()
                .at(Location::file("rigs.ron").field(Some("rigs[2]".into())))
                .because("1:47: Expected float")
                .fix("save a rigs.ron that reads")
                .docs("docs/guide/reload-contract.md"),
        );
        assert_eq!(
            p.line(),
            "rigs.ron did not read — rigs.ron (rigs[2]): 1:47: Expected float. Fix: save a rigs.ron that reads"
        );
        assert_eq!(
            p.to_json(),
            serde_json::json!({
                "what": "rigs.ron did not read",
                "where": {"file": "rigs.ron", "field": "rigs[2]"},
                "why": "1:47: Expected float",
                "fix": "save a rigs.ron that reads",
                "docs": "docs/guide/reload-contract.md",
            })
        );
    }

    #[test]
    fn a_location_names_only_what_it_has() {
        assert_eq!(Location::file("a.ron").to_string(), "a.ron");
        assert_eq!(Location::file("a.ron").field(Some("version".into())).to_string(), "a.ron (version)");
        assert_eq!(Location::entity("crate_b").to_string(), "`crate_b`");
        let both = Location { entity: Some("crate_b".into()), ..Location::file("maps/y.map.ron") };
        assert_eq!(both.to_string(), "maps/y.map.ron `crate_b`");
    }

    #[test]
    fn a_fix_after_a_sentence_without_a_stop_gets_one() {
        let p = Problem::with("`crate_b` is not on the map", Detail::default().fix("reopen the map"));
        assert_eq!(p.line(), "`crate_b` is not on the map. Fix: reopen the map");
        let q = Problem::with("Nothing to undo.", Detail::default().fix("make a change first"));
        assert_eq!(q.line(), "Nothing to undo. Fix: make a change first");
    }

    #[test]
    fn detail_round_trips_through_a_problem() {
        let d = Detail::default().at(Location::entity("a")).because("b").fix("c");
        assert_eq!(Problem::with("w", d.clone()).detail(), d);
    }
}
