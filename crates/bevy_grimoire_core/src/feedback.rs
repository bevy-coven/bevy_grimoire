//! **A report a person filed about what is wrong on screen: the record, and the folder it lives in.**
//!
//! Two audiences, one record. `Cmd+D` files a report about **the editor itself**, for the people who
//! write `bevy_cauldron`, and it needs no project open; `Cmd+Shift+D` files one about **the project
//! that is open**, for whoever is working that project. The pick-and-note machinery is written once
//! (`bevy_cauldron::report`) and parameterised by [`Audience`]; what is separate is where the files
//! land and which agent reads them.
//!
//! # Data only
//!
//! Nothing here imports `bevy`, nothing here reads a window and nothing here encodes a PNG — the
//! editor does that, because it is the only thing with a framebuffer. `tests/engine_free.rs` fails
//! the build on an engine dependency, and this module is the reason a second program (the MCP
//! server) can read a report without a renderer.
//!
//! # The four files of one report
//!
//! `<stem>.png` (the whole frame as it was when the key was pressed), `<stem>_picks.png` (the same
//! frame with every pick outlined and the overlap between two of them washed, absent for a
//! whole-screen report), `<stem>.json` (this record) and, when the editor has a log, `<stem>.log`
//! (its tail). [`read_reports`] is the one reader, so the editor that writes them and the agent that
//! lists them agree on the shape.
//!
//! # Several picks, and the relation between them
//!
//! A person points at one thing, or shift-clicks several. "This label is under the viewport" is a
//! fact about two elements, so the record carries [`Pick`]s and, worked out from their rectangles,
//! the [`Pair`] for every two of them: [`PairKind::Contains`], `Overlaps`, `Touches` or `Apart`, the
//! shared area, and which of the two is drawn in front. [`pairs`] is the only place that decision is
//! made, and it is pure — no `bevy`, no window, so the editor's live hover and a reader of a saved
//! record cannot disagree.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The record's version, written first so a later change can be refused by name.
///
/// **2 is the picks record**: `subject` and `subject_picture` became `picks` and `picks_picture`, and
/// a [`Pick`] carries where the pointer was. There are no version-1 files to migrate — the folder was
/// empty when this landed — so [`read_reports`] names a record from another version rather than
/// converting it.
pub const FEEDBACK_VERSION: u32 = 2;

/// **How far apart two edges may be and still count as meeting**, in surface pixels.
///
/// Layout rounds to whole pixels, so two boxes that abut in a design often land one pixel apart; a
/// relation that called that `Apart` would say two touching elements have nothing to do with each
/// other. One pixel is wide enough for the rounding and far narrower than any real gap.
pub const EDGE_TOUCH_PX: f32 = 1.0;

/// Where a **project** report is written, under the project root. Never `assets/grimoire`: a game
/// loads that folder, and a report is not a thing a game reads.
pub const PROJECT_DIR: &str = "target/feedback";

/// Both files of a report start with this and the Unix milliseconds it was taken at.
pub const STEM_PREFIX: &str = "feedback-";

/// **Who a report is for.** The one value the pick-and-note machinery is parameterised by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Audience {
    /// The people who write the editor: `<data_dir>/Grimoire/feedback/`.
    Developers,
    /// Whoever works the open project: `<project>/target/feedback/`.
    Project,
}

impl Audience {
    /// How a person says which channel a report went to.
    pub fn channel(self) -> &'static str {
        match self {
            Audience::Developers => "the people who make Cauldron",
            Audience::Project => "this project",
        }
    }
}

