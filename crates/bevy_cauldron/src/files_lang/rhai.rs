//! **Rhai**: a game's scripts. Coloured by the shared tokenizer, and refused only for what a bracket scan can be
//! sure of: the engine's own compile is a crate the editor does not link.

use std::ops::Range;

use bevy_gooey::SyntaxRole;
use serde_json::Value;

use super::code::{self, Code};
use super::{Env, Finding, Lang, Language};

/// The language of `.rhai` files.
pub struct Rhai;

impl Language for Rhai {
    fn lang(&self) -> Lang {
        Lang::Rhai
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rhai"]
    }

    fn highlights(&self, text: &str, _schema: Option<&Value>) -> Vec<(Range<usize>, SyntaxRole)> {
        code::highlights(text, Code::Rhai).0
    }

    fn folds(&self, text: &str) -> Vec<Range<usize>> {
        code::bracket_folds(&code::highlights(text, Code::Rhai).1)
    }

    fn diagnose(&self, _path: &str, text: &str, _env: &Env<'_>) -> Vec<Finding> {
        code::bracket_findings(text, Code::Rhai)
    }
}
