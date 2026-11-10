//! **Re-pin the editor's own RON data files to YAML**, once, by reading each with its type (GD-03, #365).
//!
//! The editor ships one skin (`src/editor_skin.ron`) and two `bevy_elements` settings fixtures
//! (`tests/fixtures/bevy_elements*.ron`). None is a project-file kind, so `cauldron migrate` does not
//! convert them; this is the equivalent for them, and it is a committed program rather than a hand edit
//! for the reason the migration exists: RON and YAML spell an `Option`, a tuple and an enum differently,
//! so a text substitution produces a file that looks right and holds a different value.
//!
//!     cargo run -p bevy_cauldron --example fixtures_to_yaml
//!
//! Each `.ron` is read through `bevy_grimoire_migrate::read_ron` as its type, written beside itself as
//! `.yaml`, and the YAML is read back and compared to the RON value before anything is written. Once the
//! `.ron` is gone a second run reads the `.yaml` and says it still round trips.
//!
//! Not a product path: nothing in the editor, the game or an agent calls it.

use std::path::{Path, PathBuf};

use bevy_cauldron::environment::ElementsSettings;
use bevy_gooey::GooeySkin;

fn convert<T>(ron_path: &Path)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let yaml_path = ron_path.with_extension("yaml");
    let Ok(ron_text) = std::fs::read_to_string(ron_path) else {
        let text = std::fs::read_to_string(&yaml_path).unwrap_or_else(|e| panic!("{}: {e}", yaml_path.display()));
        let now: T = bevy_grimoire_core::yaml::from_str(&text, &yaml_path.display().to_string()).unwrap_or_else(|p| panic!("{}", p.line()));
        let back = bevy_grimoire_core::yaml::to_string(&now).unwrap_or_else(|p| panic!("{}", p.line()));
        assert_eq!(back, text, "{} does not round trip", yaml_path.display());
        println!("{} reads and round trips ({} bytes)", yaml_path.display(), text.len());
        return;
    };
    let was: T = bevy_grimoire_migrate::read_ron(&ron_text).unwrap_or_else(|e| panic!("{}: {e}", ron_path.display()));
    let yaml = bevy_grimoire_core::yaml::to_string(&was).unwrap_or_else(|p| panic!("{}", p.line()));
    let now: T = bevy_grimoire_core::yaml::from_str(&yaml, &yaml_path.display().to_string()).unwrap_or_else(|p| panic!("{}", p.line()));
    let was_again = bevy_grimoire_core::yaml::to_string(&was).unwrap_or_else(|p| panic!("{}", p.line()));
    let now_again = bevy_grimoire_core::yaml::to_string(&now).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(now_again, was_again, "{}: the YAML holds a different value than the RON did", ron_path.display());
    std::fs::write(&yaml_path, &yaml).unwrap_or_else(|e| panic!("{}: {e}", yaml_path.display()));
    println!("{} -> {} ({} bytes)", ron_path.display(), yaml_path.display(), yaml.len());
}

fn main() {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    convert::<GooeySkin>(&here.join("src/editor_skin.ron"));
    convert::<ElementsSettings>(&here.join("tests/fixtures/bevy_elements.ron"));
    convert::<ElementsSettings>(&here.join("tests/fixtures/bevy_elements_18.5.ron"));
}