/// **One report, as the file beside its pictures.**
///
/// An agent answers "what was wrong, where, and what did the screen look like" from this file alone,
/// without the editor running. Every field is required: a report is written by one function, and a
/// record that parses is a record the editor wrote.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// [`FEEDBACK_VERSION`].
    pub version: u32,
    /// The name both files share, `feedback-<unix millis>`. The chord's millisecond, never the
    /// save's, so a slow pick cannot collide with a later report.
    pub stem: String,
    /// When the report was taken, Unix milliseconds. The newest reads first.
    pub taken_at_ms: u64,
    pub audience: Audience,
    /// The editor's own version, so a report names the build that was wrong.
    pub app_version: String,
    /// Which screen was up: `launcher`, `loading`, `project` or `editor`.
    pub screen: String,
    /// The project root, absolute. `None` on the Projects home, where there is no project.
    pub project: Option<String>,
    /// The map that was open, when one was.
    pub map: Option<String>,
    /// The tool armed on the Map door, by name; `None` when no tool was.
    pub tool: Option<String>,
    /// The window's shape, so a picture's pixels can be read back as what the person saw.
    pub window: WindowRecord,
    /// **Every thing the person pointed at**, in the order they pointed: one entry for a click, more
    /// for shift-clicks. Never empty.
    pub picks: Vec<Pick>,
    /// **How each two of the picks sit against each other**, worked out from their rectangles by
    /// [`pairs`] when the record was written. One entry per pair `a < b`, so it reads as a triangle:
    /// three picks have three pairs, not six.
    pub relations: Vec<Pair>,
    /// What the person typed. May be empty.
    pub note: String,
    /// The whole frame, always.
    pub picture: Picture,
    /// The same frame with every pick outlined and every overlap washed. `None` only when the only
    /// pick is [`Subject::Screen`], which is what the whole picture already shows.
    pub picks_picture: Option<Picture>,
    /// The log tail's file name, when one was written.
    pub log: Option<String>,
}

/// **The window a report was taken in.**
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowRecord {
    /// The surface's size in physical pixels: this is the picture's size.
    pub physical: [u32; 2],
    /// The same size in the window's logical pixels.
    pub logical: [f32; 2],
    pub scale_factor: f32,
    /// The interface scale the layout was drawn at.
    pub ui_scale: f32,
}

/// **One thing the person pointed at.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pick {
    pub subject: Subject,
    /// Where the pointer was, in surface pixels. A [`Subject::Scene`] pick is drawn as a cross here,
    /// and an element or area pick keeps it so a reader can say which pixel was meant.
    pub at: [f32; 2],
}

/// **How two picks sit against each other**, worked out from their rectangles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PairKind {
    /// `a`'s rectangle encloses `b`'s, edges included.
    Contains,
    /// Their rectangles share pixels but neither encloses the other.
    Overlaps,
    /// Their edges meet, within [`EDGE_TOUCH_PX`], and their interiors do not.
    Touches,
    /// Their rectangles share nothing.
    Apart,
}

/// **One ordered pair of picks.** Emitted for every pair `a < b`, so the list reads as a triangle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// The earlier pick's index in [`Report::picks`].
    pub a: usize,
    /// The later pick's index.
    pub b: usize,
    pub kind: PairKind,
    /// The shared area, in square surface pixels. Zero for `Touches` and `Apart`.
    pub overlap_px2: f32,
    /// **Whichever of the two is drawn in front**: the index of `a` or `b`. Where neither is in the
    /// interface (a `Screen` pick, or a scene point), it is `a` — the pair still answers, and the
    /// record does not pretend to know something the stack did not say.
    pub front: usize,
}

