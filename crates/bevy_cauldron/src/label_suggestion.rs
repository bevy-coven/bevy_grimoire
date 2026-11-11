//! **What a labelling request carries and what it returns**, whichever endpoint answers it: the
//! context a request is built from ([`PromptCtx`]), the proposal that comes back ([`Suggestion`]), who
//! said it ([`Provenance`]), and why a request produced none ([`LabelFailure`]). The one labelling path,
//! [`crate::decide_labels`], reads and writes these.
//!
//! The rules the labeller works under, from `docs/llm_rule_authoring.md`:
//!
//! - **The model authors metadata dev-time only; it never runs in the sim.** This crate is the
//!   editor, never shipped.
//! - **Closed vocabulary only.** A suggestion can carry only tokens the project's vocabulary has:
//!   the questions are built from it. `library.resolve` inside the commit door stays the final gate
//!   on the exact bytes written.
//! - **Nothing here writes a descriptor; provenance records who decided.** The output is a
//!   [`Suggestion`]; `labels` applies each accepted axis on arrival through the commit door,
//!   stamped `By::Model`, so a person's own axes stay theirs and the model's can be told apart
//!   and reverted.
//! - **One path.** One configured endpoint per run, no fallback chain; an endpoint that cannot be
//!   reached is a loud refusal.
//!
//! No Bevy types in this module, so it is unit-testable in the GPU-free gate.

use bevy_grimoire_core::descriptor::{Confidence, DecalHost, Face, Mount};

/// The `.env` file at the project root, parsed — `KEY=VALUE` lines, `#` comments, an optional
/// `export ` prefix and single/double quotes tolerated. Hand-rolled: twenty lines of parsing is
/// not worth a dependency, and this is a dev tool's config, not a spec implementation.
pub(crate) fn dotenv(root: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(root.join(".env")) else {
        return out;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        out.insert(key.trim().to_owned(), value.to_owned());
    }
    out
}

/// A vocabulary addition the model wanted and could not have — the only legal exit for an
/// out-of-vocab idea. Flagged for a human; never enters the axes.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TokenProposal {
    pub axis: String,
    pub token: String,
    pub why: String,
}

/// The model's judgement that an asset is authored lying down, and which quarter turn would
/// stand it up. **Flagged, never auto-applied**: the turn itself (`tiles::rotate_mesh`) is a
/// measurement operation — it re-measures extents and rewrites the lattice off the GLB — and
/// measurements are the importer's, triggered deliberately by a human. The VLM supplies only the
/// judgement that one is needed.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NeedsTurn {
    /// `"x"` or `"z"` — the horizontal axis of the righting turn.
    pub axis: String,
    /// **How many 90-degree quarter turns about that axis**, 1 to 3.
    ///
    /// It was one, implied, and that is only enough for a piece lying on its side. An asset
    /// authored upside down needs two, and the only way to say so was to right it once, re-shoot
    /// it, and let the model ask again — a second photograph and a second inference, about
    /// twenty-five seconds, to express a number the model already knew. Asked for at the keyboard,
    /// 2026-08-18: *"if the mesh is upside down, it can detect that, and send back a command to
    /// rotate it so many times (snapped to grid)."*
    ///
    /// **Quarter turns, because that is what the descriptor can hold**: `align.rotate` is three
    /// integer degrees stepped by `RotateAxis::bumped`, so 90 is the smallest expressible turn and
    /// 4 is where it started. 0 is spelled `needs_turn: null`.
    ///
    /// **The direction is not asked for**, deliberately. A turn is always in the `+` sense, so a
    /// piece needing a quarter turn the other way is `3` — and telling a model which visual result
    /// `+90` produces would mean stating a handedness convention this prompt cannot verify. `2` is
    /// the case that has no such ambiguity, and it is the one that was asked for. An odd turn taken
    /// the wrong way is corrected by the re-photograph the righting already performs.
    pub turns: u8,
    pub why: String,
}

