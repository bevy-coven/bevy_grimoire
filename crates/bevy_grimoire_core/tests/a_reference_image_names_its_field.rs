//! **A reference image that is not the picture its name says is refused naming the file and the header
//! field** (#200, H3, kind `reference_images`).
//!
//! Nothing here decodes a picture, so a file cut off by a full disk, saved over by a text or renamed from
//! another format looked fine until an agent was sent to read it. `image_header::read` opens the
//! container's own header and says which field is wrong. The three bad files of the issue's first
//! acceptance are, for every format: **corrupt** (not a picture at all: `signature`), **the wrong type**
//! (a picture of another format than its name: `format`) and **missing a field** (a header that ends
//! before the width or the height).

use bevy_grimoire_core::image_header::{self, Format, Header};

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b.extend([8, 6, 0, 0, 0]);
    b
}

fn gif(w: u16, h: u16) -> Vec<u8> {
    let mut b = b"GIF89a".to_vec();
    b.extend(w.to_le_bytes());
    b.extend(h.to_le_bytes());
    b.extend([0, 0, 0]);
    b
}

/// A JFIF header (`APP0`), then a baseline frame header, then the start of a scan.
fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0];
    b.extend([0xFF, 0xC0, 0, 17, 8]);
    b.extend(h.to_be_bytes());
    b.extend(w.to_be_bytes());
    b.extend([3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    b.extend([0xFF, 0xDA, 0, 12]);
    b
}

fn riff(chunk: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = b"RIFF".to_vec();
    b.extend((4 + 8 + payload.len() as u32).to_le_bytes());
    b.extend(b"WEBP");
    b.extend(chunk);
    b.extend((payload.len() as u32).to_le_bytes());
    b.extend(payload);
    b
}

fn webp_extended(w: u32, h: u32) -> Vec<u8> {
    let mut p = vec![0, 0, 0, 0];
    p.extend(&(w - 1).to_le_bytes()[..3]);
    p.extend(&(h - 1).to_le_bytes()[..3]);
    riff(b"VP8X", &p)
}

fn webp_lossless(w: u32, h: u32) -> Vec<u8> {
    let mut p = vec![0x2F];
    p.extend(((w - 1) | ((h - 1) << 14)).to_le_bytes());
    riff(b"VP8L", &p)
}

fn webp_lossy(w: u16, h: u16) -> Vec<u8> {
    let mut p = vec![0x10, 0x02, 0x00, 0x9D, 0x01, 0x2A];
    p.extend(w.to_le_bytes());
    p.extend(h.to_le_bytes());
    riff(b"VP8 ", &p)
}

fn read(file: &str, bytes: &[u8]) -> Result<Header, bevy_grimoire_core::reload_check::Refusal> {
    image_header::read(file, bytes)
}

#[test]
fn every_format_reads_to_its_size() {
    let cases: [(&str, Vec<u8>, Format, u32, u32); 7] = [
        ("references/court.png", png(640, 480), Format::Png, 640, 480),
        ("art/hall.jpg", jpeg(1024, 768), Format::Jpeg, 1024, 768),
        ("art/hall.JPEG", jpeg(3, 2), Format::Jpeg, 3, 2),
        ("references/spin.gif", gif(32, 16), Format::Gif, 32, 16),
        ("references/x.webp", webp_extended(4000, 3000), Format::Webp, 4000, 3000),
        ("references/l.webp", webp_lossless(256, 128), Format::Webp, 256, 128),
        ("references/y.webp", webp_lossy(100, 50), Format::Webp, 100, 50),
    ];
    for (file, bytes, format, width, height) in cases {
        assert_eq!(read(file, &bytes), Ok(Header { format, width, height }), "{file}");
    }
}

/// The three bad files for each format, and the header field the refusal must name.
#[test]
fn a_corrupt_a_wrong_type_and_a_cut_off_file_each_name_the_file_and_the_field() {
    let jpeg_bytes = jpeg(640, 480);
    let sof = jpeg_bytes.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    // (the file, its bad bytes, what is wrong, the field the verdict names)
    let cases: Vec<(&str, Vec<u8>, &str, &str)> = vec![
        ("references/court.png", b"court picture".to_vec(), "corrupt", "signature"),
        ("references/court.png", gif(4, 4), "wrong type: a gif", "format"),
        ("references/court.png", png(640, 480)[..18].to_vec(), "cut off before the width ends", "width"),
        ("references/court.png", png(640, 480)[..22].to_vec(), "cut off inside the height", "height"),
        ("art/hall.jpg", b"hall picture".to_vec(), "corrupt", "signature"),
        ("art/hall.jpg", png(2, 2), "wrong type: a png", "format"),
        ("art/hall.jpg", jpeg_bytes[..sof + 6].to_vec(), "cut off inside the height", "height"),
        ("art/hall.jpg", jpeg_bytes[..sof + 8].to_vec(), "cut off inside the width", "width"),
        ("art/hall.jpg", vec![0xFF, 0xD8, 0xFF, 0xDA, 0, 2], "a scan with no frame header", "width"),
        ("references/spin.gif", b"spin picture".to_vec(), "corrupt", "signature"),
        ("references/spin.gif", webp_lossy(8, 8), "wrong type: a webp", "format"),
        ("references/spin.gif", gif(8, 8)[..7].to_vec(), "cut off inside the width", "width"),
        ("references/spin.gif", gif(8, 8)[..9].to_vec(), "cut off inside the height", "height"),
        ("references/x.webp", b"webp picture".to_vec(), "corrupt", "signature"),
        ("references/x.webp", jpeg(8, 8), "wrong type: a jpeg", "format"),
        ("references/x.webp", webp_extended(8, 8)[..26].to_vec(), "cut off inside the width", "width"),
        ("references/x.webp", webp_extended(8, 8)[..29].to_vec(), "cut off inside the height", "height"),
        ("references/l.webp", webp_lossless(8, 8)[..24].to_vec(), "cut off inside the size", "width"),
        ("references/y.webp", webp_lossy(8, 8)[..27].to_vec(), "cut off inside the width", "width"),
        ("references/y.webp", webp_lossy(8, 8)[..29].to_vec(), "cut off inside the height", "height"),
    ];
    for (file, bytes, what, field) in cases {
        let refusal = read(file, &bytes).expect_err(&format!("{file}, {what}: must be refused"));
        assert_eq!(refusal.kind, "reference_images", "{file}, {what}");
        assert_eq!(refusal.file, file, "{file}, {what}");
        assert_eq!(refusal.field.as_deref(), Some(field), "{file}, {what}: {}", refusal.reason);
        assert!(!refusal.reason.is_empty(), "{file}, {what}: the refusal says why");
        assert!(refusal.sentence().starts_with(&format!("{file} ({field}): ")), "{file}, {what}: {}", refusal.sentence());
    }
}

/// A header that reads and says a picture has no pixels is not a picture.
#[test]
fn a_size_of_zero_is_refused_at_the_field() {
    let width = read("a.png", &png(0, 5)).unwrap_err();
    assert_eq!((width.field.as_deref(), width.reason.as_str()), (Some("width"), "the width is 0 pixels"));
    let height = read("a.gif", &gif(3, 0)).unwrap_err();
    assert_eq!((height.field.as_deref(), height.reason.as_str()), (Some("height"), "the height is 0 pixels"));
}

/// The signature is judged before the extension is compared and an empty file says so; a name that is not
/// an image name is refused before the bytes are looked at.
#[test]
fn an_empty_file_and_a_name_that_is_not_a_picture_say_so() {
    let empty = read("references/court.png", &[]).unwrap_err();
    assert_eq!((empty.field.as_deref(), empty.reason.as_str()), (Some("signature"), "the file is empty"));
    let text = read("references/notes.txt", &png(2, 2)).unwrap_err();
    assert_eq!(text.field.as_deref(), Some("extension"), "{}", text.reason);
    let none = read("references/court", &png(2, 2)).unwrap_err();
    assert_eq!(none.field.as_deref(), Some("extension"), "{}", none.reason);
}

/// A corrupt file says what it starts with, in text when it is text and in hex when it is not.
#[test]
fn a_corrupt_file_says_how_it_starts() {
    let text = read("a.png", b"court picture").unwrap_err();
    assert!(text.reason.contains("\"court pi\""), "{}", text.reason);
    let bytes = read("a.png", &[0x00, 0x01, 0xFE]).unwrap_err();
    assert!(bytes.reason.contains("00 01 fe"), "{}", bytes.reason);
}
