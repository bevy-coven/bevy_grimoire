//! **One command registry.** Every [`Action`] is a command with a stable id, a title, a category
//! and a default shortcut (UI-04, issue #212; `docs/ui/vision.md` §2 and §7).
//!
//! # Why the registry wraps the census instead of replacing it
//!
//! [`keys::BINDINGS`] is the one table that says which key does what, in which context and in which
//! stance, and a dozen tests police it. A second table of shortcuts would be the second census this
//! crate refuses, so a [`Command`] *reads* its shortcut from the binding row and adds only what the
//! row does not carry: an id that does not change when the variant is renamed or the key moves, and a
//! title that is one thing where the binding's `does` text is the shared caption of a whole group of
//! keys ("turn L / turn R / tip x / tip z / straight").
//!
//! The menus, the command palette, the toolbar, context menus and MCP all dispatch by id through
//! here, so a verb appears once and every surface shows the same words and the same chord.
//!
//! # The id is `<category>.<verb>`
//!
//! The category is the part before the dot and is what a menu and the palette group by. [`identity`]
//! is an exhaustive `match`: adding an [`Action`] without naming it does not compile, and
//! `tests/every_action_is_a_command.rs` checks that no two ids and no two titles are alike.

use bevy_gooey::model::shortcut_conflicts;
use bevy_gooey::{Modifiers, Shortcut};

use crate::keys::{self, Action, Binding, Context, Live, Stance, Stances};

/// One verb the editor can be asked to do, as a menu, the palette or an agent names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// The action this command dispatches; the keys, the menus and the palette all end here.
    pub action: Action,
    /// `<category>.<verb>`, stable: a saved keymap, a script and an MCP call may name it.
    pub id: &'static str,
    /// What a menu or the palette shows, in sentence case.
    pub title: &'static str,
    /// The default shortcut, as gooey writes it: `Ctrl+S` here and `⌘S` on a mac.
    pub shortcut: Shortcut,
    /// Where the binding is live. `Global` is every tab.
    pub context: Context,
    /// What the binding asks of the stance (what the author has in hand).
    pub stances: Stances,
}

impl Command {
    /// The part of the id before the dot: what a menu groups this command under.
    pub fn category(&self) -> &'static str {
        self.id.split_once('.').map_or(self.id, |(category, _)| category)
    }

    /// The shortcut as a person reads it on this platform.
    pub fn shortcut_label(&self) -> String {
        self.shortcut.label()
    }

    /// **Can it run now, and if not, why not** — a menu greys the item and a tooltip says the reason.
    ///
    /// Judged by the rules the dispatcher uses ([`keys::fires_in`], [`Stances::allows`]), so a menu
    /// never offers what the key would refuse.
    pub fn enabled(&self, live: Live) -> Result<(), &'static str> {
        if live.0 == Context::Typing {
            return Err("A text field is taking the keys");
        }
        match live.2 {
            keys::Holder::Tab => {}
            keys::Holder::Filter => return Err("A filter box is taking the keys"),
            keys::Holder::Journal => return Err("The journal is taking the keys"),
        }
        if !keys::fires_in(self.context, live.0) {
            return Err("Not on this tab");
        }
        if !self.stances.allows(live.1) {
            return Err(match self.stances {
                Stances::Only(Stance::Holding) => "Only while holding a piece",
                Stances::Only(Stance::Proposed) => "Only while a layout is proposed",
                Stances::Only(Stance::Browsing) => "Only while browsing the library",
                Stances::Only(Stance::Picking) => "Only while picking a composition",
                Stances::Only(Stance::Idle) => "Only with nothing in hand",
                Stances::AnyBut(_) | Stances::Any => "Not in this state",
            });
        }
        Ok(())
    }
}

/// The command for one action.
pub fn command(action: Action) -> Command {
    let binding = keys::binding(action);
    let (id, title) = identity(action);
    Command {
        action,
        id,
        title,
        shortcut: shortcut_of(binding),
        context: binding.context,
        stances: binding.needs_stance,
    }
}

/// Every command, in the order the actions are declared.
pub fn all() -> Vec<Command> {
    Action::ALL.into_iter().map(command).collect()
}

/// The command with this id, if there is one.
pub fn by_id(id: &str) -> Option<Command> {
    Action::ALL
        .into_iter()
        .find(|a| identity(*a).0 == id)
        .map(command)
}

