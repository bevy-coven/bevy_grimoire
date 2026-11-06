//! **A button that carries a word is sized like the buttons that carry a word** —
//! `feedback-1793927698061`, relation: *invariance of the size class*.
//!
//! The note, from the editor at 2560x1604: *"Why is the fill button larger here? It looks out of place."*
//!
//! # The relation, and why this one
//!
//! A picture has no expected output, so the oracle is a relation between two things that both exist in one
//! run. Segura, Fraser, Sánchez & Ruiz-Cortés, *A Survey on Metamorphic Testing*
//! (`10.1109/TSE.2016.2532875`), find that the effective relations are those "strongly inspired by the
//! semantics of the program under test". The semantics here are the bar's own: **a button's width says
//! which class it belongs to.** `toolbar.rs` states that class out loud — `label_the_primary_group`
//! gives Place, Move, Erase, Clone, Run and Fill their names "because a 16 px picture is a guess until it
//! has been learned", and leaves "the rest of the bar (undo, save, view)" as pictures.
//!
//! So the relation is **membership**: two buttons in the same class are the same width, and the two
//! classes are distinguishable. A labelled button among pictures, or a picture among words, contradicts
//! the declaration — and it is exactly what a person notices as "out of place".
//!
//! The survey's other finding is why this is measured rather than read off the code: a relation close to
//! the implementation ("the labelled set contains Fill") **passes on the defect** — the set does contain
//! Fill, and that is deliberate. What is wrong is where the button *sits*, and only the laid-out widths
//! can say that.
//!
//! # What the capture shows
//!
//! Measured from the recorded frame (physical px, `scale_factor` 2):
//!
//! | button | width |
//! |---|---|
//! | Place / Move / Erase / Clone / Run (labelled) | 91–123 |
//! | undo, redo, save, comment, sparkle, focus, dot (pictures) | **65–66** |
//! | **Fill** | **107** |
//!
//! Fill is 63% wider than every neighbour in the run it sits in.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;
use bevy_cauldron::toolbar::Toolbar;
use bevy_gooey::Role;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window {
            resolution: bevy::window::WindowResolution::new(1280, 720),
            ..default()
        },
        bevy::window::PrimaryWindow,
    ));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// One button in the bar, and how wide the layout made it.
struct Button {
    entity: Entity,
    /// The button's own word, or its tooltip's when it carries no word — what a person calls it.
    name: String,
    /// Does it carry a word under its icon?
    labelled: bool,
    /// Its laid-out width in logical px.
    width: f32,
}

/// Every control in the toolbar, in the bar's own left-to-right order.
fn buttons(app: &mut App) -> Vec<Button> {
    let world = app.world_mut();
    let bar = {
        let mut q = world.query_filtered::<Entity, With<Toolbar>>();
        q.single(world).expect("the editor has one toolbar")
    };
    // Depth-first, in child order: the bar's own order is the order a person reads it in.
    let mut order: Vec<Entity> = Vec::new();
    let mut stack: Vec<Entity> = world
        .get::<Children>(bar)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    stack.reverse();
    while let Some(entity) = stack.pop() {
        order.push(entity);
        if let Some(kids) = world.get::<Children>(entity) {
            let mut kids: Vec<Entity> = kids.iter().collect();
            kids.reverse();
            stack.extend(kids);
        }
    }

    let mut out = Vec::new();
    for entity in order {
        let Some(node) = world.get::<ComputedNode>(entity).copied() else {
            continue;
        };
        // A button or a group option: the two things in this bar a person presses.
        let is_control = world
            .get::<Role>(entity)
            .is_some_and(|r| matches!(*r, Role::Button | Role::GroupOption));
        if !is_control || node.size().x <= 0.0 {
            continue;
        }
        // **Labelled by its own `Text` child**, which is what `label_the_primary_group` adds: the
        // control holds a picture and (when it has one) a word.
        let (labelled, word) = world
            .get::<Children>(entity)
            .map(|kids| {
                let mut found = (false, String::new());
                for kid in kids.iter() {
                    if let Some(text) = world.get::<Text>(kid) {
                        found = (true, text.0.clone());
                        break;
                    }
                }
                found
            })
            .unwrap_or((false, String::new()));
        // **The name to print**: the word when it has one, else the tooltip's line, which is how the
        // capture identifies the same button ("Tooltip ... / Button").
        let name = if labelled {
            word
        } else {
            world
                .get::<Children>(entity)
                .and_then(|kids| {
                    kids.iter().find_map(|kid| {
                        let mut at = kid;
                        for _ in 0..4 {
                            if let Some(tip) = world.get::<bevy_gooey::Tooltip>(at) {
                                return Some(tip.text.clone());
                            }
                            at = world.get::<ChildOf>(at).map(|p| p.parent())?;
                        }
                        None
                    })
                })
                .unwrap_or_else(|| "?".to_owned())
        };
        out.push(Button {
            entity,
            name,
            labelled,
            width: node.size().x * node.inverse_scale_factor(),
        });
    }
    out
}

