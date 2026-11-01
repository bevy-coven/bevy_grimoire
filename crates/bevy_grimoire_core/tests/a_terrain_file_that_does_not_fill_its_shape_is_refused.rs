//! **A terrain file is read through one validating reader** (#187): heights that do not fill
//! `width × depth`, a side under two cells, or a size or height that is not a finite number would
//! index past the end of `heights` (or underflow `width - 1`) in every reader. `Heightfield::parse`
//! refuses them and names the shape, as `Relations::parse` and `Standards::parse` refuse theirs.

use bevy_grimoire_core::heightfield::{Heightfield, TERRAIN_VERSION};

fn field(origin: (f32, f32), cell_m: f32, width: usize, depth: usize, heights: Vec<f32>) -> String {
    ron::to_string(&Heightfield { version: TERRAIN_VERSION, origin, cell_m, width, depth, heights }).unwrap()
}

#[test]
fn heights_that_do_not_fill_the_shape_are_refused_with_the_shape() {
    let e = Heightfield::parse("(origin: (0.0, 0.0), cell_m: 1.0, width: 4, depth: 4, heights: [0.0, 0.0, 0.0])")
        .expect_err("3 heights cannot fill 4x4");
    assert!(e.contains("3 heights for 4x4"), "{e}");
    let e = Heightfield::parse(&field((0.0, 0.0), 1.0, 2, 2, vec![0.0; 5])).expect_err("5 heights overfill 2x2");
    assert!(e.contains("5 heights for 2x2"), "{e}");
}

#[test]
fn a_side_under_two_cells_is_refused() {
    for (w, d) in [(0, 4), (4, 0), (0, 0), (1, 4), (4, 1)] {
        let e = Heightfield::parse(&field((0.0, 0.0), 1.0, w, d, vec![0.0; w * d]))
            .expect_err(&format!("a {w}x{d} terrain has no cell to stand in"));
        assert!(e.contains(&format!("{w}x{d}")), "{e}");
    }
}

#[test]
fn a_size_or_height_that_is_not_a_finite_number_is_refused() {
    for cell_m in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let text = field((0.0, 0.0), cell_m, 2, 2, vec![0.0; 4]);
        assert!(Heightfield::parse(&text).is_err(), "cell_m {cell_m} was accepted");
    }
    for origin in [(f32::NAN, 0.0), (0.0, f32::NEG_INFINITY)] {
        let text = field(origin, 1.0, 2, 2, vec![0.0; 4]);
        assert!(Heightfield::parse(&text).is_err(), "origin {origin:?} was accepted");
    }
    let text = field((0.0, 0.0), 1.0, 2, 2, vec![0.0, f32::NAN, 0.0, 0.0]);
    let e = Heightfield::parse(&text).expect_err("a NaN height");
    assert!(e.contains("height 1"), "the refusal names the cell: {e}");
}

#[test]
fn a_well_formed_field_reads_back_as_written() {
    let mut flat = Heightfield::flat((-3.0, 2.0), 0.5, 3, 2, 1.25);
    flat.heights[4] = -0.5;
    let text = ron::ser::to_string_pretty(&flat, ron::ser::PrettyConfig::default()).unwrap();
    assert_eq!(Heightfield::parse(&text), Ok(flat));
}

#[test]
fn text_that_is_not_a_heightfield_is_refused() {
    let e = Heightfield::parse("(origin: (0.0, 0.0))").expect_err("missing fields");
    assert!(e.starts_with("terrain:"), "{e}");
}
