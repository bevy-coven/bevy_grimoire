//! **The style of one place, as an agent reads it** (SG-02, #401; the series #399).
//!
//! [`crate::design::Tree`] says what is in `design/`; this module answers the question an agent asks before it
//! shapes a place: *what should this feel like?* The answer is a **chain** of files, the game's first, then the
//! region's, then the level's, then the area's that holds a point. Each place contributes its `style.md` and, when
//! a topic is asked for and the place has it, the file for that topic. Each file arrives under a heading naming
//! its path.
//!
//! # The most specific place comes last
//!
//! A model makes best use of the start and the end of its context and loses what sits in the middle
//! (10.48550/arXiv.2307.03172), so the game's frame opens the answer and the area's rules close it.
//!
//! # A style file is prompt text, so it needs the person's trust
//!
//! A project cloned from somewhere else can carry a `design/style.md` that tells an agent with a shell to do
//! anything, and an agent follows the text it is given — the same hole the project's guide closed (AG-03). So
//! [`compose`] asks the caller, per file, whether the person has allowed **that exact content** (a
//! [`crate::trust::TextGrant`]) or the whole project. A file that has not been allowed is **withheld**: its
//! place in the answer is one line saying it is waiting for the person, its text is never in the answer, and no
//! picture it cites is attached.
//!
//! # The pictures the text cites
//!
//! The pictures a served file cites come back with their captions, at most [`STYLE_IMAGES_MAX`], each at most
//! [`STYLE_IMAGE_MAX_BYTES`], nearest place first and then in the order the text cites them. **A picture left out
//! is named with its path, its caption and the reason**, so an agent that cannot see still has the words.
//!
//! # What it refuses
//!
//! Never guessing: a map the project does not have, a point outside the map, a point with no map, and a topic
//! the game does not state are each refused with the reason. **The text itself is never cut**: a chain over
//! [`crate::design::STYLE_CHAIN_MAX_BYTES`] is refused with its files' sizes.
//!
//! Engine-free: `std`, `serde_json`, and the design tree.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::design::{self, PlaceKind, STYLE_CHAIN_MAX_BYTES, STYLE_TOPIC, Place, Topic, Tree};
use crate::trust::{self, Decision};

/// **The most pictures one answer attaches.** Eight pictures are a lot of an agent's context; a ninth is named,
/// with its path and its caption, and not attached.
pub const STYLE_IMAGES_MAX: usize = 8;

/// **The largest picture one answer attaches, in bytes** (1 MiB). A bigger one is named and not attached.
pub const STYLE_IMAGE_MAX_BYTES: u64 = 1_048_576;

/// **What an agent asks for**: a map and a point on it, and a topic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Request {
    /// A map of the project. With none, the answer is the game's files alone.
    pub map: Option<String>,
    /// A point on the map, `(x, z)` in map metres: the area that holds it is the last place of the chain.
    pub at: Option<(f32, f32)>,
    /// A topic the game states: `dialogue`, `sound`. Each place that has a file for it adds that file.
    pub topic: Option<String>,
}

/// **What became of one file of the chain.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileState {
    /// Its text is in the answer.
    Served,
    /// The person has not allowed it: one line stands in its place.
    Withheld,
    /// It could not be read whole (too large, not UTF-8): the line names why.
    Unread,
}

impl FileState {
    /// The word `grimoire/style` answers with.
    pub fn word(self) -> &'static str {
        match self {
            FileState::Served => "served",
            FileState::Withheld => "withheld",
            FileState::Unread => "unread",
        }
    }
}

/// **One file of the chain**, in the order the answer reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub place: PlaceKind,
    /// Relative to the project root, `/`-separated.
    pub path: String,
    pub bytes: usize,
    pub state: FileState,
}

