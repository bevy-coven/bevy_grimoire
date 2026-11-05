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
//! `<stem>.png` (the whole frame as it was when the key was pressed), `<stem>_subject.png` (the same
//! frame with the thing that was picked outlined, absent for a whole-screen report), `<stem>.json`
//! (this record) and, when the editor has a log, `<stem>.log` (its tail). [`read_reports`] is the one
//! reader, so the editor that writes them and the agent that lists them agree on the shape.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The record's version, written first so a later change can be migrated.
pub const FEEDBACK_VERSION: u32 = 1;

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
    /// What the person pointed at.
    pub subject: Subject,
    /// What the person typed. May be empty.
    pub note: String,
    /// The whole frame, always.
    pub picture: Picture,
    /// The same frame with the subject outlined. `None` only for [`Subject::Screen`], which is what
    /// the whole picture already shows.
    pub subject_picture: Option<Picture>,
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

/// The picture file the subject is outlined in.
pub fn subject_picture_file(stem: &str) -> String {
    format!("{stem}_subject.png")
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
        let read = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<Report>(&text).map_err(|e| e.to_string()));
        match read {
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
                if let Some(subject) = report.subject_picture.as_mut() {
                    subject.file = subject_picture_file(&stem);
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
    pub subject_picture: Option<PathBuf>,
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

    fn report(taken_at_ms: u64, subject: Subject) -> Report {
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
            subject,
            note: "the panel is cut off".into(),
            picture: picture("feedback-1.png"),
            subject_picture: None,
            log: None,
        }
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
            std::env::temp_dir().join(format!("bevy_grimoire_feedback_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// **Every kind of subject survives the round trip**, screen and scene and area and element — the
    /// record is written by one function and read by another program, so a field that does not
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
            let mut wanted = report(i as u64 + 1, subject);
            wanted.subject_picture = Some(picture("feedback-1_subject.png"));
            let text = serde_json::to_string_pretty(&wanted).expect("a report encodes");
            let got: Report = serde_json::from_str(&text).expect("and decodes");
            assert_eq!(got, wanted, "case {i}");
        }
    }

    /// A subject kind is a tag a reader can match on, not a shape to guess at.
    #[test]
    fn the_subject_names_its_kind() {
        let text = serde_json::to_string(&element()).unwrap();
        assert!(text.starts_with(r#"{"kind":"element""#), "{text}");
        let text = serde_json::to_string(&Subject::Screen).unwrap();
        assert_eq!(text, r#"{"kind":"screen"}"#);
        assert_eq!(
            serde_json::to_string(&Audience::Project).unwrap(),
            r#""project""#
        );
    }

    /// **An unknown field is refused**, so a record from a later version says so instead of dropping
    /// the field an agent needed.
    #[test]
    fn an_unknown_field_is_refused() {
        let mut value = serde_json::to_value(report(1, Subject::Screen)).unwrap();
        value["severity"] = serde_json::json!("high");
        let err = serde_json::from_value::<Report>(value).expect_err("refused");
        assert!(err.to_string().contains("unknown field"), "{err}");
    }

    #[test]
    fn the_stem_is_the_prefix_and_the_millisecond() {
        assert_eq!(stem(1_700_000_000_123), "feedback-1700000000123");
        assert_eq!(picture_file("feedback-7"), "feedback-7.png");
        assert_eq!(subject_picture_file("feedback-7"), "feedback-7_subject.png");
        assert_eq!(record_file("feedback-7"), "feedback-7.json");
        assert_eq!(log_file("feedback-7"), "feedback-7.log");
    }

    /// **The newest reads first**, by the millisecond it was taken at.
    #[test]
    fn the_reader_lists_the_newest_first() {
        let dir = fresh_dir("newest");
        std::fs::create_dir_all(&dir).unwrap();
        for ms in [3u64, 1, 2] {
            let stem = stem(ms);
            std::fs::write(dir.join(picture_file(&stem)), b"png").unwrap();
            let text = serde_json::to_string(&report(ms, Subject::Screen)).unwrap();
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
        let text = serde_json::to_string(&report(10, Subject::Screen)).unwrap();
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
