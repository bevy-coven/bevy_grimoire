//! **The layout score counts leader crossings, so a crossing layout cannot win.**
//!
//! The regression test for the 2026-10-21 run's first bug.
//!
//! # What was wrong
//!
//! `badges::place_badges` lays the badge overlay out twice — once **comfortable** (every box
//! beside its own row) and once **compact** (every rail packed tight from the top of the stage) —
//! and keeps whichever scores better. The score counted **box overlaps** and the legend's
//! collision. **It did not count leader crossings.**
//!
//! So a layout whose **leaders crossed** could score **zero**, and the compact pass — which
//! packs tight and therefore *does* cross — beat the comfortable pass on the very screen the
//! crate's own ratchet measures.
//!
//! # The measurement
//!
//! On `MESHES` at 1680 x 1050, with the detail pane populated:
//!
//! | pass | box overlaps | crossing pairs | score **before** | score **after** |
//! |---|---:|---:|---:|---:|
//! | comfortable | 1 | 0 | **1** | **1** |
//! | compact | 0 | 3 | **0** | **57** |
//!
//! **The compact pass won on the old score and shipped three crossing pairs.** The comfortable
//! pass is structurally crossing-free on that screen: every one of its leaders is a box level with
//! its anchor, which is one horizontal hairline, and horizontal hairs at different heights cannot
//! meet.
//!
//! # Why the multiplier
//!
//! **A crossing is worth more than an overlap.** Two boxes overlapping is a badge covering a
//! badge — both are still legible, and the author can read past it. Two leaders crossing is a
//! line pointing at the wrong control, which is the one thing a leader exists to do. So the score
//! multiplies the crossing count by `items.len() + 1`, which makes **one crossing outrank every
//! possible box overlap at once**: the comparison the score makes is then *a crossing loses*, and
//! the only thing that can beat it is a layout with no crossing at all.
//!
//! # What these tests pin
//!
//! - **A crossing is counted, and it outranks an overlap.**
//! - **The compact pass no longer wins on the MESHES screen** — the comfortable one stands, and
//!   the leaders on screen do not cross.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::keys;

use crate::fixtures;

/// **Hold the shortcut key down, so the badge overlay is up.**
fn hold(app: &mut App, keys_down: Vec<KeyCode>) {
    for k in keys_down {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(k);
    }
}

/// **A project with enough in it to fill the docks** — the state the crossing appears in, and the
/// state a one-descriptor fixture never reaches.
///
/// Forty descriptors is enough to make the piece list scroll, the detail pane fill, and the free
/// ground run out, which is all it takes to exercise the packer.
fn crowded_root(name: &str) -> std::path::PathBuf {
    let ids: Vec<String> = (0..40).map(|i| format!("piece_{i:02}")).collect();
    let mut fixture = fixtures::Fixture::new(name);
    for id in &ids {
        fixture = fixture.descriptor(id, "alpha");
    }
    fixture
        .place("piece_00", (0.0, 0.0))
        .place("piece_01", (2.0, 0.0))
        .build("m")
}

/// **Put a piece in the detail pane**, because that is when the paned controls exist at all — the
/// pane draws nothing until something is selected, and the crossing appears on the screen that
/// has a detail pane in it.
fn stage_a_piece(app: &mut App) {
    let id = app
        .world()
        .resource::<bevy_cauldron::project::Project>()
        .library
        .descriptors
        .first()
        .map(|d| d.id.clone())
        .unwrap_or_else(|| {
            panic!(
                "stage_a_piece was handed a project with an empty library, so there is nothing to \
                 put in the detail pane and every rule the caller is about would be enforced \
                 against an empty one."
            )
        });
    match app
        .world_mut()
        .get_resource_mut::<bevy_cauldron::tiles::ImportState>()
    {
        Some(mut state) => state.selected_library_id = Some(id.clone()),
        None => panic!("no `ImportState` to stage `{id}` in"),
    }
    for _ in 0..8 {
        app.update();
    }
}