/// **A picture the answer attaches.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// Relative to the project root, `/`-separated.
    pub file: String,
    /// The file on disk, resolved: inside the project, which `file` alone does not prove (a link may leave it).
    pub path: PathBuf,
    /// The text's own words for it: its alt text, or its title when it has none.
    pub caption: String,
    /// The style file that cites it.
    pub cited_by: String,
    pub bytes: u64,
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub mime: &'static str,
}

/// **A picture the text cites and the answer does not attach**, with why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    /// The path, or the destination as the Markdown wrote it when it names no file of the project.
    pub file: String,
    pub caption: String,
    pub cited_by: String,
    pub reason: String,
}

/// **A file waiting for the person**, with the text the question is about. The text is for the editor's
/// question and is never part of [`Answer::json`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Withheld {
    pub path: String,
    pub text: String,
}

/// **The answer.**
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The chain as Markdown: each file under a heading naming its path, then the pictures not attached.
    pub text: String,
    pub files: Vec<File>,
    pub pictures: Vec<Picture>,
    pub left_out: Vec<LeftOut>,
    pub withheld: Vec<Withheld>,
    /// Why no decision could be made about a file: a store that will not read, or a project root that will not
    /// resolve. Every such file is withheld.
    pub trouble: Option<String>,
}

impl Answer {
    /// **The answer as `grimoire/style` sends it.** The withheld files' text is not in it.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "text": self.text,
            "files": self.files.iter().map(|f| serde_json::json!({
                "place": f.place.word(),
                "path": f.path,
                "bytes": f.bytes,
                "state": f.state.word(),
            })).collect::<Vec<_>>(),
            "pictures": self.pictures.iter().map(|p| serde_json::json!({
                "file": p.file,
                "path": p.path.to_string_lossy(),
                "caption": p.caption,
                "cited_by": p.cited_by,
                "bytes": p.bytes,
                "mime": p.mime,
            })).collect::<Vec<_>>(),
            "left_out": self.left_out.iter().map(|l| serde_json::json!({
                "file": l.file,
                "caption": l.caption,
                "cited_by": l.cited_by,
                "reason": l.reason,
            })).collect::<Vec<_>>(),
            "withheld": self.withheld.iter().map(|w| w.path.as_str()).collect::<Vec<_>>(),
            "trouble": self.trouble,
        })
    }
}

