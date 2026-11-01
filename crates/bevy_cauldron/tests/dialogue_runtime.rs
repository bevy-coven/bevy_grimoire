//! **A conversation attached to a piece plays in the game when it is talked to** (#106): the saved row names a
//! conversation, the project's `DialogueScript` defines it, and the crate's own runtime walks it.
#![cfg(feature = "bevy_speech_bubbles")]

use bevy::prelude::*;
use bevy_cauldron::ops::{self, Applied};
use bevy_speech_bubbles::{
    BubbleAssets,
    ChoicePicked, DialogueNodeEntered, DialogueScript, Speaker, Speakers, SpeechBubblesPlugin, StartConversation,
};
use serde_json::json;

use crate::fixtures;

const SCRIPT: &str = r#"(conversations: {"hello": (start: "ask", nodes: {
    "ask": Choice(speaker: "Mara", prompt: "Stay?", options: [(text: "Yes", next: "yes"), (text: "No", next: "no")]),
    "yes": Line(speaker: "Mara", text: "Good.", next: None),
    "no": Line(speaker: "Mara", text: "Pity.", next: None),
})})"#;

struct Talker;
impl Component for Talker {
    type Mutability = bevy::ecs::component::Mutable;
    const STORAGE_TYPE: bevy::ecs::component::StorageType = bevy::ecs::component::StorageType::Table;
}

#[test]
fn the_conversation_a_piece_carries_is_walked_by_the_crates_runtime_down_the_chosen_option() {
    let root = fixtures::Fixture::new("dialogue_runtime").descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_speech_bubbles\"])").unwrap();
    std::fs::write(root.join("family/bevy_speech_bubbles.ron"), SCRIPT).unwrap();
    let mut editor = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        editor.update();
    }
    let project = editor.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = editor.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    let mut ctx = ops::Ctx { project: Some(project), open: Some(open), session: "dialogue_runtime".into(), ..ops::Ctx::default() };
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let done = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_bubbles_unused", "value": "\"hello\""}));
    assert!(done.is_err() || matches!(done, Ok(Applied::Refused(_))), "a crate the project does not list is refused");
    let done = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_speech_bubbles", "value": "\"hello\""})).unwrap();
    assert!(matches!(done, Applied::Done { .. }), "{done:?}");

    // The game reads the saved map and the saved script, as it would at load.
    let map: bevy_grimoire_core::map::Map =
        ron::from_str(&std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.ron")).unwrap()).unwrap();
    let name: String = ron::from_str(&map.placements[0].family["bevy_speech_bubbles"]).unwrap();
    let script: DialogueScript = ron::from_str(&std::fs::read_to_string(root.join("family/bevy_speech_bubbles.ron")).unwrap()).unwrap();

    let mut game = App::new();
    game.add_plugins((MinimalPlugins, AssetPlugin::default(), SpeechBubblesPlugin::<Talker>::default()))
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Mesh>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(script);
    let mara = game.world_mut().spawn(Transform::default()).id();
    let font = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/FiraMono-Regular.ttf")).unwrap();
    game.insert_resource(BubbleAssets::from_font_bytes(Handle::default(), font).unwrap());
    game.world_mut().resource_mut::<Speakers>().0.insert("Mara".into(), Speaker { entity: Some(mara), name: "Mara".into(), portrait: None });
    game.world_mut().write_message(StartConversation { id: name.clone() });
    let mut entered: Vec<String> = Vec::new();
    let mut frame = |game: &mut App| {
        game.update();
        entered.extend(game.world_mut().resource_mut::<Messages<DialogueNodeEntered>>().drain().map(|e| format!("{}:{}", e.conversation, e.node)));
    };
    for _ in 0..4 {
        frame(&mut game);
    }
    game.world_mut().write_message(ChoicePicked { index: 1 });
    for _ in 0..3 {
        frame(&mut game);
    }
    assert_eq!(entered, ["hello:ask", "hello:no"], "the talker was asked, and the second option led to its line");
}
