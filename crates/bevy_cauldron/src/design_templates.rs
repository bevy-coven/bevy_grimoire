//! **A new project starts with a style, and each topic starts from a template** (SG-08, #407; the series #399).
//!
//! The New project wizard writes `design/style.md` from [`STYLE_TEMPLATE`], and in the Design workspace **New topic** writes
//! `design/<topic>.md` from that topic's template. The templates are `include_str!`'d from the one folder beside this file,
//! `design_templates/`, so the text a person reads in the file and the text a test asserts are the same bytes.
//!
//! **A fixed, shallow shape that an agent learns once.** Every `style.md` has one H1, the place's name, and the same nine H2s in
//! the same order ([`STYLE_H2S`]); each H2 holds one italic prompt line and nothing else. Agent context files in practice keep a
//! single H1 with H2 and H3 topics (10.1145/3840295), and a fixed order lets an agent find the same rule in every file.
//!
//! **Rules an agent can check, not adjectives.** What Do and Don't ask for is one statement to a line that a screenshot can
//! confirm or refute (Sanctuary's own: "No feature under 0.10 m unless it changes the silhouette"), and Pictures asks for each
//! picture's alt text to say what to take from it (SG-03).
//!
//! **The topics** are the five in [`TOPICS`], named in this one place: `dialogue`, `interaction`, `sound`, `ui` and `names`, each
//! with the same one-H1, short-H2 shape. A topic below the game must exist at the game (`design::Tree::problems` says so), so
//! [`topics_offered`] offers a lower place only the topics the game already has.
//!
//! **Nothing here writes into a project the editor did not create.** Opening a project writes no file; the wizard and **Create
//! style** are the two acts that call [`write_style`], and neither overwrites a file that is there.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::design::{self, Tree};

/// **The token a template carries where the place's name goes**, replaced when the template is written.
pub const PLACE_TOKEN: &str = "{place}";

/// **The template of a place's `style.md`**, compiled in so a build with no assets folder still writes it.
pub const STYLE_TEMPLATE: &str = include_str!("design_templates/style.md");

/// **The nine H2s every `style.md` has, in the order the template writes them.** A test reads the written file against this list, so a
/// section cannot be dropped or reordered without failing there.
pub const STYLE_H2S: [&str; 9] = ["Pillars", "Mood", "Palette", "Light", "Shapes and materials", "Sound", "Do", "Don't", "Pictures"];

/// **The topics a game can start from a template, and the template of each**, in the order a place offers them.
pub const TOPICS: [(&str, &str); 5] = [
    ("dialogue", include_str!("design_templates/dialogue.md")),
    ("interaction", include_str!("design_templates/interaction.md")),
    ("sound", include_str!("design_templates/sound.md")),
    ("ui", include_str!("design_templates/ui.md")),
    ("names", include_str!("design_templates/names.md")),
];

/// **The template of a topic nobody wrote one for**: a topic an author made at the game by hand, which a lower place may take up.
/// It has the same one-H1, one-H2 shape, with the topic's name where it is needed.
const OTHER_TOPIC_TEMPLATE: &str = "# {place}: {topic}\n\n## Notes\n\n*What this place says about {topic}, in a few short lines.*\n";

/// The place of the whole game, as a folder under the project root.
const GAME_PLACE: &str = design::DESIGN_DIR;

/// **The text of a place's `style.md`**, with the place's name as its one H1.
pub fn style_text(place_name: &str) -> String {
    STYLE_TEMPLATE.replace(PLACE_TOKEN, place_name.trim())
}

/// **The text of a topic's template for a place**, or `None` for a topic that has none.
pub fn topic_text(topic: &str, place_name: &str) -> Option<String> {
    TOPICS.iter().find(|(name, _)| *name == topic).map(|(_, template)| template.replace(PLACE_TOKEN, place_name.trim()))
}

/// **Grant the game's `design/style.md` as it stands in `root`**, which is the other half of the wizard's writing it: the person who made the project
/// wrote this text, so it is allowed, and `grimoire_style` serves it with no question (SG-02, #401). `store` is the override, as for the guide's
/// grant: `None` is the person's own `trusted.yaml`. A store that cannot be read or written is an `Err` naming it, never a silent grant-less style.
pub fn grant_style(root: &Path, store_root: Option<&crate::trusted::StoreRoot>, store: Option<&Path>) -> Result<(), String> {
    grant_file(root, &format!("{GAME_PLACE}/{}.{}", design::STYLE_TOPIC, design::MARKDOWN_EXT), store_root, store)
}

