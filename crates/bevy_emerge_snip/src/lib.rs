//! **Cutting one rectangle out of a picture of the editor.**
//!
//! **The rectangle is in window pixels**, the same pixels the still is in, with the origin at the
//! top left and `x1`, `y1` exclusive. **A rectangle that is empty, reversed, negative or outside
//! the picture is refused**, not clamped: a clamped crop is a picture of a different place than the
//! agent asked for, and nothing in the answer would say so.

use image::{ImageFormat, ImageReader, RgbaImage};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// **A rectangle in window pixels: `[x0, y0, x1, y1]`, `x1`/`y1` exclusive.**
pub type Rect = [i64; 4];

/// **What a crop is, and what it was cut from.**
pub struct Crop {
    /// The crop, as PNG bytes.
    pub png: Vec<u8>,
    /// The crop's size in window pixels: `[width, height]`.
    pub size: [u32; 2],
    /// The whole picture's size in window pixels: `[width, height]`.
    pub picture: [u32; 2],
}

/// **Cut `rect` out of a PNG.**
///
/// **Refuses** with a sentence naming the rectangle and the picture's size, so the agent can fix the
/// call rather than guess.
pub fn crop_png(png: &[u8], rect: Rect) -> Result<Crop, String> {
    let picture = ImageReader::with_format(Cursor::new(png), ImageFormat::Png)
        .decode()
        .map_err(|e| format!("the editor's picture did not decode as a PNG: {e}"))?
        .into_rgba8();
    let (w, h) = picture.dimensions();
    let crop = cut(&picture, rect)?;
    let (cw, ch) = crop.dimensions();
    let mut out = Cursor::new(Vec::new());
    crop.write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("the crop did not encode as a PNG: {e}"))?;
    Ok(Crop {
        png: out.into_inner(),
        size: [cw, ch],
        picture: [w, h],
    })
}

/// **Cut a rectangle out of a decoded picture.** The bounds rule lives here and nowhere else, so a
/// snip and an agent's crop refuse exactly the same rectangles.
fn cut(picture: &RgbaImage, rect: Rect) -> Result<RgbaImage, String> {
    let (w, h) = picture.dimensions();
    let [x0, y0, x1, y1] = rect;
    let inside =
        x0 >= 0 && y0 >= 0 && x0 < x1 && y0 < y1 && x1 <= i64::from(w) && y1 <= i64::from(h);
    if !inside {
        return Err(format!(
            "rect [{x0}, {y0}, {x1}, {y1}] is not a rectangle inside the picture: x0 and y0 must be \
             at least 0, x1 and y1 must be greater than them, and x1 and y1 must be at most the \
             picture size {w} x {h} window pixels."
        ));
    }
    let (cw, ch) = ((x1 - x0) as u32, (y1 - y0) as u32);
    Ok(image::imageops::crop_imm(picture, x0 as u32, y0 as u32, cw, ch).to_image())
}

/// **One snip, as the file beside its crop.** An agent answers "where was the person and what was
/// in the area" from this file alone, without the editor running.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SnipRecord {
    /// The name both files share, `snip-<unix millis>`. Set by [`write_snip`].
    pub stem: String,
    /// When the snip was saved, Unix milliseconds. The newest reads first.
    pub taken_at_ms: u64,
    /// The map that was open, or empty on a door with no map.
    pub map: String,
    /// The tool armed on the Map door, by name; empty when no tool was.
    pub active_tool: String,
    /// The one-line note the person typed. May be empty.
    pub note: String,
    /// The dragged rectangle in logical window pixels: `[x0, y0, x1, y1]`.
    pub rect_window_px: [f32; 4],
    /// The window's scale factor when the snip was taken.
    pub scale_factor: f32,
    /// The crop in picture pixels: the same pixels as the PNG.
    pub rect_picture_px: Rect,
    /// The crop's size in picture pixels: `[width, height]`.
    pub size: [u32; 2],
    /// The whole picture's size in picture pixels: `[width, height]`.
    pub picture: [u32; 2],
    /// The crop's file name, beside this file.
    pub picture_file: String,
    /// Where the map camera was looking. None on a door with no map camera.
    pub camera: Option<CameraView>,
    /// The ground point under the pointer where the drag was released, in world metres. None when that point was off the map.
    pub cursor_world: Option<[f32; 3]>,
    /// The placements whose position is inside the dragged rectangle.
    pub placements: Vec<PlacedIn>,
}