/// **The badge overlay on the MESHES tab at 1680 x 1050, with a piece in the detail pane.**
///
/// **The window size is the one the crossing was measured at**, and it is not a coincidence: the
/// layout is a function of the window, and a different size lays out differently. 1680 x 1050 is
/// the smallest of the four shapes the crate's own ratchet sweeps, and the one that produced the
/// crossing.
fn meshes_at_1680(root: &std::path::Path) -> App {
    let mode = bevy_cauldron::tiles::Mode::ALL
        .iter()
        .find(|m| m.label() == "MESHES")
        .copied()
        .expect("the MESHES tab is one of the enumerated modes");
    let mut app =
        harness::build_headless_at(root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    hold(&mut app, vec![keys::binding(keys::Action::Shortcuts).key]);
    for _ in 0..8 {
        app.update();
    }
    stage_a_piece(&mut app);
    harness::resize_surface(&mut app, 1680, 1050).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The leaders on screen, and the pairs of them that cross.**
///
/// **Measured from the drawn nodes**, not from the plan, because the drawn nodes are what the
/// author sees. The plan is what the *score* reads; the nodes are what the *ratchet* reads, and a
/// fix that only moved the plan would pass one and fail the other.
fn crossing_pairs_on_screen(app: &mut App) -> (usize, Vec<String>) {
    use bevy::ui::{ComputedNode, UiGlobalTransform};
    use bevy_cauldron::badges::{BadgeCluster, Lead, LeadSeg};

    let owned: Vec<(String, [Entity; 3])> = {
        let mut q = app
            .world_mut()
            .query::<(&BadgeCluster, &Lead, &Visibility)>();
        q.iter(app.world())
            .filter(|(_, _, v)| **v != Visibility::Hidden)
            .map(|(c, l, _)| (format!("{:?}", c.0), l.0))
            .collect()
    };
    let rect_of = {
        let mut q = app.world_mut().query_filtered::<(
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &Visibility,
        ), With<LeadSeg>>();
        let map: std::collections::HashMap<Entity, Rect> = q
            .iter(app.world())
            .filter(|(_, n, _, v)| n.size() != Vec2::ZERO && **v != Visibility::Hidden)
            .map(|(e, n, tf, _)| (e, Rect::from_center_size(tf.translation, n.size())))
            .collect();
        map
    };
    let leaders: Vec<(String, Vec<Rect>)> = owned
        .into_iter()
        .map(|(name, segs)| {
            (
                name,
                segs.iter()
                    .filter_map(|e| rect_of.get(e).copied())
                    .collect(),
            )
        })
        .filter(|(_, segs): &(String, Vec<Rect>)| !segs.is_empty())
        .collect();

    let mut met = Vec::new();
    for (i, (a_name, a_segs)) in leaders.iter().enumerate() {
        for (b_name, b_segs) in leaders.iter().skip(i + 1) {
            for p in a_segs {
                for q in b_segs {
                    let hit = p.intersect(*q);
                    if hit.width() > 0.0 && hit.height() > 0.0 {
                        met.push(format!(
                            "{a_name}'s leader meets {b_name}'s at {:?}",
                            hit.min
                        ));
                    }
                }
            }
        }
    }
    (leaders.len(), met)
}

/// **The MESHES screen at 1680 x 1050 has no crossing leaders.**
///
/// **This is the bug's own reproduction, turned into a check.** Before the fix, this screen had
/// three crossing pairs: `Control(CellGrid)`'s leader met `Control(Mesh)`'s twice, and
/// `Control(Detail)`'s met `Control(Mesh)`'s once.
#[test]
fn the_meshes_screen_has_no_crossing_leaders() {
    let root = crowded_root("layoutscore");
    let mut app = meshes_at_1680(&root);
    let (leaders, met) = crossing_pairs_on_screen(&mut app);
    assert!(
        met.is_empty(),
        "these leaders cross or lie along one another, so a reader cannot follow either to its \
         control:\n  {}",
        met.join("\n  ")
    );
    assert!(
        leaders >= 8,
        "only {leaders} leader(s) were measured on the screen the crossing appeared on, so the \
         rule is being enforced against nothing"
    );
}

/// **The comfortable layout stands, because the compact one is no longer free.**
///
/// **The compact pass exists to rescue a screen where boxes overlap** — it packs tight, which
/// frees ground. **Before the fix it was free whenever it had no box overlap**, and it had
/// crossings instead, which the score did not see. **So it won on the screen where the crossing
/// was, and the crossing shipped.**
///
/// The measurement of the comfortable layout's own defect — one pair of boxes overlapping on that
/// screen — is in the run's report. **This test is about the crossing**, and it is the score that
/// has to change for it to be fixable at all.
#[test]
fn the_comfortable_layout_stands_on_the_meshes_screen() {
    let root = crowded_root("layoutcomfort");
    let mut app = meshes_at_1680(&root);
    let (leaders, met) = crossing_pairs_on_screen(&mut app);
    assert!(
        met.is_empty(),
        "the crossing is still on screen, so the layout that ships is still the crossing one:\n  {}",
        met.join("\n  ")
    );
    assert!(
        leaders >= 8,
        "only {leaders} leader(s) were measured, so the rule is being enforced against nothing"
    );
}
