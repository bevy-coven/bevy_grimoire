//! **The Tiles tab's page keys as registered operations** (E-002, #129 slice 36): `tile_page_enter`, `tile_page_leave`, `tile_open`,
//! `tile_close` and `tile_new`.
//!
//! The tab has two pages: the Tiles page (the kit's tiles, one row each, `+ New Tile` at the top, and the draft in hand) and the
//! Meshes page (where the library is picked from). `right` (`PageEnter`) opens the tile under the cursor and drills to the Meshes
//! page, `left` (`PageLeave`) comes back, `Enter` (`TileOpen`) opens the tile under the cursor (or raises the name prompt on
//! `+ New Tile`), `C` (`CloseTile`) puts the tile in hand down and `N` (`BuildNew`) names a new blank tile. An agent calls the same
//! operations through `grimoire/operation_apply`, and names the tile (`tile_open`'s `id`, `tile_new`'s `name`) where a key has a
//! cursor and a prompt.
//!
//! Each reads where the page is and the tile in hand off the view it is carried ([`crate::ops_view::View::tile_page`] and
//! `tile_hand`) and answers with a [`crate::ops_view::PageAnswer`], which the front end carries out in
//! [`crate::build::Build::take_page`]. The draft is session state until `Cmd+S` saves it (`composition_define`): nothing here writes
//! a file, and a different tile opened is a different document, so the tab's history starts afresh (`Build::opened`).

use crate::build::{NEW_TILE_ROW, blank, claims_no_tile, has_a_box, row_composition};
use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};
use crate::ops_view::{PageAnswer, TileAnswer, TileHand};
use crate::project::Project;
use bevy_grimoire_core::composition::Composition;
use serde::Deserialize;
use serde_json::json;

/// **What a close says when no tile is open**: guidance on the key, a refusal for an agent.
pub const NOTHING_TO_CLOSE: &str = "nothing open to close — the kit's tiles are on this page";

/// **What a new tile's prompt says**, on `N`, on `Enter` on `+ New Tile` and for an agent that names no tile.
pub const NAME_THE_TILE: &str = "name the tile — Enter opens it, Esc leaves things as they are";

/// **The page and the tile in hand, as the operations read them.**
struct Tab<'a> {
    kit: &'a [Composition],
    project: &'a Project,
    /// The Tiles page's cursor row, or `None` on the Meshes page.
    browsing: Option<usize>,
    hand: Option<&'a TileHand>,
}

impl<'a> Tab<'a> {
    fn of(ctx: &'a Ctx) -> Result<Tab<'a>, Refusal> {
        let Some(view) = ctx.view.as_ref() else {
            return Err(Refusal::new(
                "no view is in hand, so there is no Tiles page to work on. An editor front end carries one in `Ctx::view`.",
            ));
        };
        let Some(project) = ctx.project.as_ref() else {
            return Err(Refusal::new("no project is open, so there are no tiles."));
        };
        Ok(Tab { kit: &project.compositions.compositions, project, browsing: view.tile_page.map(|p| p.row), hand: view.tile_hand.as_ref() })
    }

    /// **The tile in hand that is not yet in the kit** — the draft.
    fn draft(&self) -> Option<&'a Composition> {
        self.hand.map(|h| &h.open).filter(|c| !self.kit.iter().any(|s| s.id == c.id))
    }

    /// **How many rows the page draws**: `+ New Tile`, the kit's tiles and the draft.
    fn rows(&self) -> usize {
        self.kit.len() + usize::from(self.draft().is_some()) + 1
    }