/// A binding's default shortcut. Shift is part of it only where the row requires it; the platform's
/// own modifier is gooey's `PRIMARY`, so `Cmd+S` on a mac is `Ctrl+S` elsewhere, as [`keys::MOD_KEYS`]
/// already says.
pub fn shortcut_of(binding: &Binding) -> Shortcut {
    let mut modifiers = Modifiers::NONE;
    if binding.needs_mod {
        modifiers = modifiers | Modifiers::PRIMARY;
    }
    if binding.needs_shift == Some(true) {
        modifiers = modifiers | Modifiers::SHIFT;
    }
    Shortcut::new(modifiers, binding.key)
}

/// Every chord a binding answers to: a row that is indifferent to Shift fires with it held too.
fn chords(binding: &Binding) -> Vec<Shortcut> {
    let plain = shortcut_of(binding);
    let shifted = Shortcut::new(plain.modifiers | Modifiers::SHIFT, plain.key);
    match binding.needs_shift {
        Some(true) => vec![shifted],
        Some(false) => vec![plain],
        None => vec![plain, shifted],
    }
}

/// Two commands that would both fire on one chord in one live state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clash {
    pub first: &'static str,
    pub second: &'static str,
    /// The tab on which they are both live.
    pub tab: Context,
    /// The stance in which they are both live.
    pub stance: Stance,
}

/// The tabs a person can be on: every context but `Global` (the frame around them) and `Typing`
/// (which suppresses everything).
fn tabs() -> impl Iterator<Item = Context> {
    Context::ALL
        .into_iter()
        .filter(|c| !matches!(c, Context::Global | Context::Typing))
}

/// The commands live on `tab` while the author holds `stance`, each with the chords it answers to.
pub fn keymap(tab: Context, stance: Stance) -> Vec<(&'static str, Shortcut)> {
    keys::BINDINGS
        .iter()
        .filter(|b| keys::fires_in(b.context, tab) && b.needs_stance.allows(stance))
        .flat_map(|b| {
            let id = identity(b.action).0;
            chords(b).into_iter().map(move |chord| (id, chord))
        })
        .collect()
}

/// **Every pair of commands that share a chord in a state where both are live.** Empty is the rule:
/// gooey's [`shortcut_conflicts`] decides what a clash is, over the commands live on each tab in
/// each stance, because a key is allowed to mean one thing on the Map and another on the Anim bench.
pub fn conflicts() -> Vec<Clash> {
    let mut found: Vec<Clash> = Vec::new();
    for tab in tabs() {
        for stance in Stance::ALL {
            let map = keymap(tab, stance);
            for (first, second) in shortcut_conflicts(&map) {
                // A row indifferent to Shift is listed twice, so one pair can be reported twice.
                if first == second {
                    continue;
                }
                let first = id_named(&first);
                let second = id_named(&second);
                let clash = Clash { first, second, tab, stance };
                if !found.contains(&clash) {
                    found.push(clash);
                }
            }
        }
    }
    found
}

fn id_named(id: &str) -> &'static str {
    Action::ALL
        .into_iter()
        .map(|a| identity(a).0)
        .find(|candidate| *candidate == id)
        .unwrap_or("?")
}

