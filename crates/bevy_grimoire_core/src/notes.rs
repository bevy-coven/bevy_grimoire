//! **Notes for agents: the data, its file beside the map, and the rules that keep it honest.**
//!
//! A person marks tiles or placed meshes with a note; an agent lists the open ones, does the work
//! through ordinary operations and resolves the note. This module is the engine-free half of that:
//! what a note is, where it is written, and the four changes a note can undergo. The operations
//! that make those changes undoable and reachable from the UI and MCP are built on top of it.
//!
//! # Where it lives
//!
//! One file per map, **beside the map**: `maps/town.map.yaml` has `maps/town.notes.yaml`. It is
//! written through [`crate::save::save_atomic`] like the map is, and its extension is `ron`,
//! so a [`crate::live::FileWatch`] over the project already sees it change and a running game or
//! editor reloads it with the rest.
//!
//! # What a note keeps
//!
//! An id (`n1`, `n2`, … never reused: a resolved note stays on record, so its id is spoken for),
//! its targets (placement ids and cells), its text, and its state. Resolving records the ids of the
//! operations the agent ran and one line of what changed, so a resolved note answers "what was done
//! about this".

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// The file format's version, written first so a later change can be migrated.
pub const NOTES_VERSION: u32 = 1;

/// What a note is about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Target {
    /// A placed mesh, by [`crate::map::Placed::id`]. **An id inside this project's map files, not a value of
    /// another file's kind**, so it carries no `x-dark-factory-ref`.
    Placement(String),
    /// A tile, by grid cell.
    Cell(i32, i32),
}

/// Where a note stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum State {
    /// Written by a person and not yet sent: no agent sees it, the sign stands on the map.
    Draft,
    Open,
    /// An agent holds a lease on it (#91). The lease runs to `until` (seconds since the Unix epoch) and
    /// is renewed while the agent works; one not renewed in time expires and the note is `Open` again.
    Claimed { by: String, until: u64 },
    /// Worked. `ops` names the operations the agent ran; `summary` is one line of what changed.
    Resolved {
        ops: Vec<String>,
        summary: String,
        /// What the agent assumed where the note did not say (#92). Empty for a note resolved by hand.
        #[serde(default)]
        assumptions: Vec<String>,
        /// Capture paths of the area before the work and after it (#92); empty for a note resolved by hand.
        #[serde(default)]
        captures: Vec<String>,
    },
    /// The person reviewed the work and kept it (#93): archived, its sign gone. Carries the record
    /// [`State::Resolved`] held, so Cmd+Z on Keep brings the note back to done with nothing lost.
    Kept {
        ops: Vec<String>,
        summary: String,
        #[serde(default)]
        assumptions: Vec<String>,
        #[serde(default)]
        captures: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub id: String,
    pub targets: Vec<Target>,
    pub text: String,
    pub state: State,
    /// Where the person drew it and what stood there; `None` for a note made from targets alone.
    #[serde(default)]
    pub site: Option<Site>,
    /// What happened to this note, oldest first. Append-only: an edit keeps the text it replaced.
    #[serde(default)]
    pub history: Vec<Event>,
    /// The person said "decide these yourself": the holder may not ask about open choices (a wall
    /// height, a colour), it states what it assumed in its resolution. The fence is unchanged, so a
    /// change that would need the person's answer is refused rather than asked about (E-070).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto: bool,
}

/// One thing that happened to a note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Seconds since the Unix epoch.
    pub at: u64,
    /// Who did it: `person` for the editor's own user.
    pub by: String,
    pub what: What,
}

/// What happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum What {
    Written,
    /// The text was changed; `old` is the text it replaced.
    Edited { old: String },
    /// A draft was sent, so an agent may now claim it.
    Sent,
    /// An agent took the lease, to `until`.
    Claimed { until: u64 },
    /// The holder renewed the lease, to `until`.
    Renewed { until: u64 },
    /// The holder, or the person, gave the note back.
    Released,
    /// The lease ran out unrenewed, so the note went back to the queue.
    Expired,
    /// The holder asked the person something before going on (#92).
    Asked { question: String },
    /// The person answered the latest question.
    Answered { answer: String },
    /// The holder resolved the note.
    Resolved,
    /// The person kept the work: the note is archived (#93).
    Kept,
    /// The person took Keep back (Cmd+Z): the note is done again, awaiting review.
    Unkept,
    /// The person undid the note's work as one step; the note is open again with its history intact.
    WorkUndone,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Note {
    /// Appends to the history, by the editor's own user.
    fn log(&mut self, what: What) {
        self.log_at(now(), "person", what);
    }

    fn log_at(&mut self, at: u64, by: &str, what: What) {
        self.history.push(Event { at, by: by.to_owned(), what });
    }
}

