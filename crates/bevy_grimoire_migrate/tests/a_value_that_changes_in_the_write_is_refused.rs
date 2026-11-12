//! **`convert` refuses a type whose value would change in the move** (GD-03, #365; review M5 c).
//!
//! The project-level test of a value-changing migration could only trigger the refusal through a file it could not read at all.
//! This one drives `convert` itself with a type that does not round-trip: its reader adds one to `n`, so the value read back from
//! the YAML differs from the value read from the RON, and the second write differs from the first. That is the branch the
//! faithfulness check lives in.

use bevy_grimoire_migrate::{COMMAND, migrate::convert};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Lossy {
    n: u32,
}

impl<'de> Deserialize<'de> for Lossy {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            n: u32,
        }
        Raw::deserialize(d).map(|raw| Lossy { n: raw.n + 1 })
    }
}

#[derive(Serialize, Deserialize)]
struct Faithful {
    n: u32,
}

#[test]
fn a_type_that_reads_the_yaml_differently_is_refused_naming_the_file() {
    let refused = convert::<Lossy>("(n: 1)", "assets/grimoire/lossy.ron", COMMAND).err().unwrap_or_else(|| panic!("a value that changes is refused"));
    let said = refused.line();
    assert!(said.contains("assets/grimoire/lossy.ron"), "the file is named: {said}");
    assert!(said.contains("would change value"), "and what is wrong: {said}");
}

#[test]
fn a_type_that_round_trips_is_converted() {
    let done = convert::<Faithful>("(n: 1)", "faithful.ron", COMMAND).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(done.text, "n: 1\n");
}