/// **The relation between two picks, and its overlap**: the only place a relation is decided.
///
/// `rects[i]` is pick `i`'s rectangle as `[x0, y0, x1, y1]` in surface pixels, or `None` when it has
/// none — a [`Subject::Screen`] pick. `front[i]` is pick `i`'s position in the front-to-back stack,
/// lower being further front. A pair with no rectangle on either side is [`PairKind::Apart`] with no
/// overlap and `a` in front, which is the honest answer: nothing was compared.
///
/// One entry per pair `a < b`, in that order, so a reader knows the shape before it reads it.
pub fn pairs(picks: &[Pick], rects: &[Option<[f32; 4]>], front: &[usize]) -> Vec<Pair> {
    let mut out = Vec::new();
    for a in 0..picks.len() {
        for b in a + 1..picks.len() {
            let (Some(ra), Some(rb)) = (rects.get(a).copied().flatten(), rects.get(b).copied().flatten())
            else {
                out.push(Pair {
                    a,
                    b,
                    kind: PairKind::Apart,
                    overlap_px2: 0.0,
                    front: a,
                });
                continue;
            };
            let shared = overlap_rect(ra, rb);
            let overlap_px2 = shared.map_or(0.0, |r| {
                (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0)
            });
            let kind = if encloses(ra, rb) {
                PairKind::Contains
            } else if shared.is_some() {
                PairKind::Overlaps
            } else if edges_meet(ra, rb) {
                PairKind::Touches
            } else {
                PairKind::Apart
            };
            let fa = front.get(a).copied().unwrap_or(a);
            let fb = front.get(b).copied().unwrap_or(b);
            out.push(Pair {
                a,
                b,
                kind,
                overlap_px2,
                front: if fb < fa { b } else { a },
            });
        }
    }
    out
}

/// **The shared rectangle of two**, or `None` when their interiors do not meet.
///
/// **A [`PairKind::Touches`] is this returning `None` while [`edges_meet`] says the edges are within
/// [`EDGE_TOUCH_PX`] on some axis** — two boxes that abut share a line, which is zero pixels of area,
/// so a zero-width rectangle is not a rectangle here.
pub fn overlap_rect(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = a[2].min(b[2]);
    let y1 = a[3].min(b[3]);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
}

/// Does `a` enclose `b`, edges included? Both orders are asked by [`pairs`], so containment is
/// whichever way round the two picks were made.
fn encloses(a: [f32; 4], b: [f32; 4]) -> bool {
    let within = |lo: f32, hi: f32, blo: f32, bhi: f32| blo >= lo && bhi <= hi;
    within(a[0], a[2], b[0], b[2]) && within(a[1], a[3], b[1], b[3])
        || within(b[0], b[2], a[0], a[2]) && within(b[1], b[3], a[1], a[3])
}

/// **Do the two boxes' edges meet**, within [`EDGE_TOUCH_PX`] on one axis and while they share some
/// extent on the other? The test that makes [`PairKind::Touches`] possible where [`overlap_rect`]
/// finds no area: two boxes that abut share a line of zero width.
fn edges_meet(a: [f32; 4], b: [f32; 4]) -> bool {
    let near = |p: f32, q: f32| (p - q).abs() <= EDGE_TOUCH_PX;
    let shares_x = a[0] <= b[2] + EDGE_TOUCH_PX && b[0] <= a[2] + EDGE_TOUCH_PX;
    let shares_y = a[1] <= b[3] + EDGE_TOUCH_PX && b[1] <= a[3] + EDGE_TOUCH_PX;
    let vertical = (near(a[0], b[2]) || near(a[2], b[0])) && shares_y;
    let horizontal = (near(a[1], b[3]) || near(a[3], b[1])) && shares_x;
    vertical || horizontal
}

/// **What the person pointed at.** The tag is `kind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Subject {
    /// A node of the interface, with the chain of nodes it sits in.
    Element(Element),
    /// A dragged box, in surface pixels.
    Area { rect_px: [f32; 4] },
    /// The whole screen. Nobody picked anything.
    Screen,
    /// A point in the map camera's viewport.
    Scene(Scene),
}

/// **A node of the interface, and how to find it again.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    /// `Entity::to_bits()`. The same run's entity; a restart renumbers the world.
    pub entity: u64,
    /// The node's rectangle in surface pixels: `[x0, y0, x1, y1]`.
    pub rect_px: [f32; 4],
    /// Outermost first, the picked node last.
    pub path: Vec<PathStep>,
}