/// The place a note was written about, as it was when written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Site {
    /// Inclusive cell rectangle `(min_x, min_z, max_x, max_z)`.
    pub cells: (i32, i32, i32, i32),
    /// World bounds `(min_x, min_z, max_x, max_z)` in metres.
    pub world: (f32, f32, f32, f32),
    /// Ids of the placements inside the area when the note was written.
    pub pieces: Vec<String>,
    /// Cells around the area an agent may also touch.
    pub margin: i32,
    /// Camera when written: `(x, y, z, yaw, pitch)`.
    pub camera: (f32, f32, f32, f32, f32),
}

/// An agent works a note within one cell of its area.
/// The statuses a filter may name, in the order the panel cycles them.
pub const STATUSES: [&str; 3] = ["draft", "sent", "done"];

pub const DEFAULT_MARGIN: i32 = 1;

/// More pieces than this in one note's work must be asked about first (#92).
pub const LARGE_CHANGE: usize = 10;

/// What an agent's change would do, for [`Notes::fence`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// Inclusive cell rectangle the change touches; `None` for one that touches no ground.
    pub cells: Option<(i32, i32, i32, i32)>,
    /// Pieces it adds, removes or moves.
    pub pieces: usize,
    /// Ids of pieces it removes that a person placed (the operation log's author says so).
    pub person_placed: Vec<String>,
    /// What the note asks for that the kit cannot supply, when the agent found something.
    pub needs_unmapped: Option<String>,
}

/// What the holder records when it resolves a note.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolution {
    pub summary: String,
    pub assumptions: Vec<String>,
    pub ops: Vec<String>,
    /// `[before, after]` capture paths.
    pub captures: Vec<String>,
}

/// What one logged operation touched: the placement ids and the grid cells, read from the operation
/// log by the editor (#93). Engine-free, so the review rule is provable here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Footprint {
    /// The operation's id, as the log names it.
    pub op: String,
    pub placements: Vec<String>,
    pub cells: Vec<(i32, i32)>,
}

impl Footprint {
    fn shares_with(&self, other: &Footprint) -> Option<String> {
        if let Some(p) = self.placements.iter().find(|p| other.placements.contains(p)) {
            return Some(format!("placement {p}"));
        }
        self.cells.iter().find(|c| other.cells.contains(c)).map(|(x, z)| format!("cell ({x},{z})"))
    }
}

/// The first later operation that depends on the work of `note_ops`: one that touched a placement id
/// or a cell they touched (#93's "depends on"). `later` is every operation logged after the note's
/// first one that is not one of the note's own. Returns `(later op id, what it shares)`.
pub fn undo_blocker(note_ops: &[Footprint], later: &[Footprint]) -> Option<(String, String)> {
    later
        .iter()
        .filter(|l| !note_ops.iter().any(|n| n.op == l.op))
        .find_map(|l| note_ops.iter().find_map(|n| l.shares_with(n)).map(|s| (l.op.clone(), s)))
}

impl Site {
    /// The cell rectangle an agent may work: the area grown by the margin.
    pub fn reach(&self) -> (i32, i32, i32, i32) {
        let (a, b, c, d) = self.cells;
        (a - self.margin, b - self.margin, c + self.margin, d + self.margin)
    }
}

impl Note {
    pub fn is_open(&self) -> bool {
        self.state == State::Open
    }

    /// The agent holding the lease, if any.
    pub fn holder(&self) -> Option<&str> {
        match &self.state {
            State::Claimed { by, .. } => Some(by),
            _ => None,
        }
    }

    /// When this note was sent, for send order: the time of its last `Sent` event.
    fn sent_at(&self) -> u64 {
        self.history.iter().rev().find(|e| e.what == What::Sent).map_or(0, |e| e.at)
    }