/// **Where the map camera was.**
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CameraView {
    pub translation: [f32; 3],
    pub forward: [f32; 3],
}

/// **A placement that was inside the rectangle**: its id and its world position in metres.
///
/// **A stamped row is listed too**, with its row id in `id` and the stamp it belongs to in `stamp`.
/// A stamp's rows have no placement of their own, so the stamp's id is the one an agent acts on
/// (`stamp_remove` takes it). `stamp` is absent for an ordinary placement.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlacedIn {
    pub id: String,
    pub world: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<String>,
}

/// **The two files a snip leaves.**
#[derive(Debug, Clone, PartialEq)]
pub struct SnipFiles {
    pub png: PathBuf,
    pub json: PathBuf,
}

/// **Write a snip into `dir`: the crop as `<stem>.png` and its record as `<stem>.json`.**
///
/// `picture` is the whole window as `(width, height, RGBA8 bytes)`, and `rect` is the crop in those
/// picture pixels. The record's `stem`, `picture_file`, `size`, `picture` and `rect_picture_px` are
/// set here, because only here is the crop known.
///
/// **Refuses to overwrite.** Both names are checked, then created with `create_new`, so a snip
/// never replaces an earlier one. A bad rectangle writes nothing at all.
pub fn write_snip(
    dir: &Path,
    stem: &str,
    picture: (u32, u32, Vec<u8>),
    rect: Rect,
    mut record: SnipRecord,
) -> Result<SnipFiles, String> {
    let (w, h, rgba) = picture;
    let whole = RgbaImage::from_raw(w, h, rgba).ok_or_else(|| {
        format!(
            "the picture is {w} x {h} window pixels, but its pixel data is not exactly that many."
        )
    })?;
    let crop = cut(&whole, rect)?;
    let (cw, ch) = crop.dimensions();
    let mut png = Cursor::new(Vec::new());
    crop.write_to(&mut png, ImageFormat::Png)
        .map_err(|e| format!("the crop did not encode as a PNG: {e}"))?;
    record.stem = stem.to_string();
    record.picture_file = format!("{stem}.png");
    record.size = [cw, ch];
    record.picture = [w, h];
    record.rect_picture_px = rect;
    let text = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("the snip record did not encode as JSON: {e}"))?;
    let files = SnipFiles {
        png: dir.join(format!("{stem}.png")),
        json: dir.join(format!("{stem}.json")),
    };
    for path in [&files.png, &files.json] {
        if path.exists() {
            return Err(format!(
                "{} already exists, so nothing was written and the earlier snip is kept.",
                path.display()
            ));
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("could not make {}: {e}", dir.display()))?;
    write_new(&files.png, &png.into_inner())?;
    write_new(&files.json, text.as_bytes())?;
    Ok(files)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("could not create {}: {e}", path.display()))?;
    file.write_all(bytes)
        .map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// **The name a snip carries from the moment it is confirmed until its picture is written.** A
/// file with this prefix is a record of a snip whose picture has not landed, so [`read_snips`]
/// skips it and [`read_pending`] lists it.
pub const PENDING_PREFIX: &str = "pending-";

/// **How long a pending record may wait for its picture before it reads as abandoned.** The
/// readback lands a frame or more after the confirm, so ten seconds is far past any live save.
pub const PENDING_ABANDONED_AFTER_MS: u64 = 10_000;

/// **Record a confirmed snip before its picture is taken.** The editor's readback lands a frame or
/// more after the confirm, so without this record a read in between finds nothing at all.
/// `create_new` means a record already there is refused rather than replaced.
pub fn write_pending(dir: &Path, record: &SnipRecord) -> Result<PathBuf, String> {
    let refuse = |e: String| format!("{e}; nothing was saved.");
    std::fs::create_dir_all(dir)
        .map_err(|e| refuse(format!("could not make {}: {e}", dir.display())))?;
    let path = dir.join(format!("{PENDING_PREFIX}{}.json", record.stem));
    let text = serde_json::to_string_pretty(record).map_err(|e| refuse(e.to_string()))?;
    write_new(&path, text.as_bytes()).map_err(refuse)?;
    Ok(path)
}