/// What the model proposed, already validated against the live vocabulary — every token in these
/// lists exists, in vocabulary order, deduplicated.
///
/// Serde is derived and currently unread: the reason for it was the suggestion cache under
/// `target/` that persisted these between sessions, and that cache is gone — see `labels`' module
/// note for why a proposal no longer outlives the frame it lands in.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Suggestion {
    /// The model's one-sentence identification — the reasoning the axis answers hang off, and the
    /// human-review headline.
    pub what: String,
    pub kind: Vec<String>,
    pub effects: Vec<String>,
    pub look: Vec<String>,
    pub offers_surfaces: Vec<String>,
    pub mount: Option<Mount>,
    /// The item's visual front — which face it should present to the room. A judgement only
    /// appearance can answer, which is why the importer defaults it and the model proposes it.
    ///
    /// **A proposal, and it loses to a measurement.** `Glb::derive_front` reads the vertex buffer,
    /// and symmetry is a property of the buffer that two three-quarter renders cannot settle, so
    /// `labels::apply_fields` fills an unmeasured front from this and refuses to overwrite a
    /// measured one.
    pub front: Option<Face>,
    pub needs_turn: Option<NeedsTurn>,
    pub note: Option<String>,
    pub rooms: Vec<String>,
    pub group: Option<String>,
    pub confidence: Confidence,
    pub token_proposals: Vec<TokenProposal>,
}

impl Suggestion {
    /// **The words this proposal offers as the piece's description**, and the only place that
    /// decides which they are: the `note` when it holds anything, else [`Suggestion::what`].
    ///
    /// Stated once so [`crate::labels::apply_fields`] writes exactly the words the panel showed. Two
    /// readers, one fact: a panel proposing one sentence and an apply that wrote another is the
    /// class of defect the whole "one value, never two" pass exists to end.
    pub fn description(&self) -> Option<&str> {
        [self.note.as_deref(), Some(self.what.as_str())]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|s| !s.is_empty())
    }
}

/// Who said so, when, in how many attempts — the review header's facts. Never the key, never the
/// endpoint host.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    /// **The model that ANSWERED**, off the response's own `model` field — never the name the request
    /// asked for: a server serves what it serves, and this is the field a person reads to know whose
    /// judgement is in the library.
    pub model: String,
    /// `YYYY-MM-DD`, supplied by the caller (this module owns no clock).
    pub date: String,
    /// How many requests the answer took: always 1, since a decision is read, never reprompted.
    pub attempts: u8,
}

/// Everything the prompt needs about the subject, gathered by the caller — no `&Project` in here.
pub struct PromptCtx {
    pub id: String,
    pub mesh: String,
    /// Measured footprint (w, d) and height, metres — given to the model so size disambiguates a
    /// pan from a pot; never the model's to infer.
    pub footprint: Option<(f32, f32)>,
    pub height: Option<f32>,
    pub mount_now: String,
    pub kind_now: Vec<String>,
    pub effects_now: Vec<String>,
    pub look_now: Vec<String>,
    pub offers_now: Vec<String>,
    pub note_now: Option<String>,
    /// Room / group names already in use across the library, so the model converges on the
    /// existing vocabulary of free text instead of coining synonyms.
    pub rooms_in_use: Vec<String>,
    pub groups_in_use: Vec<String>,
    /// **What the MESH says about its own front**, measured off the vertices by
    /// `bevy_grimoire_core::glb::Glb::derive_front` — not something the model is asked to see.
    ///
    /// `Some(Some(face))` measured and asymmetric; `Some(None)` measured **symmetric**, so there is
    /// no front to claim; `None` could not be measured at all (no mesh, unreadable file).
    ///
    /// This exists because the model got it wrong three times in one pass, on facts the code already
    /// held: a front for a 1x1 floor tile, and a front for two seats that
    /// `the_seat_meshes_declare_the_front_that_was_measured_off_them` asserts are symmetric and of
    /// which *"none may be asserted"*. Two renders cannot settle symmetry; a vertex buffer can. The
    /// prompt already takes this line about size — *"Sizes are measured and given to you"* — and
    /// this is the same rule applied to the same class of question.
    pub front_measured: Option<Option<Face>>,
}