    /// The tile a page row names: a kit tile, or the draft after them.
    fn row_tile(&self, row: usize) -> Option<&'a Composition> {
        row_composition(row).and_then(|i| self.kit.get(i)).or_else(|| self.draft())
    }

    /// **Whether this tile is the one in hand.**
    fn in_hand(&self, tile: &Composition) -> bool {
        self.hand.is_some_and(|h| h.open.id == tile.id)
    }

    /// **Open a tile, or refuse it by name**: a tile already in hand is not reopened (that would throw away its unsaved edits and
    /// both undo stacks), and a composition with no box is not a tile this tab can edit.
    fn open(&self, tile: &Composition) -> Result<(PageAnswer, String), Refusal> {
        if self.in_hand(tile) {
            let said = format!("`{}` is already open — {} member(s)", tile.id, tile.members.len());
            return Ok((PageAnswer { made: true, browsing: None, ..Default::default() }, said));
        }
        if !has_a_box(tile) {
            return Err(Refusal::new(claims_no_tile(&tile.id)));
        }
        let said = format!("`{}` opened — {} member(s), Cmd+S saves over it", tile.id, tile.members.len());
        let answer = PageAnswer { made: true, browsing: None, tile: TileAnswer::Open { tile: tile.clone(), placing: true }, name: false };
        Ok((answer, said))
    }
}

/// **An operation that takes no fields**, or the refusal that says so.
fn no_fields(params: &serde_json::Value, what: &str) -> Result<(), Refusal> {
    match params {
        serde_json::Value::Null => Ok(()),
        serde_json::Value::Object(o) if o.is_empty() => Ok(()),
        _ => Err(Refusal::new(format!("{what} takes no fields: it acts on the page the Tiles tab is on."))),
    }
}

/// **The chord that opens a tile**, read from the census so a sentence naming it cannot become a lie the day a key moves.
fn open_chord() -> String {
    crate::keys::chord(crate::keys::Action::TileOpen)
}

// ── tile_page_enter ──────────────────────────────────────────────────────────────────────────────

/// **Open the tile under the Tiles page's cursor and drill to the Meshes page**, as `right` does.
pub struct TilePageEnter;

impl TilePageEnter {
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<(PageAnswer, String), Refusal> {
        no_fields(params, "entering a tile")?;
        let tab = Tab::of(ctx)?;
        let Some(row) = tab.browsing else {
            return Err(Refusal::new("the Tiles page is not up, so there is no row to open: `tile_page_leave` goes back to it."));
        };
        // **Row zero drills and says so.** The drill needs no tile; `Enter` (`tile_open`) is what acts on `+ New Tile`.
        if row == NEW_TILE_ROW {
            let said = if tab.rows() > 1 {
                format!("meshes — nothing in hand; {} on a tile row opens one", open_chord())
            } else {
                format!("no tiles in the kit yet — {} on + New Tile names one", open_chord())
            };
            return Ok((PageAnswer { made: true, browsing: None, ..Default::default() }, said));
        }
        let Some(tile) = tab.row_tile(row) else {
            return Err(Refusal::new(format!("no tile at row {row}; the page has {}", tab.rows())));
        };
        tab.open(tile)
    }
}

impl Operation for TilePageEnter {
    fn name(&self) -> &'static str {
        "tile_page_enter"
    }
    fn changes_view(&self) -> bool {
        true
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({"type": "object", "additionalProperties": false, "properties": {}})
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        Ok(json!({ "says": said, "opens": matches!(answer.tile, TileAnswer::Open { .. }) }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        let opens = matches!(answer.tile, TileAnswer::Open { .. });
        if let Some(view) = ctx.view.as_mut() {
            view.page = answer;
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "opens": opens })) })
    }
}

// ── tile_page_leave ──────────────────────────────────────────────────────────────────────────────

/// **Go back to the Tiles page from the Meshes page**, as `left` does (and `Esc`, on the Meshes page).
pub struct TilePageLeave;