/// **One node in the chain under the pointer**, described by what a reader can act on.
///
/// Type names are not used: in this workspace `ComponentInfo::name` only answers with Bevy's `debug`
/// feature, and most component names read as a sentence about the feature being off.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathStep {
    /// The node's `Name`, when it has one.
    pub name: Option<String>,
    /// `format!("{:?}", bevy_gooey::Role)`, when it carries one.
    pub role: Option<String>,
    /// `format!("{:?}", chrome::Control)`, when the node is where a verb lives.
    pub control: Option<String>,
    /// The node's own `Text`, else its first descendant's. Trimmed, capped at [`TEXT_CAP`].
    pub text: Option<String>,
}

/// What a [`PathStep`]'s `text` is cut to, in characters. Long enough for a button's words, short
/// enough that a panel's paragraph does not become the record.
pub const TEXT_CAP: usize = 120;

/// **A point in the map camera's viewport.** Absent fields are the honest answer when there is no
/// map open: the world point and the camera are still worth having.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    /// The ground point under the click, in world metres.
    pub world: [f32; 3],
    /// The same point in map space, when a map is open.
    pub map: Option<[f32; 2]>,
    /// The map cell under it, when a map is open.
    pub cell: Option<[i32; 2]>,
    /// The placement the editor's own `pick_at` chose there, by id.
    pub placement: Option<String>,
    /// Where the camera was.
    pub camera: CameraPose,
}

/// **Where a camera was looking.**
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraPose {
    pub translation: [f32; 3],
    pub forward: [f32; 3],
}

/// **One picture of a report**: its file name beside the record, and its size in pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Picture {
    pub file: String,
    pub size: [u32; 2],
}

/// The name a report carries, `feedback-<unix millis>`.
pub fn stem(taken_at_ms: u64) -> String {
    format!("{STEM_PREFIX}{taken_at_ms}")
}

/// The last `count` lines of `text`, with the trailing newline kept when there is one.
///
/// The tail of a long log, for the report's `.log`: an agent wants the lines around the mistake, not
/// two megabytes of history. A file with fewer lines than that is kept whole.
pub fn last_lines(text: &str, count: usize) -> String {
    if count == 0 {
        return String::new();
    }
    let ends_with_newline = text.ends_with('\n');
    let mut lines: Vec<&str> = text.lines().rev().take(count).collect();
    lines.reverse();
    let mut out = lines.join("\n");
    if ends_with_newline && !out.is_empty() {
        out.push('\n');
    }
    out
}

/// The picture file a report's whole frame is written to.
pub fn picture_file(stem: &str) -> String {
    format!("{stem}.png")
}

/// The picture file every pick is outlined in.
pub fn picks_picture_file(stem: &str) -> String {
    format!("{stem}_picks.png")
}

/// The record's own file.
pub fn record_file(stem: &str) -> String {
    format!("{stem}.json")
}

/// The log tail's file.
pub fn log_file(stem: &str) -> String {
    format!("{stem}.log")
}

