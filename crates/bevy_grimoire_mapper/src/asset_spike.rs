//! **A spike, not a migration (#204, hot-reload H7): `standards.ron` through Bevy's asset system, beside
//! [`bevy_grimoire_core::live::FileWatch`].**
//!
//! The question is whether the editor's own watcher (a stat of the project's files, one reload after
//! the writes settle) should be Bevy's `AssetLoader` with its file watcher instead. This module is the
//! smallest honest version of the other side: one asset type, one loader that reads the file with the
//! type's own reader (`Standards::parse`, so a refusal is the one the reload verdict names), and a
//! headless app that has Bevy's watcher on. `tests/asset_spike.rs` and `examples/asset_spike_measure.rs`
//! put the two side by side; `docs/guide/reload-contract.md` ("The asset system against the watcher")
//! has the measurements and the recommendation.
//!
//! Behind the cargo feature `asset_spike`, which turns on Bevy's `file_watcher` (the `notify`
//! crates) and nothing else. No build of the editor without the feature contains any of it.

use std::fmt;
use std::path::Path;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, AssetPlugin, LoadContext};
use bevy::prelude::*;
use bevy_grimoire_core::standards::Standards;

/// `standards.ron` as an asset: the parsed rules, held by Bevy's `Assets` storage.
#[derive(Asset, TypePath, Debug, PartialEq)]
pub struct StandardsAsset(pub Standards);

/// Why a standards file did not load: the type's own refusal, or the read.
#[derive(Debug)]
pub struct StandardsError(pub String);

impl fmt::Display for StandardsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StandardsError {}

/// Reads a `.ron` as [`Standards`] with `Standards::parse`, the reader `load_standards` uses.
#[derive(Default, TypePath)]
pub struct StandardsLoader;

impl AssetLoader for StandardsLoader {
    type Asset = StandardsAsset;
    type Settings = ();
    type Error = StandardsError;

    async fn load(&self, reader: &mut dyn Reader, _: &(), _: &mut LoadContext<'_>) -> Result<StandardsAsset, StandardsError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await.map_err(|e| StandardsError(e.to_string()))?;
        let text = String::from_utf8(bytes).map_err(|e| StandardsError(e.to_string()))?;
        Standards::parse(&text).map(StandardsAsset).map_err(StandardsError)
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

/// What the app has heard, in order: a `Modified` or `Added` event for the standards asset, with the
/// asset as it was then, and a load failure with its text.
#[derive(Resource, Default)]
pub struct Heard {
    pub events: Vec<Heard1>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Heard1 {
    Loaded(Standards),
    Failed(String),
}

fn listen(
    mut events: MessageReader<AssetEvent<StandardsAsset>>,
    assets: Res<Assets<StandardsAsset>>,
    mut failed: MessageReader<AssetLoadFailedEvent<StandardsAsset>>,
    mut heard: ResMut<Heard>,
) {
    for event in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = event
            && let Some(asset) = assets.get(*id)
        {
            heard.events.push(Heard1::Loaded(asset.0.clone()));
        }
    }
    for event in failed.read() {
        heard.events.push(Heard1::Failed(event.error.to_string()));
    }
}

/// A headless app that loads `<dir>/standards.ron` through the asset system with Bevy's file watcher
/// on (Bevy's own default debounce, 300 ms) and returns it with the handle. `dir` is absolute.
pub fn app_over(dir: &Path) -> (App, Handle<StandardsAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin { file_path: dir.to_string_lossy().into_owned(), watch_for_changes_override: Some(true), ..default() },
    ))
    .init_asset::<StandardsAsset>()
    .register_asset_loader(StandardsLoader)
    .init_resource::<Heard>()
    .add_systems(Update, listen);
    let handle = app.world().resource::<AssetServer>().load::<StandardsAsset>(bevy_grimoire_core::standards::FILE);
    (app, handle)
}