    /// The map-space rectangle `[x0, z0, x1, z1]` in metres an agent may work: the drawn area grown
    /// by its margin, else the cells named, grown by the default margin. `None` for a note on
    /// placements only, which claims no ground.
    pub fn reach_rect(&self) -> Option<[f32; 4]> {
        let t = crate::grid::TILE;
        if let Some(s) = &self.site {
            let (a, b, c, d) = s.reach();
            return Some([a as f32 * t, b as f32 * t, (c + 1) as f32 * t, (d + 1) as f32 * t]);
        }
        let m = DEFAULT_MARGIN as f32 * t;
        self.frame_rect().map(|r| [r[0] - m, r[1] - m, r[2] + m, r[3] + m])
    }

    fn require_holder(&self, id: &str, by: &str, did: &str) -> Result<(), String> {
        match self.holder() {
            Some(h) if h == by => Ok(()),
            Some(h) => Err(format!("notes: {id} is claimed by {h}, not {by}, so it was not {did}")),
            None => Err(format!("notes: {id} is not claimed by {by}, so it was not {did}")),
        }
    }

    /// The question the person has not answered yet, if one stands.
    pub fn pending_question(&self) -> Option<&str> {
        self.history
            .iter()
            .rev()
            .find_map(|e| match &e.what {
                What::Asked { question } => Some(Some(question.as_str())),
                What::Answered { .. } => Some(None),
                _ => None,
            })
            .flatten()
    }

    /// Whether the person has answered a question since the agent last claimed the note.
    fn answered_since_claim(&self) -> bool {
        let claim = self.history.iter().rposition(|e| matches!(e.what, What::Claimed { .. })).unwrap_or(0);
        self.history[claim..].iter().any(|e| matches!(e.what, What::Answered { .. }))
    }

    /// The inclusive cell rectangle the person marked: where it was drawn, else the cells named.
    pub fn area_cells(&self) -> Option<(i32, i32, i32, i32)> {
        if let Some(s) = &self.site {
            return Some(s.cells);
        }
        self.targets.iter().filter_map(|t| if let Target::Cell(x, z) = t { Some((*x, *z)) } else { None }).fold(
            None,
            |r: Option<(i32, i32, i32, i32)>, (x, z)| {
                Some(r.map_or((x, z, x, z), |r| (r.0.min(x), r.1.min(z), r.2.max(x), r.3.max(z))))
            },
        )
    }

    /// [`Note::area_cells`] grown by the margin: the ground an agent may work.
    pub fn reach_cells(&self) -> Option<(i32, i32, i32, i32)> {
        let m = self.site.as_ref().map_or(DEFAULT_MARGIN, |s| s.margin);
        self.area_cells().map(|(a, b, c, d)| (a - m, b - m, c + m, d + m))
    }

    pub fn is_draft(&self) -> bool {
        self.state == State::Draft
    }

    /// The word the Notes panel shows and filters by. The set grows as agents claim, ask and are
    /// reviewed (#91–#93); today a note is a draft, sent, or done.
    pub fn status(&self) -> &'static str {
        match self.state {
            State::Draft => "draft",
            State::Open => "sent",
            State::Claimed { .. } => "claimed",
            State::Resolved { .. } => "done",
            State::Kept { .. } => "kept",
        }
    }

    /// The map-space rectangle `[x0, z0, x1, z1]` in metres a camera should frame to show this
    /// note: where it was drawn, else the cells it names. `None` when it names placements only,
    /// which the caller frames by id.
    pub fn frame_rect(&self) -> Option<[f32; 4]> {
        if let Some(s) = &self.site {
            return Some([s.world.0, s.world.1, s.world.2, s.world.3]);
        }
        let t = crate::grid::TILE;
        self.targets
            .iter()
            .filter_map(|t| if let Target::Cell(x, z) = t { Some((*x, *z)) } else { None })
            .fold(None, |r: Option<[f32; 4]>, (x, z)| {
                let (x0, z0, x1, z1) = (x as f32 * t, z as f32 * t, (x + 1) as f32 * t, (z + 1) as f32 * t);
                Some(r.map_or([x0, z0, x1, z1], |r| [r[0].min(x0), r[1].min(z0), r[2].max(x1), r[3].max(z1)]))
            })
    }

    /// A sign stands over it: a draft or an open note, not a resolved one.
    pub fn stands(&self) -> bool {
        !matches!(self.state, State::Resolved { .. } | State::Kept { .. })
    }
}

/// Every note of one map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Notes {
    pub version: u32,
    pub notes: Vec<Note>,
    /// How long an agent's lease on a note lasts before it must renew, in seconds. A project setting:
    /// edit the file and the next claim uses it.
    #[serde(default = "default_lease")]
    pub lease_secs: u64,
}