/// **Remove a snip's pending record once its picture has landed, or its save has failed.** A
/// record that was never removed would show a snip as being saved forever, so this runs on both
/// outcomes. A record already gone is not an error.
pub fn clear_pending(dir: &Path, stem: &str) {
    let _ = std::fs::remove_file(dir.join(format!("{PENDING_PREFIX}{stem}.json")));
}

/// **Remove every pending record in `dir`, and return how many.** Run once when a project opens: a
/// record present then was left by an editor that ended before its picture landed, so nothing is
/// still waiting on it. A folder that does not exist has none.
pub fn clear_all_pending(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(PENDING_PREFIX)
            && name.ends_with(".json")
            && std::fs::remove_file(entry.path()).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

/// **The snips confirmed but not yet written, newest first, and the pending files that could not be
/// read.** Each record's stem is the one its picture will be given. A folder that does not exist has
/// none. A file that will not parse is named in the second list rather than hiding the rest, as
/// [`read_snips`] does.
pub fn read_pending(dir: &Path) -> (Vec<SnipRecord>, Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (Vec::new(), Vec::new());
    };
    let mut records = Vec::new();
    let mut problems = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix(PENDING_PREFIX))
            .map(str::to_string)
        else {
            continue;
        };
        let read = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<SnipRecord>(&text).map_err(|e| e.to_string()));
        match read {
            Ok(mut record) => {
                record.stem = stem;
                records.push(record);
            }
            Err(e) => problems.push(format!("{} is not a snip record: {e}", path.display())),
        }
    }
    records.sort_by(|a, b| {
        b.taken_at_ms
            .cmp(&a.taken_at_ms)
            .then_with(|| b.stem.cmp(&a.stem))
    });
    (records, problems)
}

/// **Read every snip in `dir`, newest first.** A file that is not a snip record is named in the
/// second list rather than hiding the rest. A folder that does not exist has no snips.
pub fn read_snips(dir: &Path) -> (Vec<SnipRecord>, Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), Vec::new()),
        Err(e) => {
            return (
                Vec::new(),
                vec![format!("could not read {}: {e}", dir.display())],
            );
        }
    };
    let mut records = Vec::new();
    let mut problems = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let pending = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(PENDING_PREFIX));
        if pending || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // **The file's own name names the picture.** A stem written inside the record is not
        // trusted, because a stem with path separators would point the picture outside the folder.
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let read = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<SnipRecord>(&text).map_err(|e| e.to_string()));
        match read {
            Ok(mut record) => {
                let png = dir.join(format!("{stem}.png"));
                if !png.is_file() {
                    problems.push(format!(
                        "{} has no picture {}, so it is not listed",
                        path.display(),
                        png.display()
                    ));
                    continue;
                }
                record.picture_file = format!("{stem}.png");
                record.stem = stem;
                records.push(record);
            }
            Err(e) => problems.push(format!("{} is not a snip record: {e}", path.display())),
        }
    }
    records.sort_by(|a, b| {
        b.taken_at_ms
            .cmp(&a.taken_at_ms)
            .then_with(|| b.stem.cmp(&a.stem))
    });
    (records, problems)
}

#[cfg(test)]
mod written {
    use super::*;

    fn record(taken_at_ms: u64, note: &str) -> SnipRecord {
        SnipRecord {
            stem: String::new(),
            taken_at_ms,
            map: "m".into(),
            active_tool: "Place".into(),
            note: note.into(),
            rect_window_px: [0.0, 0.0, 4.0, 4.0],
            scale_factor: 1.0,
            rect_picture_px: [0, 0, 0, 0],
            size: [0, 0],
            picture: [0, 0],
            picture_file: String::new(),
            camera: None,
            cursor_world: None,
            placements: Vec::new(),
        }
    }