/// **The style of the place `request` names**, with the files the person has not allowed withheld.
///
/// `decide` is asked once per file that read, with its project-relative path and its text, and answers whether the
/// person has allowed it. An `Err` from it withholds the file and is reported as [`Answer::trouble`].
///
/// An `Err` here is a refusal with its reason: a request that names nothing real, or a chain that would have to be
/// cut.
pub fn compose(
    tree: &Tree,
    request: &Request,
    decide: &mut dyn FnMut(&str, &str) -> Result<Decision, String>,
) -> Result<Answer, String> {
    check(tree, request)?;
    let chain = tree.chain_to(request.map.as_deref(), request.at);
    let topic = request.topic.as_deref().filter(|t| *t != STYLE_TOPIC);
    // **Each place's files, its overall feel first and then the topic asked for.**
    let rows_of = |place: &Place| -> Vec<Topic> {
        let mut rows: Vec<Topic> = place.topic(STYLE_TOPIC).cloned().into_iter().collect();
        rows.extend(topic.and_then(|t| place.topic(t)).cloned());
        rows
    };
    refuse_a_chain_over_its_limit(&chain, &rows_of)?;

    let mut files: Vec<File> = Vec::new();
    let mut sections: Vec<String> = Vec::new();
    let mut withheld: Vec<Withheld> = Vec::new();
    let mut trouble: Option<String> = None;
    let mut served: Vec<String> = Vec::new();
    for place in &chain {
        for row in rows_of(place) {
            let Some(text) = tree.text(&row.file) else {
                let why = tree
                    .problems
                    .iter()
                    .find(|p| p.where_.as_ref().and_then(|w| w.file.as_deref()) == Some(row.file.as_str()))
                    .map(|p| p.line())
                    .unwrap_or_else(|| "it could not be read".to_owned());
                sections.push(format!("## {}\n\n**`{}` was not served: {why}**\n", row.file, row.file));
                files.push(File { place: place.kind, path: row.file, bytes: row.bytes, state: FileState::Unread });
                continue;
            };
            let allowed = match decide(&row.file, text) {
                Ok(decision) => decision != Decision::Untrusted,
                Err(e) => {
                    trouble.get_or_insert(e);
                    false
                }
            };
            if allowed {
                sections.push(format!("## {}\n\n{}\n", row.file, text.trim_end()));
                served.push(row.file.clone());
                files.push(File { place: place.kind, path: row.file, bytes: row.bytes, state: FileState::Served });
            } else {
                // **One line stands where the text would have been, and the text goes no further.**
                sections.push(format!("## {}\n\n**{}**\n", row.file, trust::withheld_text_sentence(&tree.root, &row.file)));
                withheld.push(Withheld { path: row.file.clone(), text: text.to_owned() });
                files.push(File { place: place.kind, path: row.file, bytes: row.bytes, state: FileState::Withheld });
            }
        }
    }
    let (pictures, left_out) = pictures_of(tree, &chain, &rows_of, &served);
    let mut text = if sections.is_empty() {
        let about = match request.topic.as_deref() {
            Some(t) => format!(" for the topic `{t}`"),
            None => String::new(),
        };
        format!("No style has been written for this place{about}: none of the files `design/` holds applies to it.\n")
    } else {
        sections.join("\n")
    };
    if !left_out.is_empty() {
        text.push_str("\n## Pictures not attached\n\n");
        for l in &left_out {
            let caption = if l.caption.is_empty() { "no caption".to_owned() } else { format!("\"{}\"", l.caption) };
            text.push_str(&format!("- `{}`, cited by `{}`, {caption}: {}\n", l.file, l.cited_by, l.reason));
        }
    }
    Ok(Answer { text, files, pictures, left_out, withheld, trouble })
}

/// **A request that names something real, or the reason it does not.**
fn check(tree: &Tree, request: &Request) -> Result<(), String> {
    if request.at.is_some() && request.map.is_none() {
        return Err("`at` is a point on a map, and no map was named. Name the map, or leave `at` out.".to_owned());
    }
    if let Some(map) = request.map.as_deref() {
        let Some(floor) = tree.maps.get(map) else {
            let names: Vec<String> = tree.maps.keys().map(|n| format!("`{n}`")).collect();
            let have = if names.is_empty() { "it has no maps".to_owned() } else { format!("its maps are {}", names.join(", ")) };
            return Err(format!("the project has no map `{map}`; {have}."));
        };
        if let Some((x, z)) = request.at {
            if !x.is_finite() || !z.is_finite() {
                return Err(format!("`at` is [{x}, {z}], and a point has two finite numbers."));
            }
            let Some((x0, z0, x1, z1)) = *floor else {
                return Err(format!(
                    "the map `{map}` does not read, so `at` cannot be checked against its floor. Fix the map, or leave `at` out."
                ));
            };
            if !design::inside([x0, z0, x1, z1], (x, z)) {
                return Err(format!(
                    "`at` [{x}, {z}] is outside the map `{map}`, whose floor is [{x0}, {z0}, {x1}, {z1}] in metres."
                ));
            }
        }
    }
    if let Some(topic) = request.topic.as_deref() {
        let game: Vec<&str> =
            tree.place(design::DESIGN_DIR).map(|p| p.topics.iter().map(|t| t.name.as_str()).collect()).unwrap_or_default();
        if !game.contains(&topic) {
            let have = if game.is_empty() {
                "the game states no topics".to_owned()
            } else {
                format!("its topics are {}", game.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", "))
            };
            return Err(format!("the game has no topic `{topic}`; {have}."));
        }
    }
    Ok(())
}

