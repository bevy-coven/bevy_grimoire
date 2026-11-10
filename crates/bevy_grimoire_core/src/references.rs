//! **Reference images as project data** (E-075): the pictures a person points at with "like this".
//!
//! A reference is a project-relative image path, a caption and an id (`r1`, `r2`, … never reused).
//! The file sits **beside the map** (`maps/town.map.yaml` has `maps/town.references.yaml`), like
//! [`crate::notes`], so a [`crate::live::FileWatch`] sees it change. An agent reads the list and names
//! a reference back by id with [`References::cite`], so the picture it worked from is on record.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

pub const REFERENCES_VERSION: u32 = 1;

const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub id: String,
    /// Relative to the project root, `/`-separated, normalised.
    pub path: String,
    pub caption: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct References {
    pub version: u32,
    pub refs: Vec<Reference>,
    /// The highest number ever issued, so a removed id is not given out again.
    #[serde(default)]
    pub issued: u32,
}

impl Default for References {
    fn default() -> Self {
        References { version: REFERENCES_VERSION, refs: Vec::new(), issued: 0 }
    }
}

/// `town.map.yaml` → `town.references.yaml`, in the same directory.
pub fn path_for_map(map_path: &Path) -> PathBuf {
    let name = map_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = name
        .strip_suffix(crate::map::MAP_SUFFIX)
        .or_else(|| name.strip_suffix(".yaml"))
        .unwrap_or(&name);
    map_path.with_file_name(format!("{stem}.references.yaml"))
}

/// A project-relative image path with `.` removed; refuses absolute paths, `..` and non-images.
fn normalise(path: &str) -> Result<String, String> {
    let mut parts = Vec::new();
    for c in Path::new(path).components() {
        match c {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(format!("references: {path:?} must be a path inside the project")),
        }
    }
    let Some(last) = parts.last() else {
        return Err("references: a reference needs a path".to_owned());
    };
    let ext = last.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(format!("references: {path:?} is not an image ({})", IMAGE_EXTS.join(", ")));
    }
    Ok(parts.join("/"))
}

impl References {
    pub fn parse(text: &str) -> Result<References, String> {
        let r: References =
            crate::yaml::from_str(text, "references").map_err(|p| format!("references: parse: {}", p.line()))?;
        if r.version != REFERENCES_VERSION {
            return Err(format!("references: version {} is not the {REFERENCES_VERSION} this build reads", r.version));
        }
        Ok(r)
    }

    /// **The file as the one YAML writer writes it.**
    pub fn to_yaml(&self) -> Result<String, String> {
        crate::yaml::to_string(self).map_err(|p| format!("references: {}", p.line()))
    }

    /// A file that is not there is no references, not an error.
    pub fn load(path: &Path) -> Result<References, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => References::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(References::default()),
            Err(e) => Err(format!("references: {}: {e}", path.display())),
        }
    }

    /// Atomic; no references and no file writes nothing.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.refs.is_empty() && !path.exists() {
            return Ok(());
        }
        crate::yaml::save(path, &self.to_yaml()?)
    }

    pub fn get(&self, id: &str) -> Option<&Reference> {
        self.refs.iter().find(|r| r.id == id)
    }

    /// Adds a picture; the same path twice is one reference, and the error names it.
    pub fn add(&mut self, path: &str, caption: &str) -> Result<String, String> {
        let path = normalise(path)?;
        if let Some(r) = self.refs.iter().find(|r| r.path == path) {
            return Err(format!("references: {path} is already {}", r.id));
        }
        let issued = self.issued.max(self.refs.iter().filter_map(|r| r.id[1..].parse().ok()).max().unwrap_or(0)) + 1;
        self.issued = issued;
        let id = format!("r{issued}");
        self.refs.push(Reference { id: id.clone(), path, caption: caption.trim().to_owned() });
        Ok(id)
    }

    pub fn remove(&mut self, id: &str) -> Result<Reference, String> {
        let i = self.refs.iter().position(|r| r.id == id).ok_or_else(|| format!("references: no reference {id}"))?;
        Ok(self.refs.remove(i))
    }

    /// The line an agent writes into a note's summary or an assumption to say which picture it used.
    pub fn cite(&self, id: &str) -> Result<String, String> {
        let r = self.get(id).ok_or_else(|| format!("references: no reference {id}"))?;
        Ok(format!("reference {} ({}): {}", r.id, r.path, r.caption))
    }

    /// The ids that `texts` cite as `reference rN` and that are not in this set, in first-cited order. A
    /// note's resolution cites the picture it used; an id that names nothing is a citation nobody can follow.
    pub fn unknown_citations<'a>(&self, texts: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for text in texts {
            let mut rest = text;
            while let Some(at) = rest.find("reference r") {
                rest = &rest[at + "reference ".len()..];
                let digits: String = rest[1..].chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    continue;
                }
                let id = format!("r{digits}");
                if self.get(&id).is_none() && !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    /// `(id, path)` of every reference whose file is not under `root`.
    pub fn missing(&self, root: &Path) -> Vec<(String, String)> {
        self.refs.iter().filter(|r| !root.join(&r.path).is_file()).map(|r| (r.id.clone(), r.path.clone())).collect()
    }
}