    /// **A 16 x 10 window where every pixel says where it is.**
    fn picture() -> (u32, u32, Vec<u8>) {
        let mut bytes = Vec::new();
        for y in 0..10u8 {
            for x in 0..16u8 {
                bytes.extend_from_slice(&[x, y, 0, 255]);
            }
        }
        (16, 10, bytes)
    }

    fn fresh_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bevy_emerge_snip_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_snip_writes_its_crop_and_its_record_and_reads_back_newest_first() {
        let dir = fresh_dir("reads_back");
        let first = write_snip(&dir, "snip-1", picture(), [4, 2, 9, 7], record(1, "old"))
            .expect("a valid rect writes");
        write_snip(&dir, "snip-2", picture(), [0, 0, 16, 10], record(2, "new"))
            .expect("the whole window writes");
        let crop = image::load_from_memory(&std::fs::read(&first.png).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(crop.dimensions(), (5, 5));
        assert_eq!(crop.get_pixel(0, 0).0, [4, 2, 0, 255]);
        let (records, problems) = read_snips(&dir);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].note, "new");
        assert_eq!(records[1].note, "old");
        assert_eq!(records[1].stem, "snip-1");
        assert_eq!(records[1].picture_file, "snip-1.png");
        assert_eq!(records[1].rect_picture_px, [4, 2, 9, 7]);
        assert_eq!(records[1].size, [5, 5]);
        assert_eq!(records[1].picture, [16, 10]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_snip_never_replaces_an_earlier_one() {
        let dir = fresh_dir("no_overwrite");
        write_snip(&dir, "snip-1", picture(), [0, 0, 4, 4], record(1, "first"))
            .expect("the first writes");
        let err = write_snip(&dir, "snip-1", picture(), [0, 0, 8, 8], record(2, "second"))
            .expect_err("a taken name is refused");
        assert!(err.contains("already exists"), "{err}");
        let (records, _) = read_snips(&dir);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].note, "first");
        assert_eq!(records[0].size, [4, 4]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rect_outside_the_picture_writes_nothing() {
        let dir = fresh_dir("bad_rect");
        let err = write_snip(&dir, "snip-1", picture(), [0, 0, 17, 4], record(1, ""))
            .expect_err("a rect past the edge is refused");
        assert!(err.contains("16 x 10"), "{err}");
        assert!(!dir.join("snip-1.png").exists());
        assert!(!dir.join("snip-1.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_that_does_not_exist_has_no_snips() {
        let (records, problems) = read_snips(&fresh_dir("never_made"));
        assert!(records.is_empty());
        assert!(problems.is_empty());
    }

    /// **A snip confirmed on Enter is listed as pending until its picture lands.** The picture is
    /// written by the editor's readback, which is a frame or more later, so a read in between must
    /// say the snip exists and is still being saved, not return an empty list.
    #[test]
    fn a_confirmed_snip_is_pending_until_its_picture_lands() {
        let dir = fresh_dir("pending");
        let mut rec = record(7, "the gate is too narrow");
        rec.stem = "snip-7".into();
        write_pending(&dir, &rec).expect("the pending record is written");

        let (records, problems) = read_snips(&dir);
        assert!(records.is_empty(), "a pending snip is not yet a snip: {records:?}");
        assert!(problems.is_empty(), "a pending snip is not a problem: {problems:?}");
        let (pending, _) = read_pending(&dir);
        assert_eq!(pending.len(), 1, "the pending snip is listed");
        assert_eq!(pending[0].stem, "snip-7");
        assert_eq!(pending[0].note, "the gate is too narrow");

        write_snip(&dir, "snip-7", picture(), [0, 0, 16, 10], rec.clone())
            .expect("the picture lands");
        clear_pending(&dir, "snip-7");
        assert!(read_pending(&dir).0.is_empty(), "the picture landed, so nothing is pending");
        let (records, _) = read_snips(&dir);
        assert_eq!(records.len(), 1, "the finished snip is listed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A pending file that will not parse is named, not dropped.** A crash mid-write leaves one
    /// of these, and before the fix it was in neither list.
    #[test]
    fn a_broken_pending_record_is_named_and_the_rest_still_list() {
        let dir = fresh_dir("broken_pending");
        let mut rec = record(8, "kept");
        rec.stem = "snip-8".into();
        write_pending(&dir, &rec).expect("the pending record is written");
        std::fs::write(dir.join("pending-snip-9.json"), "{ half a record")
            .expect("the broken record is written");
        let (pending, problems) = read_pending(&dir);
        assert_eq!(pending.len(), 1, "the readable pending snip is still listed");
        assert!(
            problems.iter().any(|p| p.contains("pending-snip-9.json")),
            "the broken pending record is named nowhere: {problems:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The startup sweep removes every pending record and keeps the finished snips.**
    #[test]
    fn the_startup_sweep_clears_pending_records_and_keeps_snips() {
        let dir = fresh_dir("sweep");
        let mut stale = record(10, "stale");
        stale.stem = "snip-10".into();
        write_pending(&dir, &stale).expect("the pending record is written");
        write_snip(&dir, "snip-11", picture(), [0, 0, 16, 10], record(11, "kept"))
            .expect("the finished snip is written");
        assert_eq!(clear_all_pending(&dir), 1, "one pending record was left");
        assert!(read_pending(&dir).0.is_empty(), "the stale record is gone");
        let (records, _) = read_snips(&dir);
        assert_eq!(records.len(), 1, "the finished snip is kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A pending record that cannot be written says that nothing was saved.** The editor shows the
    /// refusal in its status line, and a bare IO error reads as if a half-saved snip might be left.
    #[test]
    fn a_pending_record_that_cannot_be_written_says_nothing_was_saved() {
        let dir = fresh_dir("unwritable");
        std::fs::create_dir_all(&dir).expect("the folder is made");
        let blocker = dir.join("in-the-way");
        std::fs::write(&blocker, "a file, not a folder").expect("the blocker is written");
        let mut rec = record(12, "refused");
        rec.stem = "snip-12".into();
        let err = write_pending(&blocker, &rec).expect_err("a file cannot hold records");
        assert!(err.ends_with("nothing was saved."), "the refusal does not say so: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    /// **A picture where every pixel says where it is**, so a crop can be checked pixel by pixel.
    fn picture(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([x as u8, y as u8, 0, 255]);
        }
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn a_rect_inside_the_picture_is_cut_from_the_right_place() {
        let crop = crop_png(&picture(16, 10), [4, 2, 9, 7]).expect("a valid rect");
        assert_eq!(crop.size, [5, 5]);
        assert_eq!(crop.picture, [16, 10]);
        let got = image::load_from_memory(&crop.png).unwrap().to_rgba8();
        assert_eq!(got.dimensions(), (5, 5));
        // **The top left of the crop is the picture's (4, 2)**, not its origin.
        assert_eq!(got.get_pixel(0, 0).0, [4, 2, 0, 255]);
        assert_eq!(got.get_pixel(4, 4).0, [8, 6, 0, 255]);
    }

    #[test]
    fn a_rect_that_is_empty_reversed_negative_or_outside_is_refused_with_the_size() {
        let p = picture(16, 10);
        for bad in [
            [3, 3, 3, 8],  // empty in x
            [3, 8, 9, 8],  // empty in y
            [9, 2, 4, 7],  // reversed in x
            [-1, 0, 5, 5], // negative origin
            [0, 0, 17, 5], // past the right edge
            [0, 0, 5, 11], // past the bottom edge
        ] {
            let err = crop_png(&p, bad).err().expect("should refuse");
            assert!(
                err.contains("16 x 10"),
                "the refusal does not give the size: {err}"
            );
        }
    }

    #[test]
    fn a_rect_that_touches_the_far_edge_is_accepted() {
        let p = picture(16, 10);
        let whole = crop_png(&p, [0, 0, 16, 10]).expect("the whole picture is a valid rect");
        assert_eq!(whole.size, [16, 10]);
        let corner = crop_png(&p, [15, 9, 16, 10]).expect("the last pixel is a valid rect");
        assert_eq!(corner.size, [1, 1]);
    }

    #[test]
    fn bytes_that_are_not_a_png_are_refused() {
        let err = crop_png(b"not a picture", [0, 0, 1, 1])
            .err()
            .expect("should refuse");
        assert!(err.contains("did not decode"), "{err}");
    }
}