/// **What each mount MEANS**, in the model's terms rather than the editor's.
///
/// Short labels carry no meaning on their own, and the wrong one was being chosen for a reason that
/// is obvious in hindsight: **every asset is photographed standing on a neutral floor**, so "on
/// floor" describes the picture of nearly everything. Reported from the keyboard, 2026-08-15: *"we
/// have the labels on floor and on top of the floor, but that seems to confuse the VLM because most
/// things are on top of the floor."*
///
/// So each line says what the mount is FOR — whether the piece needs a host, and which — rather
/// than where it happens to be resting in the render. The question the model has to answer is about
/// the object's nature in a real room, not about the photograph.
pub(crate) fn mount_meaning(m: &Mount) -> &'static str {
    match m {
        Mount::OnFloor => {
            "stands on the ground by itself in a real room - furniture, crates, \
                           machines, appliances. Do NOT choose this merely because the photo shows \
                           it on a floor; everything is photographed that way"
        }
        Mount::OnSurface { .. } => {
            "too small or unstable to stand on the ground - it belongs ON \
                                    TOP OF another piece of furniture (a mug, a lamp, a keyboard, \
                                    a book, a tray)"
        }
        Mount::OnSurfaceOrFloor { .. } => {
            "small enough to sit on a table but also at home directly on the floor \
                                          - a set of tools, a pile of ingots, a bundle of rods, a \
                                          stack of sheets. Choose this only when BOTH readings are \
                                          natural; a mug is `surface`"
        }
        Mount::OnFace { .. } => {
            "fixed flat against the vertical face of another piece - not \
                                 resting on anything"
        }
        Mount::OnWall { .. } => {
            "fixed to a room wall at a height, carrying no weight below it - a \
                                 sconce, a sign, a switch, a screen"
        }
        Mount::OnCeiling => "hangs from above - a pendant lamp, a duct, a fan",
        Mount::Tiled => {
            "a repeating surface piece that covers ground or wall in a grid - floor \
                         panels, wall panels"
        }
        Mount::InOpening { .. } => "fills a hole cut in a wall - a door leaf, a window, a grille",
        Mount::Decal {
            on: DecalHost::Floor,
        } => {
            "a flat marking painted ON the floor with no \
                                                  thickness - a line, an arrow, a stain"
        }
        Mount::Decal {
            on: DecalHost::Wall { .. },
        } => {
            "a flat marking painted ON a wall with no \
                                                       thickness - a sign, a stencil, a poster"
        }
        Mount::Decal {
            on: DecalHost::Ceiling,
        } => {
            "a flat marking painted ON the ceiling with no \
                                                    thickness"
        }
    }
}

/// **The one place a `Face` gets its wire name**: the option names of the `front` question and the
/// words a measured front is stated in.
pub fn face_token(f: Face) -> &'static str {
    match f {
        Face::North => "north",
        Face::East => "east",
        Face::South => "south",
        Face::West => "west",
    }
}

/// **Why a labelling request produced no suggestion** — a type, because the one thing a caller
/// branches on is a fact about the failure, and it must not be recovered by searching a sentence.
///
/// `labels::poll_tasks` asks [`LabelFailure::is_unreachable`] to decide whether to mark the link down
/// and stop a long walk rather than burn it: the transport being down is the whole batch's news, and
/// reporting it once per queued mesh buries the one line that says what to do. The sentence is
/// written exactly once, in the [`std::fmt::Display`] impl, and the question is asked of the value.
#[derive(Debug, Clone, PartialEq)]
pub enum LabelFailure {
    /// Nothing is listening. Carries the transport's own words plus the remedy for this endpoint.
    Unreachable {
        transport: String,
        remedy: Option<String>,
    },
    /// **Everything else, in the words the author has to read**: a timeout, a non-2xx body, a
    /// response naming no model, an unreadable shot.
    /// One variant and not five, because nothing branches on the difference — the day something
    /// does is the day to split it.
    Refused(String),
}

impl LabelFailure {
    /// **Is this the failure whose remedy is "bring the endpoint up"?**
    ///
    /// The one question a caller asks of a failure, and the reason this is a type rather than a
    /// sentence: it decides whether the status band says the link is down and whether a running
    /// batch stops instead of reporting the same fault once per queued mesh.
    pub fn is_unreachable(&self) -> bool {
        matches!(self, LabelFailure::Unreachable { .. })
    }
}

impl std::fmt::Display for LabelFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelFailure::Unreachable {
                transport,
                remedy: Some(remedy),
            } => write!(f, "the VLM endpoint is unreachable ({transport}) — {remedy}"),
            LabelFailure::Unreachable {
                transport,
                remedy: None,
            } => write!(f, "the VLM endpoint is unreachable: {transport}"),
            LabelFailure::Refused(text) => f.write_str(text),
        }
    }
}

/// **A plain-string complaint from outside the transport is a refusal, and never the link being
/// down.** `labels::spawn_request` mixes `encode_png`'s and the shots' `String` errors in with this
/// type at one boundary; converting them here is what keeps any of them from being read
/// as an unreachable endpoint, which is precisely what a substring match over the joined text
/// allowed.
impl From<String> for LabelFailure {
    fn from(text: String) -> LabelFailure {
        LabelFailure::Refused(text)
    }
}