/// Five minutes.
pub const DEFAULT_LEASE_SECS: u64 = 300;

fn default_lease() -> u64 {
    DEFAULT_LEASE_SECS
}

impl Default for Notes {
    fn default() -> Self {
        Notes { version: NOTES_VERSION, notes: Vec::new(), lease_secs: DEFAULT_LEASE_SECS }
    }
}

/// The notes file that belongs to a map file: `town.map.yaml` → `town.notes.yaml`, in the same
/// directory. A map path that does not end `.map.yaml` gets `.notes.yaml` appended to its stem.
pub fn path_for_map(map_path: &Path) -> PathBuf {
    let name = map_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = name
        .strip_suffix(crate::map::MAP_SUFFIX)
        .or_else(|| name.strip_suffix(".yaml"))
        .unwrap_or(&name);
    map_path.with_file_name(format!("{stem}.notes.yaml"))
}

impl Notes {
    pub fn parse(text: &str) -> Result<Notes, String> {
        let notes: Notes =
            crate::yaml::from_str(text, "notes").map_err(|p| format!("notes: parse: {}", p.line()))?;
        if notes.version != NOTES_VERSION {
            return Err(format!(
                "notes: version {} is not the {NOTES_VERSION} this build reads",
                notes.version
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for n in &notes.notes {
            if !seen.insert(n.id.as_str()) {
                return Err(format!("notes: id {} appears twice", n.id));
            }
        }
        Ok(notes)
    }

    /// **The file as the one YAML writer writes it**, which ends with a newline already.
    pub fn to_yaml(&self) -> Result<String, String> {
        crate::yaml::to_string(self).map_err(|p| format!("notes: {}", p.line()))
    }

    /// Reads the notes file; a file that is not there is no notes, not an error.
    pub fn load(path: &Path) -> Result<Notes, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Notes::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Notes::default()),
            Err(e) => Err(format!("notes: {}: {e}", path.display())),
        }
    }