/// **The bar has two size classes, and every button's width says which one it is in.**
///
/// A button carrying a word must be wider than the pictures — that is the class's whole point — and no two
/// buttons of one class may differ by more than a word's length. The capture's Fill is a labelled button
/// sitting in the run of pictures, so it is wider than every neighbour while claiming to be one of them.
#[test]
fn a_labelled_button_is_never_the_odd_one_in_a_run_of_pictures() {
    let mut app = editor("capture_1793927698061");
    let found = buttons(&mut app);
    assert!(
        found.len() >= 8,
        "the toolbar draws its buttons; finding {} means this test is not looking at the bar",
        found.len()
    );

    let labelled: Vec<&Button> = found.iter().filter(|b| b.labelled).collect();
    let pictures: Vec<&Button> = found.iter().filter(|b| !b.labelled).collect();
    assert!(
        !labelled.is_empty() && !pictures.is_empty(),
        "the bar has both classes (words for the tools, pictures for the rest): {} labelled, {} pictures",
        labelled.len(),
        pictures.len()
    );

    let widest_picture = pictures
        .iter()
        .map(|b| b.width)
        .fold(f32::MIN, f32::max);
    let mut offenders = Vec::new();

    // **A labelled button is a class apart**: it must be wider than the widest picture, or the word under
    // it is a word nobody can read. Fill at 107 physical px (53.5 logical) against pictures at 33 logical
    // is well clear — so this does not fire on the defect, and that is correct: the width is right for its
    // class. What is wrong is *where it stands*, which the next assertion owns.
    for button in &labelled {
        if button.width <= widest_picture {
            offenders.push(format!(
                "`{}` carries a word but is {:.1} logical px wide, no wider than the widest picture \
                 ({widest_picture:.1}): its label does not fit its class",
                button.name, button.width
            ));
        }
    }

    // **The defect.** The bar's semantics are that the labelled controls stand together (gooey's toggle
    // group) and the pictures follow as one run. A labelled button *inside* the pictures' run is the one
    // that looks out of place — and the way to see it in the layout is that a labelled button's immediate
    // neighbours, on the side away from the labelled group, are pictures narrower than itself.
    //
    // Stated as a run rule rather than an index rule so it survives the bar being reordered: the picture
    // buttons between the first and last picture, taken as a run, must not be interrupted by a labelled
    // one.
    let first_picture = found.iter().position(|b| !b.labelled);
    if let Some(first) = first_picture {
        let tail = &found[first..];
        let interrupted: Vec<&Button> = tail.iter().filter(|b| b.labelled).collect();
        for button in interrupted {
            offenders.push(format!(
                "`{}` carries a word and stands at {:.1} logical px among pictures of {:.1} px: it is the \
                 odd one in the run, which is what \"It looks out of place\" reports",
                button.name, button.width, widest_picture
            ));
            let _ = button.entity;
        }
    }

    let measured: Vec<String> = found
        .iter()
        .map(|b| {
            format!(
                "  {:<12} {:>6.1} logical px  {}",
                b.name,
                b.width,
                if b.labelled { "(word)" } else { "(picture)" }
            )
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "the toolbar's two size classes are mixed — the note says \"Why is the fill button larger here? \
         It looks out of place.\"\n\n{}\n\nevery control in the bar:\n{}",
        offenders.join("\n"),
        measured.join("\n")
    );
}