/// **Read every report in `dir`, newest first, and name every file that is not one.**
///
/// A `.json` that parses and whose `<stem>.png` exists is a report. Anything else — a file that does
/// not parse, a record whose picture is missing — is named in the second list rather than hiding the
/// rest, exactly as [`bevy_grimoire_snip::read_snips`] does for a snip. A folder that does not exist
/// has no reports and no problems.
///
/// **The file's own name names the pictures**, never a stem written inside the record: a stem with a
/// path separator in it would point the picture outside the folder.
pub fn read_reports(dir: &Path) -> (Vec<Report>, Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), Vec::new()),
        Err(e) => {
            return (
                Vec::new(),
                vec![format!("could not read {}: {e}", dir.display())],
            );
        }
    };
    let mut reports = Vec::new();
    let mut problems = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let read = std::fs::read_to_string(&path).map_err(|e| e.to_string());
        let text = match read {
            Ok(text) => text,
            Err(e) => {
                problems.push(format!("{} is not a feedback report: {e}", path.display()));
                continue;
            }
        };
        // **The version is read before the shape.** A version-1 record has a `subject` key this
        // build does not know, so `deny_unknown_fields` would refuse it — true, but it would say
        // "unknown field `subject`" when the real answer is that the file is one version old. This
        // is that sentence.
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                problems.push(format!("{} is not a feedback report: {e}", path.display()));
                continue;
            }
        };
        if let Some(version) = value.get("version").and_then(serde_json::Value::as_u64)
            && version != u64::from(FEEDBACK_VERSION)
        {
            problems.push(format!(
                "{} is version {version}, and this build reads {FEEDBACK_VERSION}",
                path.display()
            ));
            continue;
        }
        match serde_json::from_value::<Report>(value) {
            Ok(mut report) => {
                let png = dir.join(picture_file(&stem));
                if !png.is_file() {
                    problems.push(format!(
                        "{} has no picture {}, so it is not listed",
                        path.display(),
                        png.display()
                    ));
                    continue;
                }
                report.stem = stem.clone();
                report.picture.file = picture_file(&stem);
                if let Some(picks) = report.picks_picture.as_mut() {
                    picks.file = picks_picture_file(&stem);
                }
                reports.push(report);
            }
            Err(e) => problems.push(format!("{} is not a feedback report: {e}", path.display())),
        }
    }
    reports.sort_by(|a, b| {
        b.taken_at_ms
            .cmp(&a.taken_at_ms)
            .then_with(|| b.stem.cmp(&a.stem))
    });
    (reports, problems)
}

/// The four paths one report is written to, for an editor that wants to say where it went.
pub struct ReportFiles {
    pub picture: PathBuf,
    pub picks_picture: Option<PathBuf>,
    pub record: PathBuf,
    pub log: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(file: &str) -> Picture {
        Picture {
            file: file.to_owned(),
            size: [1280, 720],
        }
    }

    /// A pick of `subject`, with the pointer at `at`.
    fn pick(subject: Subject, at: [f32; 2]) -> Pick {
        Pick { subject, at }
    }

    fn report(taken_at_ms: u64, picks: Vec<Pick>) -> Report {
        let rects = rects_of(&picks);
        let front: Vec<usize> = (0..picks.len()).collect();
        Report {
            version: FEEDBACK_VERSION,
            stem: stem(taken_at_ms),
            taken_at_ms,
            audience: Audience::Developers,
            app_version: "0.2.3".into(),
            screen: "editor".into(),
            project: Some("/tmp/project".into()),
            map: Some("m".into()),
            tool: Some("Place".into()),
            window: WindowRecord {
                physical: [1280, 720],
                logical: [640.0, 360.0],
                scale_factor: 2.0,
                ui_scale: 1.2,
            },
            relations: pairs(&picks, &rects, &front),
            picks,
            note: "the panel is cut off".into(),
            picture: picture("feedback-1.png"),
            picks_picture: None,
            log: None,
        }
    }

    /// Each pick's rectangle, or `None` where its subject has none.
    fn rects_of(picks: &[Pick]) -> Vec<Option<[f32; 4]>> {
        picks
            .iter()
            .map(|pick| match &pick.subject {
                Subject::Element(element) => Some(element.rect_px),
                Subject::Area { rect_px } => Some(*rect_px),
                Subject::Screen | Subject::Scene(_) => None,
            })
            .collect()
    }

    fn element() -> Subject {
        Subject::Element(Element {
            entity: 42,
            rect_px: [10.0, 20.0, 110.0, 60.0],
            path: vec![
                PathStep {
                    name: Some("panel".into()),
                    role: Some("Panel".into()),
                    control: None,
                    text: None,
                },
                PathStep {
                    name: None,
                    role: Some("Button".into()),
                    control: Some("Back".into()),
                    text: Some("Save".into()),
                },
            ],
        })
    }

    fn scene() -> Subject {
        Subject::Scene(Scene {
            world: [1.0, 0.0, 2.0],
            map: Some([1.0, 2.0]),
            cell: Some([1, 2]),
            placement: Some("chair_3".into()),
            camera: CameraPose {
                translation: [0.0, 12.0, 12.0],
                forward: [0.0, -1.0, -1.0],
            },
        })
    }