    /// Writes the notes file atomically. No notes and no file writes nothing, so a map that never
    /// had a note never grows one.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.notes.is_empty() && !path.exists() {
            return Ok(());
        }
        crate::yaml::save(path, &self.to_yaml()?)
    }

    pub fn get(&self, id: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    /// The notes an agent still has to work, in the order they were left.
    /// Every note, newest first (ids are never reused, so file order is writing order), or only
    /// those whose [`Note::status`] is `status`. Resolved notes are listed like any other.
    pub fn listing(&self, status: Option<&str>) -> Vec<&Note> {
        self.notes.iter().rev().filter(|n| status.is_none_or(|s| n.status() == s)).collect()
    }

    pub fn open(&self) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(|n| n.is_open())
    }

    pub fn resolved(&self) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(|n| !n.stands())
    }

    fn next_id(&self) -> String {
        let top = self
            .notes
            .iter()
            .filter_map(|n| n.id.strip_prefix('n')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("n{}", top + 1)
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Note, String> {
        self.notes.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("notes: no note {id}"))
    }

    /// Leaves a note. A note needs a target and some text.
    pub fn add(&mut self, targets: Vec<Target>, text: &str) -> Result<String, String> {
        if targets.is_empty() {
            return Err("notes: a note needs at least one target".to_owned());
        }
        if text.trim().is_empty() {
            return Err("notes: a note needs some text".to_owned());
        }
        let id = self.next_id();
        self.notes.push(Note { id: id.clone(), targets, text: text.to_owned(),
            state: State::Open,
            site: None,
            history: Vec::new(),
            auto: false,
        });
        self.get_mut(&id)?.log(What::Written);
        Ok(id)
    }

    /// Leaves a draft note on a cell rectangle, recording what the map holds there. The targets
    /// are the placements inside the area (as [`Site::pieces`]) or, when it holds none, its cells.
    pub fn add_area(
        &mut self,
        map: &crate::map::Map,
        cells: (i32, i32, i32, i32),
        cell_size: f32,
        camera: (f32, f32, f32, f32, f32),
        text: &str,
    ) -> Result<String, String> {
        let (x0, z0) = (cells.0.min(cells.2), cells.1.min(cells.3));
        let (x1, z1) = (cells.0.max(cells.2), cells.1.max(cells.3));
        let world = (x0 as f32 * cell_size, z0 as f32 * cell_size, (x1 + 1) as f32 * cell_size, (z1 + 1) as f32 * cell_size);
        let pieces: Vec<String> = map
            .placements
            .iter()
            .filter(|p| {
                let (x, z) = (p.at.0, p.at.1);
                x >= world.0 && x < world.2 && z >= world.1 && z < world.3
            })
            .map(|p| p.id.clone())
            .collect();
        let targets = if pieces.is_empty() {
            let mut t = Vec::new();
            for x in x0..=x1 {
                for z in z0..=z1 {
                    t.push(Target::Cell(x, z));
                }
            }
            t
        } else {
            pieces.iter().cloned().map(Target::Placement).collect()
        };
        let site = Site { cells: (x0, z0, x1, z1), world, pieces, margin: DEFAULT_MARGIN, camera };
        let id = self.add(targets, text)?;
        let note = self.get_mut(&id)?;
        note.state = State::Draft;
        note.site = Some(site);
        Ok(id)
    }

    /// Leaves a draft note on pieces a person Shift-clicked, recording the cells they cover.
    pub fn add_pieces(
        &mut self,
        map: &crate::map::Map,
        ids: &[String],
        cell_size: f32,
        camera: (f32, f32, f32, f32, f32),
        text: &str,
    ) -> Result<String, String> {
        let mut found = Vec::new();
        for id in ids {
            let p = map
                .placements
                .iter()
                .find(|p| &p.id == id)
                .ok_or_else(|| format!("notes: no placement {id}"))?;
            found.push(p);
        }
        if found.is_empty() {
            return Err("notes: a note needs at least one target".to_owned());
        }
        let cell = |v: f32| (v / cell_size).floor() as i32;
        let (mut x0, mut z0, mut x1, mut z1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for p in &found {
            let (x, z) = (cell(p.at.0), cell(p.at.1));
            (x0, z0, x1, z1) = (x0.min(x), z0.min(z), x1.max(x), z1.max(z));
        }
        let world = (x0 as f32 * cell_size, z0 as f32 * cell_size, (x1 + 1) as f32 * cell_size, (z1 + 1) as f32 * cell_size);
        let pieces: Vec<String> = ids.to_vec();
        let targets = pieces.iter().cloned().map(Target::Placement).collect();
        let id = self.add(targets, text)?;
        let note = self.get_mut(&id)?;
        note.state = State::Draft;
        note.site = Some(Site { cells: (x0, z0, x1, z1), world, pieces, margin: DEFAULT_MARGIN, camera });
        Ok(id)
    }

    /// Takes a note off the map. Its id stays spoken for only while other notes still hold a
    /// higher one, so removing the newest note frees its id.
    pub fn remove(&mut self, id: &str) -> Result<Note, String> {
        let at = self.notes.iter().position(|n| n.id == id).ok_or_else(|| format!("notes: no note {id}"))?;
        Ok(self.notes.remove(at))
    }

    /// Changes a draft or open note's text. A resolved note is a record and is not edited.
    pub fn edit(&mut self, id: &str, text: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            return Err("notes: a note needs some text".to_owned());
        }
        let note = self.get_mut(id)?;
        if !note.stands() {
            return Err(format!("notes: {id} is resolved; reopen it to edit"));
        }
        let old = std::mem::replace(&mut note.text, text.to_owned());
        note.log(What::Edited { old });
        Ok(())
    }

    /// Says whether the holder decides open choices itself instead of asking (E-070). Only on a
    /// note that still stands; a resolved note is a record.
    pub fn set_auto(&mut self, id: &str, auto: bool) -> Result<(), String> {
        let note = self.get_mut(id)?;
        if !note.stands() {
            return Err(format!("notes: {id} is resolved; reopen it to change it"));
        }
        note.auto = auto;
        Ok(())
    }

    /// Sends one draft: an agent may claim it from now on.
    pub fn send(&mut self, id: &str) -> Result<(), String> {
        let note = self.get_mut(id)?;
        if !note.is_draft() {
            return Err(format!("notes: {id} is not a draft, so it was not sent"));
        }
        note.state = State::Open;
        note.log(What::Sent);
        Ok(())
    }

    /// Sends every draft, in the order they were written, and returns their ids. No drafts is an
    /// error, so a button that does nothing says so.
    pub fn send_all(&mut self) -> Result<Vec<String>, String> {
        let ids: Vec<String> = self.notes.iter().filter(|n| n.is_draft()).map(|n| n.id.clone()).collect();
        if ids.is_empty() {
            return Err("notes: there are no drafts to send".to_owned());
        }
        for id in &ids {
            self.send(id)?;
        }
        Ok(ids)
    }

    /// Returns every note whose lease ran out by `now` to the queue, and says which.
    pub fn expire(&mut self, now: u64) -> Vec<String> {
        let mut gone = Vec::new();
        for n in &mut self.notes {
            if let State::Claimed { until, .. } = n.state
                && until <= now
            {
                n.state = State::Open;
                n.log_at(until, "lease", What::Expired);
                gone.push(n.id.clone());
            }
        }
        gone
    }

    /// An agent takes the lease on a sent note, for [`Notes::lease_secs`]; returns when it ends.
    /// Refused for a note that is not sent, one another agent holds (naming it), one whose reach
    /// overlaps a claimed note (until that resolves), and one whose reach overlaps a note sent
    /// earlier and still waiting (overlapping notes are worked in send order). Leases that ran out
    /// by `now` are given back first, so the clock is the caller's.
    pub fn claim(&mut self, id: &str, by: &str, now: u64) -> Result<u64, String> {
        if by.trim().is_empty() {
            return Err("notes: a claim names the agent making it".to_owned());
        }
        self.expire(now);
        let note = self.get(id).ok_or_else(|| format!("notes: no note {id}"))?;
        match &note.state {
            State::Claimed { by: holder, until } => {
                return Err(format!("notes: {id} is claimed by {holder} until {until}, so it was not claimed"));
            }
            State::Draft => return Err(format!("notes: {id} is a draft, not sent, so it was not claimed")),
            State::Resolved { .. } | State::Kept { .. } => {
                return Err(format!("notes: {id} is already resolved, so it was not claimed"));
            }
            State::Open => {}
        }
        if let Some(mine) = note.reach_rect() {
            let key = (note.sent_at(), self.notes.iter().position(|n| n.id == id));
            for (i, other) in self.notes.iter().enumerate() {
                if other.id == id || !other.reach_rect().is_some_and(|r| rects_overlap(mine, r)) {
                    continue;
                }
                match &other.state {
                    State::Claimed { by: holder, .. } => {
                        return Err(format!(
                            "notes: {id} overlaps {}, which {holder} is working, so it waits until that resolves",
                            other.id
                        ));
                    }
                    State::Open if (other.sent_at(), Some(i)) < key => {
                        return Err(format!(
                            "notes: {id} overlaps {}, which was sent first, so {} is worked first",
                            other.id, other.id
                        ));
                    }
                    _ => {}
                }
            }
        }
        let until = now + self.lease_secs;
        let note = self.get_mut(id)?;
        note.state = State::Claimed { by: by.to_owned(), until };
        note.log_at(now, by, What::Claimed { until });
        Ok(until)
    }

    /// The holder keeps working: the lease runs another [`Notes::lease_secs`] from `now`. A lease
    /// that already ran out is gone and cannot be renewed; claim again.
    pub fn renew(&mut self, id: &str, by: &str, now: u64) -> Result<u64, String> {
        self.expire(now);
        let lease = self.lease_secs;
        let note = self.get_mut(id)?;
        match &note.state {
            State::Claimed { by: holder, .. } if holder == by => {}
            State::Claimed { by: holder, .. } => {
                return Err(format!("notes: {id} is claimed by {holder}, not {by}, so it was not renewed"));
            }
            _ => return Err(format!("notes: {id} is not claimed by {by} (its lease may have expired), so it was not renewed")),
        }
        let until = now + lease;
        note.state = State::Claimed { by: by.to_owned(), until };
        note.log_at(now, by, What::Renewed { until });
        Ok(until)
    }

    /// Gives a claimed note back to the queue. `by` is the agent releasing it, who must hold it;
    /// `None` is the person, who may release any claim.
    pub fn release(&mut self, id: &str, by: Option<&str>, now: u64) -> Result<(), String> {
        self.expire(now);
        let note = self.get_mut(id)?;
        let State::Claimed { by: holder, .. } = &note.state else {
            return Err(format!("notes: {id} is not claimed, so there is nothing to release"));
        };
        if by.is_some_and(|b| b != holder) {
            return Err(format!("notes: {id} is claimed by {holder}, so only {holder} or the person may release it"));
        }
        note.state = State::Open;
        note.log_at(now, by.unwrap_or("person"), What::Released);
        Ok(())
    }

    /// Resolves an open note, naming the operations that did the work and what they changed.
    pub fn resolve(&mut self, id: &str, ops: Vec<String>, summary: &str) -> Result<(), String> {
        let note = self.get_mut(id)?;
        if !note.is_open() && note.holder().is_none() {
            return Err(format!("notes: {id} is already resolved"));
        }
        note.state = State::Resolved { ops, summary: summary.to_owned(), assumptions: Vec::new(), captures: Vec::new() };
        Ok(())
    }

    /// The holder asks the person a question and waits. Only the holder may ask, and one question
    /// stands at a time: it must be answered first.
    pub fn ask(&mut self, id: &str, by: &str, question: &str, now: u64) -> Result<(), String> {
        self.expire(now);
        if question.trim().is_empty() {
            return Err("notes: a question needs text".to_owned());
        }
        let note = self.get_mut(id)?;
        note.require_holder(id, by, "asked")?;
        if note.auto {
            return Err(format!(
                "notes: {id} is auto: the person asked you to decide open choices yourself, so do not ask. State what you assumed when you resolve it"
            ));
        }
        if note.pending_question().is_some() {
            return Err(format!("notes: {id} already has a question waiting for the person's answer"));
        }
        note.log_at(now, by, What::Asked { question: question.to_owned() });
        Ok(())
    }

    /// The person answers the question that stands on a note.
    pub fn answer(&mut self, id: &str, answer: &str, now: u64) -> Result<(), String> {
        if answer.trim().is_empty() {
            return Err("notes: an answer needs text".to_owned());
        }
        let note = self.get_mut(id)?;
        if note.pending_question().is_none() {
            return Err(format!("notes: {id} has no question waiting, so there is nothing to answer"));
        }
        note.log_at(now, "person", What::Answered { answer: answer.to_owned() });
        Ok(())
    }

    /// The holder finishes the note with the record the person reviews (#92): a one-line summary,
    /// the assumptions it made, the operation ids it ran, and the pictures of the area before and
    /// after. Refused with no summary, with fewer or more than two captures, or while a question
    /// is unanswered. The note becomes `done`, awaiting review.
    pub fn resolve_record(&mut self, id: &str, by: &str, record: Resolution, now: u64) -> Result<(), String> {
        self.expire(now);
        if record.summary.trim().is_empty() {
            return Err("notes: a resolve needs a one-line summary".to_owned());
        }
        if record.captures.len() != 2 || record.captures.iter().any(|c| c.trim().is_empty()) {
            return Err("notes: a resolve names two captures, the area before the work and after it".to_owned());
        }
        let note = self.get_mut(id)?;
        note.require_holder(id, by, "resolved")?;
        if note.pending_question().is_some() {
            return Err(format!("notes: {id} has a question the person has not answered, so it was not resolved"));
        }
        note.state = State::Resolved {
            ops: record.ops,
            summary: record.summary,
            assumptions: record.assumptions,
            captures: record.captures,
        };
        note.log_at(now, by, What::Resolved);
        Ok(())
    }

    /// Whether `change`, made by `by`, may go ahead under note `id`, and why not if it may not
    /// (#92). **The editor enforces this; it does not trust the agent.**
    ///
    /// - The agent must hold the note.
    /// - Ground outside the area plus its margin is refused, whatever was asked.
    /// - A change that must be asked about is refused until the person has answered a question the
    ///   holder asked since it claimed the note: more than [`LARGE_CHANGE`] pieces, any ground in
    ///   the margin, a piece a person placed removed (`person_placed`), or a piece the kit
    ///   cannot supply (`needs_unmapped`).
    pub fn fence(&self, id: &str, by: &str, change: &Change) -> Result<(), String> {
        let note = self.get(id).ok_or_else(|| format!("notes: no note {id}"))?;
        note.require_holder(id, by, "changed")?;
        let reach = note.reach_cells();
        if let (Some((x0, z0, x1, z1)), Some((rx0, rz0, rx1, rz1))) = (change.cells, reach)
            && (x0 < rx0 || z0 < rz0 || x1 > rx1 || z1 > rz1)
        {
            return Err(format!(
                "notes: the change reaches cells ({x0},{z0})..({x1},{z1}), outside note {id}'s area plus its margin ({rx0},{rz0})..({rx1},{rz1}); nothing was changed"
            ));
        }
        let mut why = Vec::new();
        if change.pieces > LARGE_CHANGE {
            why.push(format!("it changes {} pieces, more than {LARGE_CHANGE}", change.pieces));
        }
        if let (Some((x0, z0, x1, z1)), Some((ax0, az0, ax1, az1))) = (change.cells, note.area_cells())
            && (x0 < ax0 || z0 < az0 || x1 > ax1 || z1 > az1)
        {
            why.push("it touches the margin around the area".to_owned());
        }
        if !change.person_placed.is_empty() {
            why.push(format!("it removes {}, placed by a person", change.person_placed.join(", ")));
        }
        if let Some(what) = &change.needs_unmapped {
            why.push(format!("the kit has nothing for \"{what}\""));
        }
        if why.is_empty() || note.answered_since_claim() {
            return Ok(());
        }
        if note.auto {
            return Err(format!(
                "notes: {id} is auto, so it cannot ask, and {}; narrow the change or release the note for the person; nothing was changed",
                why.join("; ")
            ));
        }
        let waiting = if note.pending_question().is_some() { " (a question is waiting for the person)" } else { "" };
        Err(format!(
            "notes: {id}: {}, so ask the person first with note_ask and go on once they answer{waiting}; nothing was changed",
            why.join("; ")
        ))
    }

    /// Puts a resolved note back to open (the undo of a resolve).
    pub fn reopen(&mut self, id: &str) -> Result<(), String> {
        let note = self.get_mut(id)?;
        if note.stands() {
            return Err(format!("notes: {id} is already open"));
        }
        note.state = State::Open;
        Ok(())
    }

    /// Placement targets that no longer exist in the map, as `(note id, placement id)`. A note
    /// outlives the placement it marked (an agent may delete it while working the note), so this
    /// is a report, never a load error.
    pub fn dangling(&self, map: &crate::map::Map) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for n in &self.notes {
            for t in &n.targets {
                if let Target::Placement(p) = t
                    && !map.placements.iter().any(|x| &x.id == p)
                {
                    out.push((n.id.clone(), p.clone()));
                }
            }
        }
        out
    }

    /// The person keeps a done note's work: it is archived and its sign goes away (#93).
    pub fn keep(&mut self, id: &str, now: u64) -> Result<(), String> {
        let note = self.get_mut(id)?;
        let State::Resolved { ops, summary, assumptions, captures } = &note.state else {
            return Err(format!("notes: {id} is not done, so there is no work to keep"));
        };
        note.state = State::Kept {
            ops: ops.clone(),
            summary: summary.clone(),
            assumptions: assumptions.clone(),
            captures: captures.clone(),
        };
        note.log_at(now, "person", What::Kept);
        Ok(())
    }

    /// Cmd+Z after Keep: the note is done again.
    pub fn unkeep(&mut self, id: &str, now: u64) -> Result<(), String> {
        let note = self.get_mut(id)?;
        let State::Kept { ops, summary, assumptions, captures } = &note.state else {
            return Err(format!("notes: {id} is not kept"));
        };
        note.state = State::Resolved {
            ops: ops.clone(),
            summary: summary.clone(),
            assumptions: assumptions.clone(),
            captures: captures.clone(),
        };
        note.log_at(now, "person", What::Unkept);
        Ok(())
    }

    /// The person undoes a done note's work. `note_ops` is what its operations touched and `later`
    /// what the operations logged after them touched. Refused, naming the later operation, when one
    /// of them depends on the work ([`undo_blocker`]); otherwise the note is `sent` again (its sign
    /// back), with its whole history and a `WorkUndone` event. The editor reverts the operations as
    /// one undo step only when this accepts.
    pub fn undo_work(&mut self, id: &str, note_ops: &[Footprint], later: &[Footprint], now: u64) -> Result<(), String> {
        let note = self.get_mut(id)?;
        if !matches!(note.state, State::Resolved { .. }) {
            return Err(format!("notes: {id} is not done, so there is no work to undo"));
        }
        if let Some((op, what)) = undo_blocker(note_ops, later) {
            return Err(format!(
                "notes: {id}'s work was not undone: later operation {op} touched {what}, which the note's work made, so undoing would overwrite it. Undo {op} first"
            ));
        }
        note.state = State::Open;
        note.log_at(now, "person", What::WorkUndone);
        Ok(())
    }
}

/// Whether two map-space rectangles `[x0, z0, x1, z1]` share ground; touching edges do not.
fn rects_overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}
