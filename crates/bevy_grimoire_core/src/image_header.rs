//! **The header of a reference image, read well enough to say whether the file is the picture its name
//! claims** (#200, H3, kind `reference_images`).
//!
//! A reference (`crate::references`) is a project-relative picture an agent is told to look at. Nothing
//! in this repository decodes it, so a file that was cut off by a full disk, saved over by a text, or
//! renamed from another format used to look fine until an agent was sent to read it. [`read`] opens
//! the container's own header (a PNG's `IHDR`, a JPEG's frame header, a GIF's screen descriptor, a
//! WebP's `VP8 `, `VP8L` or `VP8X` chunk) and answers with the format and the size, or refuses naming
//! **the file and the header field** it is wrong at:
//!
//! |field|what is wrong|
//! |---|---|
//! |`extension`|the name does not end in `png`, `jpg`, `jpeg`, `webp` or `gif`|
//! |`signature`|the file is empty, or does not start as any of the four formats|
//! |`format`|it starts as one format and its name says another (a `.png` that is a gif)|
//! |`header`|the format's first chunk or marker is not the one that carries the size|
//! |`width`, `height`|the header ends before the field, or the field is 0|
//!
//! The pixels are not read: the reader needs the first few hundred bytes of a file, or for a JPEG with a
//! large profile in front of its frame header, the first megabyte ([`HEAD`]). It is a hand-rolled
//! reader like `crate::glb`'s, so this crate takes no image dependency.

use crate::reload_check::Refusal;

/// The kind a refusal names.
const KIND: &str = "reference_images";

/// **How much of a file [`read`] needs**: a JPEG's frame header can sit behind a large profile, and a
/// reader that was given less than this refuses a picture it could have read.
pub const HEAD: usize = 1 << 20;

/// The picture formats a reference may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl Format {
    /// The name a refusal uses.
    pub fn word(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpeg",
            Format::Gif => "gif",
            Format::Webp => "webp",
        }
    }

    fn of_extension(ext: &str) -> Option<Format> {
        match ext {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" => Some(Format::Jpeg),
            "gif" => Some(Format::Gif),
            "webp" => Some(Format::Webp),
            _ => None,
        }
    }

    /// The format a file's first bytes say it is.
    fn sniff(bytes: &[u8]) -> Option<Format> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Some(Format::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Format::Jpeg)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some(Format::Gif)
        } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Format::Webp)
        } else {
            None
        }
    }
}

/// **What a picture's header says**: its format and size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub format: Format,
    pub width: u32,
    pub height: u32,
}

