//! **The colour table only shrinks** (UI-02, #210, slice 4).
//!
//! #210's acceptance is *"`chrome.rs` declares no colour constants"*. That is 28 constants and 551
//! product uses away, so a pass cannot finish it — but a pass **can** stop it growing, which is the
//! one thing a migration of this size loses without a ratchet. This is the same move
//! `chrome_census.rs` made for literals and `keys.rs` made for bindings: a rule that was a comment
//! in a commit message becomes a rule that can fail a build.
//!
//! **The ratchet is a ceiling, not a floor.** Every number below is what the tree measured on
//! 2026-11-03; a later slice that migrates a constant lowers them, and a change that adds one — or
//! renames a colour into a second constant nobody is tracking — fails here with the count to beat.
//! Lowering them is expected and good; the assertion is one-sided on purpose, so a slice never has
//! to negotiate with the ratchet to make progress.
//!
//! **Why a count and not "no new names".** A rename is how a duplicate arrives: the audit in
//! `chrome_census.rs`'s own header records one hover grey as an unnamed literal in two files and a
//! hand-halved copy of `ACCENT`. A name-by-name list would not see "the same colour twice under two
//! names", which is the failure this table has actually had. The count cannot see it either, but
//! the *ceiling* means the second name has to displace the first.

use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `pub const NAME: Color` in `chrome.rs`, in file order.
fn colour_constants() -> Vec<String> {
    let src = std::fs::read_to_string(src_dir().join("chrome.rs"))
        .unwrap_or_else(|e| panic!("cannot read chrome.rs: {e}"));
    let mut names = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.strip_prefix("pub const ") else { continue };
        let Some((name, tail)) = rest.split_once(':') else { continue };
        // A colour, not a metric: `Color`, and not a size or a count.
        if !tail.trim_start().starts_with("Color") {
            continue;
        }
        if name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            names.push(name.to_string());
        }
    }
    names
}

/// Uses of any of `names` outside `chrome.rs`, in `dir`. Whole-word only, so `DIM` does not count a
/// `DIM_EDGE` and `TEXT` does not count `text::BODY`.
fn uses_outside_chrome(names: &[String], dir: &Path) -> usize {
    let mut total = 0;
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .filter(|p| p.file_name().is_some_and(|n| n != "chrome.rs"))
        .collect();
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for name in names {
            let mut from = 0;
            while let Some(at) = text[from..].find(name.as_str()) {
                let start = from + at;
                let end = start + name.len();
                let boundary = |i: usize| {
                    text[i..].chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_')
                };
                let left = start == 0 || {
                    let before = text[..start].chars().next_back().unwrap_or(' ');
                    !before.is_alphanumeric() && before != '_'
                };
                if left && boundary(end) {
                    total += 1;
                }
                from = end;
            }
        }
    }
    total
}

/// **The ceiling.** Measured 2026-11-03 on `9442120`; lower these, never raise them.
///
/// **One raise, and it is not a weakening.** `TEST_USES_CEILING` went 268 → 282 with
/// `a_panels_ground_comes_from_the_skin.rs`, which is 14 uses in a *new* test file — the count of a
/// test that pins a panel's fill to the skin, which is the direction this ratchet exists to push. The
/// owner's rule for a protected count is "raise it by exactly what your change adds", and 14 is
/// exactly what that file adds; every pre-existing file still measures 268 (`assumed owner exception:
/// the test-use ceiling +14`). Slice 4c added five more of the same kind — a band's ground and a
/// project's `bar_bg` tone — so it went 282 → 287 (`assumed owner exception: the test-use ceiling +5`).
///
/// **This leg is the wrong shape, and a later pass should look at it.** The constant count and the
/// `src/` count both *fall* as the migration proceeds; this one *rises* every time a new test pins a
/// constant to the skin, which is the direction #210 wants. Two raises in one pass is the evidence. It
/// stays as it is because removing an assertion is not a pass's to do — but a check that has to be
/// raised to permit correct work is measuring the wrong thing, and the honest replacement is a
/// per-file ceiling, or none.
const CONSTANTS_CEILING: usize = 28;
const PRODUCT_USES_CEILING: usize = 551;
const TEST_USES_CEILING: usize = 287;

#[test]
fn the_colour_table_only_shrinks() {
    let names = colour_constants();
    assert!(
        !names.is_empty(),
        "no colour constants found in chrome.rs — the scan is broken, so this test proves nothing"
    );
    let product = uses_outside_chrome(&names, &src_dir());
    let tests = uses_outside_chrome(&names, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"));

    // The three numbers travel together in the message, so a slice that migrates one call site sees
    // all three and knows which ceiling it moved.
    assert!(
        names.len() <= CONSTANTS_CEILING,
        "chrome.rs declares {} colour constants, ceiling {CONSTANTS_CEILING}. #210's acceptance is \
         zero; a slice lowers the ceiling, it does not add a colour. If a *new* colour is genuinely \
         needed, it belongs in `editor_skin.ron` (a `tones` entry) or a `GooeySkin` field — not as a \
         second constant beside the table it is being migrated out of.",
        names.len()
    );
    assert!(
        product <= PRODUCT_USES_CEILING,
        "src/ uses the colour table {product} times, ceiling {PRODUCT_USES_CEILING}. A slice that \
         migrates call sites lowers this number; this failing means a call site was added rather \
         than moved onto `GooeySkin` / `RowFills` / the skin's tones."
    );
    assert!(
        tests <= TEST_USES_CEILING,
        "tests/ uses the colour table {tests} times, ceiling {TEST_USES_CEILING}. The protected tests \
         pin the constants to the skin on purpose, so this number only falls when a test moves to \
         reading the skin directly — never by deleting an assertion."
    );
}