/// **A chain is served whole or refused with its sizes**, because a text an agent half-reads is worse than one it
/// is told is missing.
fn refuse_a_chain_over_its_limit(chain: &[&Place], rows_of: &dyn Fn(&Place) -> Vec<Topic>) -> Result<(), String> {
    let mut rows: Vec<Topic> = chain.iter().flat_map(|p| rows_of(p)).collect();
    let total: usize = rows.iter().map(|r| r.bytes).sum();
    if total <= STYLE_CHAIN_MAX_BYTES {
        return Ok(());
    }
    // **The largest file first**: it is the one a person has to cut.
    rows.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.file.cmp(&b.file)));
    let named: Vec<String> = rows.iter().map(|r| format!("{} ({} bytes)", r.file, r.bytes)).collect();
    Err(format!(
        "the style chain for this place is {total} bytes, over the {STYLE_CHAIN_MAX_BYTES} an agent is served, and a text is never cut: {}. Shorten the largest file, or move part of it into a topic of its own.",
        named.join(", ")
    ))
}

/// **The pictures the served files cite**, nearest place first and then in the order the text cites them, split
/// into what is attached and what is left out.
fn pictures_of(
    tree: &Tree,
    chain: &[&Place],
    rows_of: &dyn Fn(&Place) -> Vec<Topic>,
    served: &[String],
) -> (Vec<Picture>, Vec<LeftOut>) {
    let mut attached: Vec<Picture> = Vec::new();
    let mut left_out: Vec<LeftOut> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let root = std::fs::canonicalize(&tree.root);
    for place in chain.iter().rev() {
        for row in rows_of(place) {
            if !served.contains(&row.file) {
                continue;
            }
            for picture in tree.pictures_of(&row.file) {
                let shown = picture.file.clone().unwrap_or_else(|| picture.cited.clone());
                // **One picture is named once**, by the nearest place that cites it.
                if !seen.insert(shown.clone()) {
                    continue;
                }
                let left = |reason: String| LeftOut {
                    file: shown.clone(),
                    caption: picture.caption.clone(),
                    cited_by: row.file.clone(),
                    reason,
                };
                let Some(rel) = picture.file.as_deref() else {
                    left_out.push(left("it names no file of this project (an address, or a path out of it)".to_owned()));
                    continue;
                };
                match attachable(root.as_deref().ok(), rel) {
                    Err(reason) => left_out.push(left(reason)),
                    Ok(_) if attached.len() >= STYLE_IMAGES_MAX => {
                        left_out.push(left(format!("only {STYLE_IMAGES_MAX} pictures are attached, nearest place first")));
                    }
                    Ok((path, bytes, mime)) => attached.push(Picture {
                        file: rel.to_owned(),
                        path,
                        caption: picture.caption.clone(),
                        cited_by: row.file.clone(),
                        bytes,
                        mime,
                    }),
                }
            }
        }
    }
    (attached, left_out)
}

/// **Whether a picture of the project can be attached**: the file, its size and its type, or the reason not.
fn attachable(root: Option<&std::path::Path>, rel: &str) -> Result<(PathBuf, u64, &'static str), String> {
    let Some(root) = root else { return Err("the project folder cannot be resolved".to_owned()) };
    let Ok(path) = std::fs::canonicalize(root.join(rel)) else { return Err("the file is not there".to_owned()) };
    // **A link that leaves the project is not a picture of it**: the text is the project's, and what it can
    // make an agent read is not.
    if !path.starts_with(root) {
        return Err("the file is outside the project".to_owned());
    }
    let Ok(meta) = std::fs::metadata(&path) else { return Err("the file cannot be read".to_owned()) };
    if !meta.is_file() {
        return Err("it is not a file".to_owned());
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return Err("it is not a PNG, JPEG, GIF or WebP picture".to_owned()),
    };
    if meta.len() > STYLE_IMAGE_MAX_BYTES {
        return Err(format!("it is {} bytes, over the {STYLE_IMAGE_MAX_BYTES} a picture may be", meta.len()));
    }
    Ok((path, meta.len(), mime))
}