/// **Grant one style file as it stands in `root`**, `path` relative to the project root: the Design workspace's Create style and New topic write the
/// editor's own template, which the person asked for, so it is allowed as the wizard's `design/style.md` is. A changed byte is a new question.
pub fn grant_file(root: &Path, path: &str, store_root: Option<&crate::trusted::StoreRoot>, store: Option<&Path>) -> Result<(), String> {
    let text = std::fs::read_to_string(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
    let store_file = crate::trusted::resolve(store_root, store)?;
    let mut trusted = bevy_grimoire_core::trust::TrustStore::load(&store_file)?;
    trusted.trust_text(root, path, &text)?;
    trusted.save(&store_file)
}

/// **The folder of a place on disk, or the sentence saying it is none.**
fn folder_of(root: &Path, place: &str) -> Result<PathBuf, String> {
    let place = place.trim_end_matches('/');
    if bevy_grimoire_core::board::is_place(place) {
        Ok(root.join(place))
    } else {
        Err(format!("`{place}` is not a place of the design tree (`design`, `design/levels/<map>`, `design/regions/<region>`, an area). Nothing was written."))
    }
}

/// **Write a place's `style.md` from the template**, which is the whole of **Create style** and the wizard's `design/style.md`. A file that
/// is already there is never overwritten.
pub fn write_style(root: &Path, place: &str, place_name: &str) -> Result<PathBuf, String> {
    let file = folder_of(root, place)?.join(format!("{}.{}", design::STYLE_TOPIC, design::MARKDOWN_EXT));
    write_new_file(&file, &style_text(place_name))?;
    Ok(file)
}

/// **Write `<place>/<topic>.md` from the topic's template.**
///
/// On the game it must be one of [`TOPICS`]. **Below the game** it must be a topic the game already has (`design/<topic>.md` is a file), so a
/// region never grows a topic the rest of the game does not mention; the game's own template is used when there is one, and the generic
/// one otherwise. A file that is there is never overwritten.
pub fn create_topic(root: &Path, place: &str, topic: &str, place_name: &str) -> Result<PathBuf, String> {
    let folder = folder_of(root, place)?;
    if !design::is_topic_name(topic) || topic == design::STYLE_TOPIC {
        return Err(format!("`{topic}` is not a topic name a place can have: letters, digits, `_` and `-`, and not `{}`. Nothing was written.", design::STYLE_TOPIC));
    }
    let on_the_game = place.trim_end_matches('/') == GAME_PLACE;
    let text = if on_the_game {
        topic_text(topic, place_name).ok_or_else(|| {
            let known: Vec<&str> = TOPICS.iter().map(|(n, _)| *n).collect();
            format!("`{topic}` is not a topic the game starts from a template: they are {}. Nothing was written.", known.join(", "))
        })?
    } else {
        let at_the_game = root.join(GAME_PLACE).join(format!("{topic}.{}", design::MARKDOWN_EXT));
        if !at_the_game.is_file() {
            return Err(format!("the game has no `{topic}` topic, and a place below it can only take up the game's topics: write {GAME_PLACE}/{topic}.md first. Nothing was written."));
        }
        topic_text(topic, place_name).unwrap_or_else(|| OTHER_TOPIC_TEMPLATE.replace(PLACE_TOKEN, place_name.trim()).replace("{topic}", topic))
    };
    let file = folder.join(format!("{topic}.{}", design::MARKDOWN_EXT));
    write_new_file(&file, &text)?;
    Ok(file)
}

/// **The topics a place may start now.** The game is offered each of [`TOPICS`] it does not have; **any lower place is offered only the game's
/// topics** it does not have itself, in the tree's order.
pub fn topics_offered(tree: &Tree, place: &str) -> Vec<String> {
    let has = |path: &str| -> Vec<&str> {
        tree.place(path).map(|p| p.topics.iter().map(|t| t.name.as_str()).filter(|n| *n != design::STYLE_TOPIC).collect()).unwrap_or_default()
    };
    let place = place.trim_end_matches('/');
    let own = has(place);
    if place == GAME_PLACE {
        return TOPICS.iter().map(|(n, _)| *n).filter(|n| !own.contains(n)).map(str::to_owned).collect();
    }
    has(GAME_PLACE).into_iter().filter(|n| !own.contains(n)).map(str::to_owned).collect()
}

/// **Write a new file, refusing one that is there.** The kernel refuses (`link(2)` fails with `EEXIST`), so two writers cannot both win.
fn write_new_file(file: &Path, text: &str) -> Result<(), String> {
    bevy_grimoire_core::atomic_file::write_new(file, text).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("{} is already there, so it was not written over. Nothing was written.", file.display())
        } else {
            format!("{}: {e}", file.display())
        }
    })
}