    fn fresh_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("grimoire_feedback_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// **Every kind of subject survives the round trip**, screen and scene and area and element, in
    /// one report of each — and then a report of two, because the thing a single-pick record never
    /// proved is that the list and the relation between its entries survive it.
    ///
    /// The record is written by one function and read by another program, so a field that does not
    /// survive is a field an agent silently never sees.
    #[test]
    fn a_report_of_every_subject_round_trips() {
        for (i, subject) in [
            Subject::Screen,
            Subject::Area {
                rect_px: [4.0, 8.0, 240.0, 160.0],
            },
            element(),
            scene(),
        ]
        .into_iter()
        .enumerate()
        {
            let mut wanted = report(i as u64 + 1, vec![pick(subject, [7.0, 11.0])]);
            wanted.picks_picture = Some(picture("feedback-1_picks.png"));
            let text = serde_json::to_string_pretty(&wanted).expect("a report encodes");
            let got: Report = serde_json::from_str(&text).expect("and decodes");
            assert_eq!(got, wanted, "case {i}");
        }

        // Two picks, one inside the other, with the relation worked out and stored.
        let wanted = report(
            9,
            vec![
                pick(element(), [20.0, 30.0]),
                pick(
                    Subject::Area {
                        rect_px: [40.0, 30.0, 80.0, 50.0],
                    },
                    [60.0, 40.0],
                ),
            ],
        );
        assert_eq!(wanted.relations.len(), 1, "two picks make one pair");
        let text = serde_json::to_string_pretty(&wanted).expect("a report encodes");
        let got: Report = serde_json::from_str(&text).expect("and decodes");
        assert_eq!(got, wanted);
    }

    /// A subject kind is a tag a reader can match on, not a shape to guess at — and a
    /// [`PairKind`] is a lowercase string, because an agent branches on it.
    #[test]
    fn a_pick_names_its_kind() {
        let text = serde_json::to_string(&pick(element(), [1.0, 2.0])).unwrap();
        assert!(
            text.contains(r#"{"kind":"element""#) && text.contains(r#""at":[1.0,2.0]"#),
            "{text}"
        );
        let text = serde_json::to_string(&Subject::Screen).unwrap();
        assert_eq!(text, r#"{"kind":"screen"}"#);
        assert_eq!(
            serde_json::to_string(&Audience::Project).unwrap(),
            r#""project""#
        );
        for (kind, wanted) in [
            (PairKind::Contains, r#""contains""#),
            (PairKind::Overlaps, r#""overlaps""#),
            (PairKind::Touches, r#""touches""#),
            (PairKind::Apart, r#""apart""#),
        ] {
            assert_eq!(serde_json::to_string(&kind).unwrap(), wanted);
        }
    }

    /// **An unknown field is refused**, so a record from a later version says so instead of dropping
    /// the field an agent needed.
    #[test]
    fn an_unknown_field_is_refused() {
        let mut value =
            serde_json::to_value(report(1, vec![pick(Subject::Screen, [0.0, 0.0])])).unwrap();
        value["severity"] = serde_json::json!("high");
        let err = serde_json::from_value::<Report>(value).expect_err("refused");
        assert!(err.to_string().contains("unknown field"), "{err}");
    }

    #[test]
    fn the_stem_is_the_prefix_and_the_millisecond() {
        assert_eq!(stem(1_700_000_000_123), "feedback-1700000000123");
        assert_eq!(picture_file("feedback-7"), "feedback-7.png");
        assert_eq!(picks_picture_file("feedback-7"), "feedback-7_picks.png");
        assert_eq!(record_file("feedback-7"), "feedback-7.json");
        assert_eq!(log_file("feedback-7"), "feedback-7.log");
    }

    /// **One pair per two picks, and every relation is the geometry's answer**: the shared area for an
    /// overlap, nothing for a meeting edge or a gap, and the front-most of the two by its stack index.
    #[test]
    fn pairs_names_overlap_touches_and_apart() {
        let square = |x: f32, y: f32, side: f32| [x, y, x + side, y + side];
        let picks = vec![
            pick(Subject::Area { rect_px: [0.0; 4] }, [0.0, 0.0]),
            pick(Subject::Area { rect_px: [0.0; 4] }, [0.0, 0.0]),
            pick(Subject::Area { rect_px: [0.0; 4] }, [0.0, 0.0]),
        ];
        let rects = vec![
            Some(square(0.0, 0.0, 4.0)),
            Some(square(3.0, 3.0, 4.0)),
            Some(square(9.0, 9.0, 1.0)),
        ];
        // The third pick is drawn in front of the first two, and the second is behind both.
        let front = vec![2, 1, 0];
        let got = pairs(&picks, &rects, &front);
        assert_eq!(got.len(), 3, "three picks are three pairs, not six: {got:?}");

        let first = got.iter().find(|p| (p.a, p.b) == (0, 1)).expect("(0, 1)");
        assert_eq!(first.kind, PairKind::Overlaps);
        assert_eq!(first.overlap_px2, 1.0, "1 × 1 of shared corner");
        assert_eq!(first.front, 1, "the second pick is nearer the front");

        let apart = got.iter().find(|p| (p.a, p.b) == (0, 2)).expect("(0, 2)");
        assert_eq!(apart.kind, PairKind::Apart);
        assert_eq!(apart.overlap_px2, 0.0);
        assert_eq!(apart.front, 2);

        // A box that only meets another along an edge: the interiors share nothing, the edges touch.
        let touching = pairs(
            &picks[..2],
            &[Some(square(0.0, 0.0, 4.0)), Some(square(4.0, 0.0, 4.0))],
            &[0, 1],
        );
        assert_eq!(touching[0].kind, PairKind::Touches, "{touching:?}");
        assert_eq!(touching[0].overlap_px2, 0.0);
    }

    /// **An enclosing rectangle is `Contains`, not `Overlaps`** — the distinction is the whole reason
    /// a report of two elements can say "this label is inside that panel" rather than "these two
    /// share pixels".
    #[test]
    fn an_enclosing_rect_is_contains_not_overlaps() {
        let picks = vec![
            pick(Subject::Area { rect_px: [0.0; 4] }, [0.0, 0.0]),
            pick(Subject::Area { rect_px: [0.0; 4] }, [0.0, 0.0]),
        ];
        // The outer one first, then the same pair the other way round: containment does not depend on
        // which pick was made first.
        let outer = Some([0.0, 0.0, 100.0, 80.0]);
        let inner = Some([20.0, 10.0, 60.0, 40.0]);
        for rects in [vec![outer, inner], vec![inner, outer]] {
            let got = pairs(&picks, &rects, &[0, 1]);
            assert_eq!(got[0].kind, PairKind::Contains, "{rects:?}");
            assert_eq!(got[0].overlap_px2, 1200.0, "40 × 30, the inner box");
        }
        // The same rectangle twice encloses itself, and its overlap is its whole area.
        let same = pairs(&picks, &[outer, outer], &[0, 1]);
        assert_eq!(same[0].kind, PairKind::Contains);
        assert_eq!(same[0].overlap_px2, 8000.0);
    }

    /// **A pick with no rectangle is compared to nothing**, and says so: `Apart`, no area, `a` in
    /// front. The pair is still emitted, because an agent that was told two things were picked must
    /// be told there are two.
    #[test]
    fn a_pick_without_a_rectangle_is_apart_from_everything() {
        let picks = vec![
            pick(Subject::Screen, [0.0, 0.0]),
            pick(Subject::Area { rect_px: [0.0; 4] }, [1.0, 1.0]),
        ];
        let got = pairs(&picks, &[None, Some([0.0, 0.0, 10.0, 10.0])], &[0, 1]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, PairKind::Apart);
        assert_eq!(got[0].overlap_px2, 0.0);
        assert_eq!(got[0].front, 0);
    }

    /// **The newest reads first**, by the millisecond it was taken at.
    #[test]
    fn the_reader_lists_the_newest_first() {
        let dir = fresh_dir("newest");
        std::fs::create_dir_all(&dir).unwrap();
        for ms in [3u64, 1, 2] {
            let stem = stem(ms);
            std::fs::write(dir.join(picture_file(&stem)), b"png").unwrap();
            let text = serde_json::to_string(&report(ms, vec![pick(Subject::Screen, [0.0, 0.0])]))
                .unwrap();
            std::fs::write(dir.join(record_file(&stem)), text).unwrap();
        }
        let (reports, problems) = read_reports(&dir);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            reports.iter().map(|r| r.taken_at_ms).collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert_eq!(reports[0].picture.file, "feedback-3.png");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A file that is not a report is named, never a panic**, and a record with no picture is
    /// named too — the list still answers.
    #[test]
    fn the_reader_names_what_is_not_a_report() {
        let dir = fresh_dir("problems");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("feedback-9.json"), "{\"nope\": true}").unwrap();
        let stem = stem(10);
        let text =
            serde_json::to_string(&report(10, vec![pick(Subject::Screen, [0.0, 0.0])])).unwrap();
        std::fs::write(dir.join(record_file(&stem)), text).unwrap();
        let (reports, problems) = read_reports(&dir);
        assert!(reports.is_empty(), "{reports:?}");
        assert_eq!(problems.len(), 2, "{problems:?}");
        // The order is the folder's, not the reader's: two problems, both named, neither hidden.
        assert!(
            problems.iter().any(|p| p.contains("feedback-9.json is not a feedback report")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p
                .contains("feedback-10.json has no picture")
                && p.contains("not listed")),
            "{problems:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A record from an older version is named, not read.** Version 1 has a `subject` key this
    /// build does not know, so without the version check the reader would say "unknown field
    /// `subject`" — true, and useless: the answer is that the file is one version old.
    #[test]
    fn a_record_from_an_older_version_is_named_not_read() {
        let dir = fresh_dir("version");
        std::fs::create_dir_all(&dir).unwrap();
        let stem = stem(11);
        std::fs::write(dir.join(picture_file(&stem)), b"png").unwrap();
        // A version-1 file as it was actually written: `subject`, `subject_picture`, version 1.
        let old = serde_json::json!({
            "version": 1,
            "stem": stem,
            "taken_at_ms": 11,
            "audience": "developers",
            "app_version": "0.2.3",
            "screen": "editor",
            "project": null,
            "map": null,
            "tool": null,
            "window": {
                "physical": [1280, 720],
                "logical": [640.0, 360.0],
                "scale_factor": 2.0,
                "ui_scale": 1.2,
            },
            "subject": {"kind": "screen"},
            "note": "",
            "picture": {"file": "feedback-11.png", "size": [1280, 720]},
            "subject_picture": null,
            "log": null,
        });
        std::fs::write(
            dir.join(record_file(&stem)),
            serde_json::to_string(&old).unwrap(),
        )
        .unwrap();
        let (reports, problems) = read_reports(&dir);
        assert!(reports.is_empty(), "{reports:?}");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("is version 1, and this build reads 2"),
            "{problems:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder that is not there has no reports and no problems: a first run is not a fault.
    #[test]
    fn a_missing_folder_is_no_reports() {
        let dir = fresh_dir("missing");
        assert_eq!(read_reports(&dir), (Vec::new(), Vec::new()));
    }

    #[test]
    fn the_log_tail_is_the_last_lines_and_keeps_its_newline() {
        assert_eq!(last_lines("a\nb\nc\n", 2), "b\nc\n");
        assert_eq!(last_lines("a\nb\nc", 2), "b\nc");
        assert_eq!(last_lines("a\nb\n", 10), "a\nb\n");
        assert_eq!(last_lines("", 10), "");
        assert_eq!(last_lines("a\nb\n", 0), "");
    }
}
