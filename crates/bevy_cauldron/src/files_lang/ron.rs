//! **RON**: what a game keeps of its own (no project file of this repository is RON any more). Coloured by the
//! shared tokenizer, and refused only for what a bracket scan can be sure of.

use std::ops::Range;

use bevy_gooey::SyntaxRole;
use serde_json::Value;

use super::code::{self, Code};
use super::{Env, Finding, Lang, Language};

/// The language of `.ron` files.
pub struct Ron;

impl Language for Ron {
    fn lang(&self) -> Lang {
        Lang::Ron
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ron"]
    }

    fn highlights(&self, text: &str, _schema: Option<&Value>) -> Vec<(Range<usize>, SyntaxRole)> {
        code::highlights(text, Code::Ron).0
    }

    fn folds(&self, text: &str) -> Vec<Range<usize>> {
        code::bracket_folds(&code::highlights(text, Code::Ron).1)
    }

    fn diagnose(&self, _path: &str, text: &str, _env: &Env<'_>) -> Vec<Finding> {
        code::bracket_findings(text, Code::Ron)
    }
}
