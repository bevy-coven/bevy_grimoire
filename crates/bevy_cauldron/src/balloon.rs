//! **The speech balloon of a dialogue preview, over the piece that says it** (#106).
//!
//! `dialogue <conversation> [options]` walks a conversation through `dialogue_preview`; the lines it
//! reports are kept here and drawn as one balloon above the placed piece that carries that
//! conversation (`family["bevy_speech_bubbles"]` is its name), so an author sees the words where the
//! speaker stands. A measurement kept for drawing: nothing is saved, and an empty preview draws nothing.

use bevy::prelude::*;
use bevy_grimoire_core::map::Map;

use crate::editor::Placement;
use crate::project::OpenMap;

/// Balloon width, logical pixels.
const WIDTH: f32 = 220.0;
/// How far above the piece's origin the balloon's tail points, metres.
const LIFT: f32 = 2.2;

/// The lines the last `dialogue` walk said, and where they are said.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct BalloonPreview {
    /// The id of the placed piece carrying the conversation; `None` anchors at the map's origin.
    pub speaker: Option<String>,
    /// `(speaker name, text)` in order said.
    pub lines: Vec<(String, String)>,
}

impl BalloonPreview {
    /// Read the `lines` of a `dialogue_preview` outcome's `after`, anchored to the piece in `map`
    /// that carries `conversation`.
    pub fn from_after(after: &serde_json::Value, map: Option<&Map>, conversation: &str) -> BalloonPreview {
        let lines = after["lines"]
            .as_array()
            .map(|l| {
                l.iter()
                    .map(|x| (x["speaker"].as_str().unwrap_or("").to_owned(), x["text"].as_str().unwrap_or("").to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        BalloonPreview { speaker: map.and_then(|m| speaker_of(m, conversation)).map(str::to_owned), lines }
    }

    /// The words of the balloon: one `speaker: text` row per line.
    pub fn text(&self) -> String {
        self.lines.iter().map(|(who, text)| format!("{who}: {text}")).collect::<Vec<_>>().join("\n")
    }
}

/// The first placed piece whose `bevy_speech_bubbles` value is the conversation's name (a RON string).
pub fn speaker_of<'a>(map: &'a Map, conversation: &str) -> Option<&'a str> {
    map.placements
        .iter()
        .find(|p| {
            p.family
                .get("bevy_speech_bubbles")
                .and_then(|v| ron::from_str::<String>(v).ok())
                .is_some_and(|name| name == conversation)
        })
        .map(|p| p.id.as_str())
}

/// The one balloon node.
#[derive(Component, Clone, Copy)]
pub struct Balloon;

/// **Draw the balloon** above its speaker, following the camera. Spawned when there are lines to say
/// and taken down when there are none.
pub fn show_balloon(
    mut commands: Commands,
    preview: Res<BalloonPreview>,
    open: Option<Res<OpenMap>>,
    ui_scale: Res<UiScale>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<crate::view::MainCamera>>>,
    placed: Query<(&Placement, &GlobalTransform)>,
    mut balloons: Query<(Entity, &mut Node, &mut Text), With<Balloon>>,
) {
    let text = preview.text();
    let anchor = camera.as_ref().zip(open.as_ref()).and_then(|(camera, open)| {
        let (cam, cam_tf) = **camera;
        let vp = cam.logical_viewport_size().filter(|s| s.x > 0.0 && s.y > 0.0)?;
        let base = preview
            .speaker
            .as_ref()
            .and_then(|id| placed.iter().find(|(p, _)| &p.0 == id).map(|(_, t)| t.translation()))
            .unwrap_or(Vec3::new(open.map.origin.0, open.map.origin.1, open.map.origin.2));
        let p = cam.world_to_viewport(cam_tf, base + Vec3::Y * LIFT).ok()?;
        Some((p / ui_scale.0, vp / ui_scale.0))
    });
    let (Some((at, vp)), false) = (anchor, text.is_empty()) else {
        for (e, ..) in &balloons {
            commands.entity(e).despawn();
        }
        return;
    };
    let left = (at.x - WIDTH * 0.5).clamp(0.0, (vp.x - WIDTH).max(0.0));
    let bottom = (vp.y - at.y).max(0.0);
    if let Ok((_, mut node, mut shown)) = balloons.single_mut() {
        node.left = Val::Px(left);
        node.bottom = Val::Px(bottom);
        if shown.0 != text {
            shown.0 = text;
        }
        return;
    }
    commands.spawn((
        Balloon,
        Text::new(text),
        TextColor(crate::chrome::TEXT),
        crate::chrome::font(crate::chrome::text::LABEL),
        BackgroundColor(crate::chrome::OVERLAY_BG),
        Node {
            // PLACES-ITSELF-OK: a balloon is anchored to a piece projected through the camera, not a panel.
            position_type: PositionType::Absolute,
            width: Val::Px(WIDTH),
            left: Val::Px(left),
            bottom: Val::Px(bottom),
            padding: UiRect::all(Val::Px(crate::chrome::GAP_ROW)),
            ..default()
        },
        GlobalZIndex(crate::chrome::BALLOON_Z),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_grimoire_core::map::Placed;

    fn placed(id: &str, convo: Option<&str>) -> Placed {
        let mut p = Placed { id: id.to_owned(), descriptor: "npc".to_owned(), ..Default::default() };
        if let Some(c) = convo {
            p.family.insert("bevy_speech_bubbles".to_owned(), format!("{c:?}"));
        }
        p
    }

    #[test]
    fn the_balloon_stands_over_the_piece_that_carries_the_conversation() {
        let map = Map { placements: vec![placed("a", None), placed("b", Some("hello")), placed("c", Some("bye"))], ..Map::default() };
        let after = serde_json::json!({"lines": [{"node": "n", "speaker": "Ann", "text": "Hi"}, {"node": "m", "speaker": "Bo", "text": "Yo"}]});
        let b = BalloonPreview::from_after(&after, Some(&map), "bye");
        assert_eq!(b.speaker.as_deref(), Some("c"));
        assert_eq!(b.text(), "Ann: Hi\nBo: Yo");
        assert_eq!(BalloonPreview::from_after(&after, Some(&map), "nobody").speaker, None);
    }
}
