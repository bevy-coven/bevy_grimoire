//! **Re-pin `bevy_oracle`'s three fixtures from RON to YAML**, once, by reading each with its type.
//!
//! The oracle's files are not project-file kinds — `checks.yaml` is the editor's own set of checks,
//! `clip_samples.yaml` a labelled capture set, `clip_calibration.yaml` the record a calibration earned —
//! so they are not on `migrate::KINDS` and the migration cannot convert them. This is the equivalent
//! conversion for them, and it is a committed program rather than a hand edit for the reason the
//! migration exists: RON and YAML spell an `Option` and an enum differently, so a text substitution
//! produces a file that looks right and holds a different value.
//!
//!     cargo run -p bevy_grimoire_migrate --example oracle_fixtures_to_yaml
//!
//! It reads each `.ron` beside its `.yaml` target's name, prints the YAML, and writes nothing: the files
//! in `crates/bevy_oracle/fixtures/` were written from its output, and re-running it is how a person
//! checks they still say the same thing.
//!
//! Not a product path: nothing in the editor, the game or an agent calls it.

use std::path::PathBuf;

use bevy_oracle::{Calibration, Checks, Labelled};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bevy_oracle/fixtures")
}

/// Read `name.ron`, write `name.yaml`, and compare what the YAML reads back as against the RON value.
fn convert<T>(name: &str)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq,
{
    let dir = fixtures();
    let ron_path = dir.join(format!("{name}.ron"));
    let yaml_path = dir.join(format!("{name}.yaml"));
    let ron_text = match std::fs::read_to_string(&ron_path) {
        Ok(t) => t,
        // **The `.ron` original is gone once it has been converted**, so a second run reads the YAML and
        // says what it holds: the same value, or not.
        Err(_) => {
            let yaml_text = std::fs::read_to_string(&yaml_path).unwrap_or_else(|e| panic!("{}: {e}", yaml_path.display()));
            let now: T = serde_yaml_ng::from_str(&yaml_text).unwrap_or_else(|e| panic!("{}: {e}", yaml_path.display()));
            let back = serde_yaml_ng::to_string(&now).expect("a value writes");
            assert_eq!(back, yaml_text, "{} does not round trip", yaml_path.display());
            println!("{} reads and round trips ({} bytes)", yaml_path.display(), yaml_text.len());
            return;
        }
    };
    let was: T = bevy_grimoire_migrate::read_ron(&ron_text).unwrap_or_else(|e| panic!("{}: {e}", ron_path.display()));
    let yaml = bevy_grimoire_core::yaml::to_string(&was).unwrap_or_else(|p| panic!("{}", p.line()));
    let now: T = serde_yaml_ng::from_str(&yaml).unwrap_or_else(|e| panic!("{name}.yaml: {e}"));
    assert!(now == was, "{name}: the YAML holds a different value than the RON did");
    std::fs::write(&yaml_path, &yaml).unwrap_or_else(|e| panic!("{}: {e}", yaml_path.display()));
    println!("{} -> {} ({} bytes)", ron_path.display(), yaml_path.display(), yaml.len());
}

fn main() {
    convert::<Checks>("checks");
    convert::<Vec<Labelled>>("clip_samples");
    convert::<Calibration>("clip_calibration");
}
