use bevy::prelude::*;
use bevy_grimoire::GamePlugin;

fn main() {
    App::new().add_plugins((DefaultPlugins, GamePlugin)).run();
}