/// **The stable id and the title of every action.** Exhaustive on purpose.
///
/// Titles are verb-first sentence case and say what the key does in this action's own words, not the
/// group's shared caption: `Undo a mesh edit` and `Undo a rig edit` are two commands with two
/// histories behind them.
pub fn identity(action: Action) -> (&'static str, &'static str) {
    match action {
        // ── The frame around every tab ───────────────────────────────────────
        Action::TabSlot1 => ("view.panel_1", "Open panel 1 of this door"),
        Action::TabSlot2 => ("view.panel_2", "Open panel 2 of this door"),
        Action::TabSlot3 => ("view.panel_3", "Open panel 3 of this door"),
        Action::EditTile => ("meshes.define_piece", "Define this piece"),
        Action::Shortcuts => ("help.show_keys", "Show every key on what it does"),
        Action::Save => ("file.save", "Save"),
        Action::Snip => ("tools.snip", "Snip an area of the editor"),
        Action::MainMenu => ("file.main_menu", "Back to the menu"),
        Action::SwitchDoor => ("view.switch_door", "Switch to the other door"),
        Action::ShowErrors => ("view.show_errors", "Show this session's errors"),
        Action::CopyInfo => ("edit.copy_tab_text", "Copy this tab's text"),
        Action::Cancel => ("edit.cancel", "Back out"),
        Action::FrameView => ("view.frame", "Frame the view"),
        Action::PanForward => ("view.pan_forward", "Pan forward"),
        Action::PanLeft => ("view.pan_left", "Pan left"),
        Action::PanBack => ("view.pan_back", "Pan back"),
        Action::PanRight => ("view.pan_right", "Pan right"),
        Action::TurnViewLeft => ("view.turn_left", "Turn the view left"),
        Action::TurnViewRight => ("view.turn_right", "Turn the view right"),
        Action::FocusFilter => ("view.focus_filter", "Filter the list"),
        // ── Map ──────────────────────────────────────────────────────────────
        Action::Undo => ("edit.undo", "Undo"),
        Action::Redo => ("edit.redo", "Redo"),
        Action::CycleTarget => ("map.cycle_target", "Target the stack"),
        Action::PalettePrev => ("map.palette_prev", "Previous piece in the palette"),
        Action::PaletteNext => ("map.palette_next", "Next piece in the palette"),
        Action::PickComposition => ("map.pick_composition", "Walk the compositions"),
        Action::CompositionPrev => ("map.composition_prev", "Previous composition"),
        Action::CompositionNext => ("map.composition_next", "Next composition"),
        Action::TurnPieceLeft => ("map.turn_left", "Turn the piece left"),
        Action::TurnPieceRight => ("map.turn_right", "Turn the piece right"),
        Action::TipX => ("map.tip_x", "Tip the piece about x"),
        Action::TipZ => ("map.tip_z", "Tip the piece about z"),
        Action::Straighten => ("map.straighten", "Straighten the piece"),
        Action::CycleGrid => ("map.cycle_grid", "Cycle the grid rung"),
        Action::LiftDown => ("map.lower", "Lower the piece"),
        Action::LiftUp => ("map.lift", "Lift the piece"),
        Action::Remove => ("map.remove_mode", "Remove mode"),
        Action::MoveMode => ("map.move_mode", "Move mode"),
        Action::CloneMode => ("map.clone_mode", "Clone mode"),
        Action::RunMode => ("map.run_mode", "Run mode"),
        Action::GroupFromSet => ("map.keep_as_composition", "Keep the set as a composition"),
        Action::RenameMap => ("map.rename", "Rename the map"),
        Action::NoteTool => ("map.note", "Leave a note for an agent"),
        Action::SetRule => ("map.set_rule", "Set the rule seed"),
        Action::OwnToggle => ("map.pin", "Pin or unpin the piece"),
        Action::Fill => ("map.fill", "Fill with the brush"),
        Action::Generate => ("map.generate", "Fill with the learned grammar"),
        Action::GenerateDeclared => ("map.generate_declared", "Fill from the declared rules"),
        Action::GenerateComposed => ("map.generate_composed", "Fill from the compositions"),
        Action::AcceptProposal => ("map.accept_layout", "Keep this layout"),
        Action::ArmComposition => ("map.arm_composition", "Arm the picked composition"),
        // ── Meshes ───────────────────────────────────────────────────────────
        Action::PrevCandidate => ("meshes.candidate_prev", "Previous candidate"),
        Action::NextCandidate => ("meshes.candidate_next", "Next candidate"),
        Action::FocusCandidates => ("meshes.focus_candidates", "Walk the candidates"),
        Action::FocusLibrary => ("meshes.focus_library", "Walk the library"),
        Action::TypeId => ("meshes.type_id", "Type an id"),
        Action::CycleMount => ("meshes.cycle_mount", "Cycle the mount"),
        Action::Accept => ("meshes.accept", "Add or update this tile"),
        Action::FoldPack => ("meshes.fold_pack", "Open or close the pack"),
        Action::AcceptEdges => ("meshes.accept_edges", "Keep the derived edges"),
        Action::Rescan => ("meshes.rescan", "Rescan the pack"),
        Action::ExcludePack => ("meshes.exclude_pack", "Exclude or restore the pack"),
        Action::RemoveTile => ("meshes.remove_piece", "Send back to Not imported"),
        Action::UndoTile => ("meshes.undo", "Undo a mesh edit"),
        Action::RedoTile => ("meshes.redo", "Redo a mesh edit"),
        Action::CellForward => ("meshes.cell_forward", "Cell forward"),
        Action::CellLeft => ("meshes.cell_left", "Cell left"),
        Action::CellBack => ("meshes.cell_back", "Cell back"),
        Action::CellRight => ("meshes.cell_right", "Cell right"),
        Action::NewToken => ("meshes.new_token", "Add a vocabulary token"),
        Action::LayerDown => ("meshes.layer_down", "Previous layer"),
        Action::LayerUp => ("meshes.layer_up", "Next layer"),
        Action::CellSolid => ("meshes.cell_solid", "Make this cell solid"),
        Action::CellEdge => ("meshes.cell_edge", "Make this cell an edge"),
        Action::CellClear => ("meshes.cell_clear", "Clear this cell"),
        Action::ScanMesh => ("meshes.scan_solid", "Rescan the solid cells"),
        Action::RotateMeshX => ("meshes.rotate_x", "Turn the mesh about x"),
        Action::RotateMeshY => ("meshes.rotate_y", "Turn the mesh about y"),
        Action::RotateMeshZ => ("meshes.rotate_z", "Turn the mesh about z"),
        Action::SuggestLabels => ("meshes.suggest_labels", "Suggest labels"),
        Action::SuggestAll => ("meshes.suggest_all", "Suggest labels for every piece"),
        Action::DiscardAllSuggestions => {
            ("meshes.discard_suggestions", "Discard every suggestion")
        }
        Action::FocusTagFilter => ("meshes.filter_tags", "Filter the tags"),
        // ── Tiles ────────────────────────────────────────────────────────────
        Action::TilePrev => ("tiles.tile_prev", "Previous tile"),
        Action::TileNext => ("tiles.tile_next", "Next tile"),
        Action::PageEnter => ("tiles.page_enter", "Go to the meshes"),
        Action::PageLeave => ("tiles.page_leave", "Back to the tiles"),
        Action::TileOpen => ("tiles.open", "Open this tile"),
        Action::TileListPrev => ("tiles.library_prev", "Previous in the library"),
        Action::TileListNext => ("tiles.library_next", "Next in the library"),
        Action::BuildForward => ("tiles.move_forward", "Move the piece forward"),
        Action::BuildBack => ("tiles.move_back", "Move the piece back"),
        Action::BuildLeft => ("tiles.move_left", "Move the piece left"),
        Action::BuildRight => ("tiles.move_right", "Move the piece right"),
        Action::MemberPrev => ("tiles.member_prev", "Previous member"),
        Action::MemberNext => ("tiles.member_next", "Next member"),
        Action::AlignForward => ("tiles.align_forward", "Flush the piece forward"),
        Action::AlignBack => ("tiles.align_back", "Flush the piece back"),
        Action::AlignLeft => ("tiles.align_left", "Flush the piece left"),
        Action::AlignRight => ("tiles.align_right", "Flush the piece right"),
        Action::BuildArm => ("tiles.take_piece", "Take the piece in hand"),
        Action::BuildDown => ("tiles.layer_down", "Move the piece down a layer"),
        Action::BuildUp => ("tiles.layer_up", "Move the piece up a layer"),
        Action::BuildRung => ("tiles.grid_deeper", "Make the grid finer"),
        Action::BuildDrop => ("tiles.drop_piece", "Drop the piece"),
        Action::BuildSlot => ("tiles.drop_slot", "Drop the piece in a slot"),
        Action::BuildTurn => ("tiles.turn_piece", "Turn the piece in the tile"),
        Action::BuildDropMember => ("tiles.remove_member", "Remove this member"),
        Action::ClearTile => ("tiles.clear", "Empty the tile"),
        Action::BuildNew => ("tiles.new", "Name a new tile"),
        Action::CloseTile => ("tiles.close", "Close the tile"),
        Action::UndoBuild => ("tiles.undo", "Undo a tile edit"),
        Action::RedoBuild => ("tiles.redo", "Redo a tile edit"),
        // ── Compose ──────────────────────────────────────────────────────────
        Action::ComposePrev => ("compose.prev", "Previous in the focused list"),
        Action::ComposeNext => ("compose.next", "Next in the focused list"),
        Action::ComposeArm => ("compose.arm", "Arm this composition for the map"),
        Action::RenameComposition => ("compose.rename", "Rename this composition"),
        Action::ComposeMemberPrev => ("compose.list_prev", "Walk the previous list"),
        Action::ComposeMemberNext => ("compose.list_next", "Walk the next list"),
        Action::CarouselPrev => ("compose.carousel_prev", "Previous composition in the carousel"),
        Action::CarouselNext => ("compose.carousel_next", "Next composition in the carousel"),
        // ── Rigs ─────────────────────────────────────────────────────────────
        Action::PrevRig => ("rigs.prev", "Previous rig"),
        Action::NextRig => ("rigs.next", "Next rig"),
        Action::AdoptMeasured => ("rigs.adopt_measured", "Adopt the measured values"),
        Action::UndoBench => ("rigs.undo", "Undo a rig edit"),
        Action::RedoBench => ("rigs.redo", "Redo a rig edit"),
        Action::ScrubBack => ("rigs.scrub_back", "Scrub the phase back"),
        Action::ScrubFwd => ("rigs.scrub_forward", "Scrub the phase forward"),
        Action::PlayPause => ("rigs.play_pause", "Play or pause"),
        Action::CheckAllRigs => ("rigs.check_all", "Check all rigs"),
        Action::ToggleGhost => ("rigs.toggle_ghost", "Show the measured gait as a ghost"),
        Action::CycleCamPreset => ("rigs.cycle_camera", "Cycle the stage camera"),
    }
}