impl TilePageLeave {
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<(PageAnswer, String), Refusal> {
        no_fields(params, "leaving the Meshes page")?;
        let tab = Tab::of(ctx)?;
        if tab.browsing.is_some() {
            return Err(Refusal::new("the Tiles page is already up: `tile_page_enter` drills to the Meshes page."));
        }
        // **A note, not a problem, when nothing is authored yet**: the answer is to make one, and `+ New Tile` is where that starts.
        // `rows` counts `+ New Tile`, so a page of exactly one is an empty kit; otherwise the cursor lands on the first real tile.
        let (row, said) = if tab.rows() <= 1 {
            (NEW_TILE_ROW, "no tiles in the kit yet — New Tile +, or build and press Cmd+S".to_owned())
        } else {
            (1, "back on the tiles".to_owned())
        };
        Ok((PageAnswer { made: true, browsing: Some(row), ..Default::default() }, said))
    }
}

impl Operation for TilePageLeave {
    fn name(&self) -> &'static str {
        "tile_page_leave"
    }
    fn changes_view(&self) -> bool {
        true
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({"type": "object", "additionalProperties": false, "properties": {}})
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        Ok(json!({ "says": said, "row": answer.browsing }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        let row = answer.browsing;
        if let Some(view) = ctx.view.as_mut() {
            view.page = answer;
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "row": row })) })
    }
}

// ── tile_open ────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenParams {
    /// A tile's id, as the Tiles page lists it; absent is the row under the cursor.
    #[serde(default)]
    id: Option<String>,
}

fn open_params(raw: &serde_json::Value) -> Result<OpenParams, Refusal> {
    if raw.is_null() {
        return Ok(OpenParams::default());
    }
    serde_json::from_value(raw.clone())
        .map_err(|e| Refusal::new(format!("this is not a tile to open: {e}. The only field is `id`, a tile's id as the Tiles page lists it.")))
}

/// **Open a tile for editing**, as `Enter` does on the Tiles page: the row under the cursor, or the tile `id` names. On `+ New
/// Tile` it raises the name prompt instead.
pub struct TileOpen;

impl TileOpen {
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<(PageAnswer, String), Refusal> {
        let id = open_params(params)?.id;
        let tab = Tab::of(ctx)?;
        let row = match &id {
            Some(id) => match tab.kit.iter().position(|c| &c.id == id) {
                Some(at) => at + 1,
                None if tab.draft().is_some_and(|d| &d.id == id) => tab.kit.len() + 1,
                None => {
                    return Err(Refusal::new(format!("no tile is called `{id}`: name one the Tiles page lists, or `tile_new` makes one.")));
                }
            },
            None => tab.browsing.ok_or_else(|| {
                Refusal::new("the Tiles page is not up, so there is no row to open: name the tile with `id`, or `tile_page_leave` goes back to the page.")
            })?,
        };
        // **Row zero makes one**, the same door `right` and the row's click use.
        if row == NEW_TILE_ROW {
            return Ok((PageAnswer { made: true, browsing: Some(NEW_TILE_ROW), name: true, ..Default::default() }, NAME_THE_TILE.to_owned()));
        }
        let Some(tile) = tab.row_tile(row) else {
            return Err(Refusal::new(format!("no tile at row {row}; the page has {}", tab.rows())));
        };
        tab.open(tile)
    }
}

impl Operation for TileOpen {
    fn name(&self) -> &'static str {
        "tile_open"
    }
    fn changes_view(&self) -> bool {
        true
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": {"type": "string", "description": "A tile's id as the Tiles page lists it. Absent opens the row under the page's cursor, and on `+ New Tile` raises the name prompt."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        Ok(json!({ "says": said, "opens": matches!(answer.tile, TileAnswer::Open { .. }), "asks_name": answer.name }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        let (opens, asks_name) = (matches!(answer.tile, TileAnswer::Open { .. }), answer.name);
        if let Some(view) = ctx.view.as_mut() {
            view.page = answer;
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "opens": opens, "asks_name": asks_name })) })
    }
}

// ── tile_close ───────────────────────────────────────────────────────────────────────────────────

/// **Put the tile in hand down**, as `C` does, and go back to the Tiles page.
pub struct TileClose;