/// A header field the file does not reach, or reaches wrong: the field and why.
type Fault = (&'static str, String);

fn short(field: &'static str, what: &str) -> Fault {
    (field, format!("the file ends inside the header, before the {what}"))
}

/// The first bytes of a file as an author would read them: text when they are text, hex when not.
fn start_of(bytes: &[u8]) -> String {
    let head = &bytes[..bytes.len().min(8)];
    if head.iter().all(|b| (0x20..0x7F).contains(b)) {
        format!("\"{}\"", String::from_utf8_lossy(head))
    } else {
        head.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
    }
}

/// **Read a picture's header.** `file` is the path the author sees (its extension is the format the name
/// claims); `bytes` is the start of the file, [`HEAD`] of it at most.
pub fn read(file: &str, bytes: &[u8]) -> Result<Header, Refusal> {
    let refuse = |(field, reason): Fault| Refusal { kind: KIND, file: file.to_owned(), field: Some(field.to_owned()), reason };
    let name = file.rsplit('/').next().unwrap_or(file);
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let Some(named) = Format::of_extension(&ext) else {
        return Err(refuse(("extension", format!("`{name}` does not end in png, jpg, jpeg, webp or gif"))));
    };
    if bytes.is_empty() {
        return Err(refuse(("signature", "the file is empty".to_owned())));
    }
    let Some(found) = Format::sniff(bytes) else {
        return Err(refuse(("signature", format!("the file starts {}, which is no png, jpeg, gif or webp picture", start_of(bytes)))));
    };
    if found != named {
        return Err(refuse(("format", format!("the file is a {} picture and its name says {}", found.word(), named.word()))));
    }
    let (width, height) = match found {
        Format::Png => png(bytes),
        Format::Jpeg => jpeg(bytes),
        Format::Gif => gif(bytes),
        Format::Webp => webp(bytes),
    }
    .map_err(refuse)?;
    for (field, size) in [("width", width), ("height", height)] {
        if size == 0 {
            return Err(refuse((field, format!("the {field} is 0 pixels"))));
        }
    }
    Ok(Header { format: found, width, height })
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn le16(b: &[u8]) -> u32 {
    u32::from(u16::from_le_bytes([b[0], b[1]]))
}

/// The signature, then the first chunk: length, `IHDR`, width, height.
fn png(b: &[u8]) -> Result<(u32, u32), Fault> {
    if b.len() < 16 {
        return Err(short("width", "width"));
    }
    if &b[12..16] != b"IHDR" {
        return Err(("header", format!("the first chunk is `{}`, not IHDR", String::from_utf8_lossy(&b[12..16]))));
    }
    if b.len() < 20 {
        return Err(short("width", "width"));
    }
    if b.len() < 24 {
        return Err(short("height", "height"));
    }
    Ok((be32(&b[16..20]), be32(&b[20..24])))
}

/// The signature, then the logical screen: width and height as little-endian 16 bits.
fn gif(b: &[u8]) -> Result<(u32, u32), Fault> {
    if b.len() < 8 {
        return Err(short("width", "width"));
    }
    if b.len() < 10 {
        return Err(short("height", "height"));
    }
    Ok((le16(&b[6..8]), le16(&b[8..10])))
}

/// Walk the segments to the first frame header (`SOF0` to `SOF15`, bar the three that are not frames):
/// precision, then height, then width, big-endian.
fn jpeg(b: &[u8]) -> Result<(u32, u32), Fault> {
    let no_frame = || short("width", "frame header that names the width");
    let mut i = 2;
    loop {
        if i >= b.len() {
            return Err(no_frame());
        }
        if b[i] != 0xFF {
            return Err(("header", format!("byte {i} is {:02x}, where a marker starts with ff", b[i])));
        }
        while i < b.len() && b[i] == 0xFF {
            i += 1;
        }
        let Some(&marker) = b.get(i) else { return Err(no_frame()) };
        i += 1;
        match marker {
            0x01 | 0xD0..=0xD8 => continue,
            // End of image, or the scan: the picture's data began and no frame header named its size.
            0xD9 | 0xDA => return Err(("width", "a scan begins before any frame header names the width".to_owned())),
            _ => {}
        }
        if i + 2 > b.len() {
            return Err(no_frame());
        }
        let length = usize::from(u16::from_be_bytes([b[i], b[i + 1]]));
        if length < 2 {
            return Err(("header", format!("the segment at byte {} is {length} bytes long, which cannot hold its own length", i - 1)));
        }
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            // Length (2), precision (1), height (2), width (2).
            if b.len() < i + 5 {
                return Err(short("height", "height"));
            }
            if b.len() < i + 7 {
                return Err(short("width", "width"));
            }
            let height = u32::from(u16::from_be_bytes([b[i + 3], b[i + 4]]));
            let width = u32::from(u16::from_be_bytes([b[i + 5], b[i + 6]]));
            return Ok((width, height));
        }
        i += length;
    }
}

/// `RIFF`, a size, `WEBP`, then the first chunk, which carries the size in one of three ways.
fn webp(b: &[u8]) -> Result<(u32, u32), Fault> {
    if b.len() < 16 {
        return Err(short("width", "width"));
    }
    match &b[12..16] {
        // Lossy: a frame tag, the start code 9d 01 2a, then width and height in 14 bits each.
        b"VP8 " => {
            if b.len() >= 26 && b[23..26] != [0x9D, 0x01, 0x2A] {
                return Err(("header", "the lossy frame does not start with the code 9d 01 2a".to_owned()));
            }
            if b.len() < 28 {
                return Err(short("width", "width"));
            }
            if b.len() < 30 {
                return Err(short("height", "height"));
            }
            Ok((le16(&b[26..28]) & 0x3FFF, le16(&b[28..30]) & 0x3FFF))
        }
        // Lossless: the byte 2f, then width - 1 and height - 1 in 14 bits each.
        b"VP8L" => {
            if b.len() >= 21 && b[20] != 0x2F {
                return Err(("header", format!("the lossless stream starts with {:02x}, not 2f", b[20])));
            }
            if b.len() < 25 {
                return Err(short("width", "width"));
            }
            let bits = u32::from_le_bytes([b[21], b[22], b[23], b[24]]);
            Ok((1 + (bits & 0x3FFF), 1 + ((bits >> 14) & 0x3FFF)))
        }
        // Extended: flags and three reserved bytes, then width - 1 and height - 1 in 24 bits each.
        b"VP8X" => {
            if b.len() < 27 {
                return Err(short("width", "width"));
            }
            if b.len() < 30 {
                return Err(short("height", "height"));
            }
            let le24 = |s: &[u8]| u32::from(s[0]) | u32::from(s[1]) << 8 | u32::from(s[2]) << 16;
            Ok((1 + le24(&b[24..27]), 1 + le24(&b[27..30])))
        }
        other => Err(("header", format!("the first chunk is `{}`, not VP8, VP8L or VP8X", String::from_utf8_lossy(other)))),
    }
}
