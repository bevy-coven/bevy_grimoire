//! **The open page's outline** (SG-04, #403): the headings of the Markdown page in the file editor, as a tree you can click.
//!
//! Two ways to reach it, and both read the one reader (`files_lang::outline`, which is `design::headings`):
//!
//! - **An agent** reads `grimoire_state.file_outline`: the open file's path and its headings, each with its `level`, `title` and
//!   `line` (counted from 1 as an editor shows it). [`state_json`] answers it from the text the file editor holds, so a page that is
//!   open and being typed in is read as it is now, saved or not.
//! - **A person** asks for it with View ▸ Show outline, and a gooey `tree` of the headings is put at the foot of the Files
//!   workspace's panel; a heading's click puts the caret on its line and brings it into view.
//!
//! **Not there until it is asked for, and taken away again when it is not**: a tree is entities that the screens counting every
//! button in the world, and the search boxes the settings and keymap screens look for, would find, so it is spawned on demand
//! (the outliner's and the library's rule) and despawned, never kept hidden. It is a block of the Files panel, so leaving the
//! Files workspace takes it with the panel, and the tick on the menu row says whether it is there, whichever way it came or went.
//! It holds no resource of its own.

use bevy::prelude::*;
use bevy_gooey::{SelectMode, Table, TableSelection, Tree, TreeNode, ValueChange, observe, tree};
use serde_json::{Value, json};

use crate::files::{self, FileOutline, Files, FilesPanel};
use crate::files_lang::{self, Heading, Lang};
use crate::screen::Screen;
use crate::session::Session;

/// How tall the outline's tree is, in logical pixels: a tree needs a height to be virtualized in, and the file tree above it
/// takes the rest of the panel.
const OUTLINE_HEIGHT_PX: f32 = 220.0;

/// The View ▸ Show outline row.
#[derive(Component)]
pub struct OutlineItem;

/// The block that holds the outline's heading and tree, and what the tree was last built from.
#[derive(Component, Default)]
pub struct OutlineBlock {
    /// The file and the revision of its text that the rows were drawn from.
    drawn: Option<(Option<String>, Option<u64>)>,
}

/// The outline's tree.
#[derive(Component)]
pub struct OutlineTree;

pub struct FileOutlinePlugin;

impl Plugin for FileOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (keep_the_outline, tick_the_menu_row).chain().run_if(in_state(Screen::Editor)));
    }
}

/// **`grimoire_state.file_outline`**: the open file's headings, for an agent. `file` is null when no file is open in the Files
/// workspace; `headings` is empty for a file with none (any language but Markdown has none).
pub fn state_json(files: Option<&Files>, session: Option<&Session>) -> Value {
    let rel = session.and_then(files::active_file);
    let text = files.zip(rel.as_deref()).and_then(|(files, rel)| files.get(rel)).map(|f| f.text.as_str());
    let (Some(rel), Some(text)) = (rel.as_deref(), text) else {
        return json!({"file": null, "headings": []});
    };
    let headings: Vec<Value> = files_lang::outline(Lang::of(rel), text).iter().map(|h| json!({"level": h.level, "title": h.title, "line": h.line + 1})).collect();
    json!({"file": rel, "headings": headings})
}

/// **View ▸ Show outline puts the block at the foot of the Files panel, and takes it out again.**
pub fn toggle_the_outline(change: On<ValueChange<bool>>, panels: Query<Entity, With<FilesPanel>>, blocks: Query<Entity, With<OutlineBlock>>, mut commands: Commands) {
    if !change.value {
        for block in &blocks {
            commands.entity(block).despawn();
        }
        return;
    }
    if !blocks.is_empty() {
        return;
    }
    let Ok(panel) = panels.single() else { return };
    let block = commands
        .spawn((OutlineBlock::default(), Name::new("file_outline"), bevy_gooey::column(crate::chrome::GAP_TIGHT_STEPS)))
        .with_children(|block| {
            crate::chrome::list_heading(block, "OUTLINE");
            block.spawn(bevy_gooey::sized(bevy_gooey::Sizing { height: Some(bevy_gooey::Extent::Px(OUTLINE_HEIGHT_PX)), ..default() })).with_children(|column| {
                column.spawn((tree(Vec::<TreeNode>::new()), OutlineTree, SelectMode::Single, observe(jump_to_the_picked_heading)));
            });
        })
        .id();
    commands.entity(panel).add_child(block);
}

/// A click on a heading puts the caret on its line.
fn jump_to_the_picked_heading(picked: On<ValueChange<TableSelection>>, mut commands: Commands) {
    let Some(key) = picked.value.iter().next() else { return };
    commands.queue(move |world: &mut World| files::reveal_heading(world, key));
}

/// **The headings as a tree**: a heading is the child of the nearest one before it that is shallower. A row's key is the heading's place
/// in the list. The second answer is the keys of the rows that have rows under them, so each opens.
fn nodes_of(headings: &[Heading]) -> (Vec<TreeNode>, Vec<u64>) {
    fn under(headings: &[Heading], at: &mut usize, parent_level: usize, parents: &mut Vec<u64>) -> Vec<TreeNode> {
        let mut out = Vec::new();
        while let Some(heading) = headings.get(*at).filter(|h| h.level > parent_level) {
            let key = *at as u64;
            *at += 1;
            let children = under(headings, at, heading.level, parents);
            let node = TreeNode::new(key, heading.title.clone());
            if children.is_empty() {
                out.push(node);
            } else {
                parents.push(key);
                out.push(node.children(children));
            }
        }
        out
    }
    let mut parents = Vec::new();
    let nodes = under(headings, &mut 0, 0, &mut parents);
    (nodes, parents)
}

/// **Draw the headings again when the file or its text moved.**
fn keep_the_outline(session: Option<Res<Session>>, outlines: Query<&FileOutline>, mut blocks: Query<&mut OutlineBlock>, mut trees: Query<&mut Table, With<OutlineTree>>) {
    let Ok(mut block) = blocks.single_mut() else { return };
    let rel = session.as_deref().and_then(files::active_file);
    let outline = outlines.iter().next();
    let drawn = (rel, outline.map(|o| o.revision));
    if block.drawn.as_ref() == Some(&drawn) {
        return;
    }
    let Ok(mut table) = trees.single_mut() else { return };
    let Some(tree) = table.source_mut::<Tree>() else { return };
    let (nodes, parents) = outline.map(|o| nodes_of(&o.headings)).unwrap_or_default();
    tree.set_nodes(nodes);
    for key in parents {
        tree.set_open(key, true);
    }
    block.drawn = Some(drawn);
}

/// **The tick on the menu row says whether the block is there**, whichever way it came or went.
fn tick_the_menu_row(items: Query<(Entity, Has<bevy_gooey::Checked>), With<OutlineItem>>, blocks: Query<(), With<OutlineBlock>>, mut commands: Commands) {
    let present = !blocks.is_empty();
    for (item, ticked) in &items {
        match (present, ticked) {
            (true, false) => {
                commands.entity(item).insert(bevy_gooey::Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<bevy_gooey::Checked>();
            }
            _ => {}
        }
    }
}