impl TileClose {
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<(PageAnswer, String), Refusal> {
        no_fields(params, "closing the tile")?;
        let tab = Tab::of(ctx)?;
        let Some(held) = tab.hand else {
            return Err(Refusal::new(NOTHING_TO_CLOSE));
        };
        // **Back to the page, the cursor where it was and clamped to what is left.** Closing a draft takes its row off the page with
        // it, so a cursor on the last row would be left pointing past the end: clamped to the committed count, which is the last
        // valid row because the page is that plus the draft plus `+ New Tile` at index 0.
        let row = tab.browsing.unwrap_or(NEW_TILE_ROW).min(tab.kit.len());
        let said = format!(
            "`{}` closed — {} member(s) in it, {} on its row opens it again",
            held.open.id,
            held.open.members.len(),
            open_chord()
        );
        Ok((PageAnswer { made: true, browsing: Some(row), tile: TileAnswer::Close, name: false }, said))
    }
}

impl Operation for TileClose {
    fn name(&self) -> &'static str {
        "tile_close"
    }
    fn changes_view(&self) -> bool {
        true
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({"type": "object", "additionalProperties": false, "properties": {}})
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        Ok(json!({ "says": said, "row": answer.browsing }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        let row = answer.browsing;
        if let Some(view) = ctx.view.as_mut() {
            view.page = answer;
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "row": row })) })
    }
}

// ── tile_new ─────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct NewParams {
    /// The new tile's name, snake_cased into the kit's namespace; absent raises the name prompt for the person to type one.
    #[serde(default)]
    name: Option<String>,
}

fn new_params(raw: &serde_json::Value) -> Result<NewParams, Refusal> {
    if raw.is_null() {
        return Ok(NewParams::default());
    }
    serde_json::from_value(raw.clone())
        .map_err(|e| Refusal::new(format!("this is not a tile to make: {e}. The only field is `name`, and it is optional.")))
}

/// **A new blank tile, named before it exists**, as `N` does: the name raises a prompt the person types into, or an agent names it.
pub struct TileNew;

impl TileNew {
    fn decide(ctx: &Ctx, params: &serde_json::Value) -> Result<(PageAnswer, String), Refusal> {
        let name = new_params(params)?.name;
        let tab = Tab::of(ctx)?;
        let Some(raw) = name else {
            return Ok((PageAnswer { made: true, browsing: tab.browsing, name: true, ..Default::default() }, NAME_THE_TILE.to_owned()));
        };
        let name = bevy_grimoire_core::naming::to_snake_case(&raw);
        if name.is_empty() {
            return Err(Refusal::new("a tile needs a name — type one, or Esc to leave it"));
        }
        let id = format!("{}/{}", tab.project.namespace, name);
        // Refused by name rather than silently replacing somebody's tile.
        if tab.kit.iter().any(|c| c.id == id) {
            return Err(Refusal::new(format!("`{id}` is already in the kit — pick another name")));
        }
        let tile = blank(&id, tab.project.lattice.cell_height);
        // A freshly named tile lands on the Meshes page like a reopened one, but nothing is picked up: there is nothing in it to steer.
        let answer = PageAnswer { made: true, browsing: None, tile: TileAnswer::Open { tile, placing: false }, name: false };
        Ok((answer, format!("building `{id}`")))
    }
}

impl Operation for TileNew {
    fn name(&self) -> &'static str {
        "tile_new"
    }
    fn changes_view(&self) -> bool {
        true
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "name": {"type": "string", "description": "The new tile's name, snake_cased into the kit's namespace. Absent raises the name prompt on the editor's screen for the person to type one."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        Ok(json!({ "says": said, "asks_name": answer.name }))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (answer, said) = Self::decide(ctx, params)?;
        let asks_name = answer.name;
        if let Some(view) = ctx.view.as_mut() {
            view.page = answer;
        }
        Ok(Outcome { summary: said, affected: 0, saved: false, inverse: None, after: Some(json!({ "asks_name": asks_name })) })
    }
}
