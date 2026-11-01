//! The close-up an after-edit check is shown is cut from the editor's picture around where the agent put
//! the piece, and is always a full-size frame inside that picture (#111).

use bevy_cauldron::after_edit::{CLOSEUP_FRACTION, closeup_rect, crop_data_url, edit_at, wants_picture};

#[test]
fn a_piece_in_the_middle_is_framed_around_itself() {
    let [x, y, w, h] = closeup_rect((0.5, 0.5), (1000, 800));
    assert_eq!((w, h), (320, 320), "{CLOSEUP_FRACTION} of the shorter side");
    assert_eq!((x, y), (340, 240));
}

#[test]
fn a_piece_at_the_edge_gets_a_full_frame_slid_inside_the_picture() {
    for centre in [(0.0, 0.0), (1.0, 1.0), (-3.0, 9.0), (0.99, 0.01)] {
        let [x, y, w, h] = closeup_rect(centre, (1000, 800));
        assert_eq!((w, h), (320, 320), "{centre:?} keeps the frame's size");
        assert!(x + w <= 1000 && y + h <= 800, "{centre:?} inside the picture: {x} {y}");
    }
    assert_eq!(closeup_rect((1.0, 1.0), (1000, 800)), [680, 480, 320, 320]);
}

#[test]
fn a_tiny_picture_still_gets_a_frame_inside_it() {
    let [x, y, w, h] = closeup_rect((0.5, 0.5), (3, 2));
    assert!(w >= 1 && h >= 1 && x + w <= 3 && y + h <= 2);
}

#[test]
fn the_place_is_the_operations_own_at_and_nothing_else() {
    assert_eq!(edit_at(&serde_json::json!({"piece": "barrel", "at": [3.5, -2.0]})), Some((3.5, -2.0)));
    assert_eq!(edit_at(&serde_json::json!({"piece": "barrel"})), None);
    assert_eq!(edit_at(&serde_json::json!({"at": [1.0]})), None);
}

#[test]
fn the_crop_is_the_named_pixels() {
    let mut img = image::RgbaImage::new(40, 20);
    for (x, _, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([x as u8, 0, 0, 255]);
    }
    let url = crop_data_url(&image::DynamicImage::ImageRgba8(img), [10, 5, 8, 8]).unwrap();
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(url.strip_prefix("data:image/png;base64,").unwrap()).unwrap();
    let crop = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(crop.dimensions(), (8, 8));
    assert_eq!(crop.get_pixel(0, 0).0[0], 10);
    assert_eq!(crop.get_pixel(7, 7).0[0], 17);
}

#[test]
fn the_built_in_checks_want_a_picture() {
    assert!(wants_picture(&bevy_oracle::Checks::builtin()));
}
