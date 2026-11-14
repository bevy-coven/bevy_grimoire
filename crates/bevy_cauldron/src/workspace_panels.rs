//! **The World and Sound workspaces' panels** (NV-08, #385).
//!
//! Until NV-08 every panel that was not the Map's, the Kit's or the rig bench's lived in the Map's INSPECTOR, one
//! scroll below the next: the terrain line, the environment's clock and weather, the light pool, the audio table.
//! The navigation baseline (`docs/ui/navigation-baseline.md`) measured what that cost: five to sixty wheel notches
//! to reach a heading, and four destinations that no route reached at all. A workspace is the *job*
//! (`docs/ui/vision.md` §4), and these are two jobs of their own, so each now has a panel of its own and the Map's
//! INSPECTOR has lost them (**nothing is reached a second way**).
//!
//! # What is here, and what is not
//!
//! - **World** — the ground and the sky under the level: `TERRAIN` always, the map's `LIGHTS` fixtures in every build (`world_lists`,
//!   NV-11, #395), and the family crates' sections when the build has them (`ENVIRONMENT` for `bevy_elements`, the pool's budget under
//!   `LIGHTS` for `bevy_lights`, `LIGHT MAP` for `bevy_light_grid`). The same functions draw the same nodes, so every observer and
//!   readout they have keeps working; only the parent moved.
//! - **Sound** — the audio panel: `AUDIO` with the project's cues listed in every build (`world_lists`, NV-11, #392), and the audition
//!   controls when the build has `bevy_siren`. A build without that crate says so in a sentence beside the list.
//! - **Water and forest are read here and written on the terrain line** (NV-11, #393, #394): `WATER` and `FOREST` list what the map's
//!   plan and forest files hold. They are operations (`water_place`, `forest_scatter`) and a second way to place them would be a second
//!   path to one operation; what the World workspace adds is the thing the line never showed, which is what the map holds now.
//!
//! # Spawned on demand
//!
//! Each panel is spawned when its workspace is entered (the frame is rebuilt on every switch, `screen::cross`) and
//! not kept hidden elsewhere: the interface's protected tests count every button and pick the first search field
//! over the whole world, so a panel that is merely `Display::None` still counts.

use bevy::prelude::*;

use crate::chrome::{self, Frame, FrameSystems, Side};
use crate::screen::Screen;
use crate::workspace::Workspace;

/// The root of the World workspace's panel, so a test and the layout can name it.
#[derive(Component)]
pub struct WorldRoot;

/// The World panel's scrolling body: the pane its sections are children of.
#[derive(Component)]
pub struct WorldPane;

/// The root of the Sound workspace's panel.
#[derive(Component)]
pub struct SoundRoot;

/// The sentence the Sound workspace shows in a build without `bevy_siren`: what is missing, and what to do about it.
pub const NO_AUDIO: &str = "This build has no audio panel. Build the editor with the bevy_siren feature to edit cues and buses";

/// The two panels' spawners and the heading both carry.
pub struct WorkspacePanelsPlugin;

impl Plugin for WorkspacePanelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(Screen::Editor),
            (
                spawn_world_panel.run_if(Workspace::world_is_open),
                spawn_sound_panel.run_if(Workspace::sound_is_open),
            )
                .after(FrameSystems),
        );
    }
}

/// **One section's column**: the block every section is spawned into, so the rows of one belong to each other. The terrain section is
/// spawned as its own column.
fn block<'a>(p: &'a mut ChildSpawnerCommands) -> EntityCommands<'a> {
    p.spawn(bevy_gooey::column(chrome::GAP_TIGHT_STEPS))
}

/// **The World workspace's panel**, in the left dock like every inspector, scrolling like the Map's.
fn spawn_world_panel(mut commands: Commands, frame: Res<Frame>) {
    let root = chrome::panel_root(
        &mut commands,
        &frame,
        Side::Left,
        chrome::CONTROLS_W,
        true,
        false,
        // What a screen reader calls this panel (UI-30, #224): the heading it draws.
        "INSPECTOR",
    )
    .insert(WorldRoot)
    .id();
    commands.entity(root).with_children(|p| {
        chrome::title(p, "INSPECTOR");
        // FOLLOW-OK: a stack of fields and buttons, not a list; nothing in it is walked by the arrows, so the scroll
        // exists for HEIGHT, as the Map's inspector's does.
        chrome::scroll_list(p, WorldPane).with_children(|p| {
            // **Environment (#98):** the clock scrubber and the forced weather, for a project that uses `bevy_elements`.
            #[cfg(feature = "bevy_elements")]
            block(p).with_children(crate::environment::spawn_section);

            // **Light map (#103):** show or hide `bevy_light_grid`'s field over the open map.
            #[cfg(feature = "bevy_light_grid")]
            block(p).with_children(crate::light_overlay::spawn_section);

            // **Terrain (#99):** one line, `<brush> <x> <z> <radius_m> <strength>`.
            crate::editor::spawn_terrain_section(p);

            // **Water (NV-11, #393):** what the map's water plan holds. Placed on the terrain line above.
            block(p).with_children(crate::world_lists::spawn_water_section);

            // **Forest (NV-11, #394):** what the map's forest holds. Planted on the terrain line above.
            block(p).with_children(crate::world_lists::spawn_forest_section);

            // **Lights (#103, NV-11 #395):** the fixtures the map carries, in every build; the family crate's section adds the pool's
            // budget from where the camera looks, and draws the heading itself. After the terrain line, which keeps its place.
            block(p).with_children(|s| {
                #[cfg(feature = "bevy_lights")]
                crate::ops_pool::spawn_section(s);
                #[cfg(not(feature = "bevy_lights"))]
                chrome::section(s, "LIGHTS");
                crate::world_lists::spawn_lights_list(s);
            });
        });
    });
}

/// **The Sound workspace's panel**: the audio table, or the sentence that says this build has none.
fn spawn_sound_panel(mut commands: Commands, frame: Res<Frame>) {
    let root = chrome::panel_root(
        &mut commands,
        &frame,
        Side::Left,
        chrome::CONTROLS_W,
        true,
        false,
        "INSPECTOR",
    )
    .insert(SoundRoot)
    .id();
    commands.entity(root).with_children(|p| {
        chrome::title(p, "INSPECTOR");
        // **The cues (NV-11, #392):** listed in every build, from the project's `family/bevy_siren.ron`. Auditioning one needs the
        // crate, so a build without it says so beside the list.
        block(p).with_children(|s| {
            #[cfg(feature = "bevy_siren")]
            crate::audio_panel::spawn_section(s);
            #[cfg(not(feature = "bevy_siren"))]
            {
                chrome::section(s, "AUDIO");
                s.spawn(bevy_gooey::label(NO_AUDIO));
            }
            crate::world_lists::spawn_cue_list(s);
        });
    });
}
