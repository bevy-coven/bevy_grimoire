//! **The crate boundary, enforced.**
//!
//! `bevy_grimoire_core` exists so that the constraint IR, the solvers, WFC and the seeded RNG can be consumed
//! by the game, by the offline search, and by a standalone editor without any of them agreeing on a
//! renderer. That property was a *comment* for months — `ir.rs` has said "Nothing here imports
//! `bevy::`" since long before there was a crate — and a comment cannot fail a build.
//!
//! Adding `bevy` here would compile fine and nobody would notice until the editor tried to link it.
//! These two tests are the ratchet, in the spirit of `tests/determinism_lint.rs` and
//! `tests/panic_budget.rs`: cheap, GPU-free, and they fail at the door.
//!
//! Widening the dependency list is a design decision (see `docs/2026-08-03-emerge-mapper-plan.md`), so it
//! should cost an argument and a deliberate edit here — not a passing `cargo build`.
//!
//! # Which test actually catches what
//!
//! Both were verified by introducing a violation and watching them fail, and the experiment was
//! informative: writing `bevy::math::Vec2` in a source file **does not reach these tests at all** —
//! the crate simply fails to compile, because `bevy` is not a dependency. The compiler is the first
//! line of defence and it is a good one.
//!
//! So the real pairing is: [`the_dependency_list_stays_closed`] is the one that bites, because adding
//! the dependency is the only way to make an engine reference compile. [`no_source_file_reaches_for_an_engine`]
//! is the backstop for the case where someone adds the dep *and* edits `ALLOWED_DEPS` to match —
//! it names the file and line that motivated it, which a manifest diff cannot.

use std::path::{Path, PathBuf};

/// Everything `bevy_grimoire_core` is allowed to depend on. Data and arithmetic, nothing that draws.
///
/// # `bevy_grimoire_rng`, and why it widened nothing
///
/// The seeded RNG used to be `bevy_grimoire_core/src/rng.rs`; it was lifted into a sibling crate so a
/// permissively-licensed consumer could depend on the generator without taking this crate, and
/// `bevy_grimoire_core` re-exports it (`pub use bevy_grimoire_rng as rng`) so no call site moved.
///
/// It is on this list because **the dependency surface did not actually grow**: `bevy_grimoire_rng`'s own
/// manifest declares `rand` and `rand_chacha` and nothing else, both of which are already here. The
/// same code is reachable at a different address. It also carries its own `tests/leaf.rs`, so the
/// boundary is policed on that side rather than taken on trust from here.
///
/// That is the argument this list is supposed to cost. A dependency that pulled in anything not
/// already on this line would need a different one.
///
/// # `bevy_grimoire_solver`, and the different argument it needs
///
/// It is the one entry here that genuinely GROWS the surface — by two crates, itself and `batsat`
/// (which brings only `bit-vec`). So the `bevy_grimoire_rng` argument above does not cover it, and this is the
/// one it needs instead.
///
/// **What this list defends is not dependency count.** The test's own purpose, stated at the top, is
/// that the crate stays consumable *"by the game, by the offline search, and by a standalone editor
/// without any of them agreeing on a renderer"* — which is why `FORBIDDEN_DEP_MARKERS` names engines
/// and nothing else. A decision procedure with no I/O, no threads, no allocator of its own and no
/// engine does not touch that property.
///
/// **What it buys is a thing this crate cannot do without.** `constraints.rs` encodes a world as
/// clauses; something has to turn clauses into bits, and writing a CDCL solver here would be a
/// far larger and less-tested piece of code than taking one.
///
/// **Why a sibling crate rather than `batsat` directly.** The back-end is the part of the constraint
/// stack most likely to be replaced — see `docs/research/2026-08-10-solver-choice.md` §6, which
/// argues its own recommendation might not survive a hard instance. Naming a workspace sibling means
/// the swap is one file in one crate, and that sibling carries its OWN `tests/leaf.rs` pinned to
/// `["batsat"]` — precisely the arrangement the `bevy_grimoire_rng` note above blesses, where the boundary is
/// policed on that side rather than taken on trust from here.
///
/// The alternative considered was `varisat`, which would have grown the surface by **thirty-two**
/// crates including two `syn` majors, `regex` and a proc-macro toolchain. That is the comparison that
/// makes this a paragraph rather than an argument.
///
/// **`serde_path_to_error`** (#200) names the field a refused file is wrong at (`placements[3].at[1]`),
/// which ron does only for a missing field. It depends on `serde` and `itoa` alone, holds no engine
/// type, and its one user is `reload_check.rs`, which reads a file a second time only after refusing
/// it. Added as an assumed owner exception (a new row, nothing loosened).
const ALLOWED_DEPS: &[&str] = &[
    "serde",
    "serde_json",
    "ron",
    "rand",
    "rand_chacha",
    "bevy_grimoire_rng",
    "bevy_grimoire_solver",
    "serde_path_to_error",
];

/// Crate names that would mean the boundary has been crossed, checked as substrings so
/// `bevy_math`/`bevy_ecs` are caught as readily as `bevy`.
const FORBIDDEN_DEP_MARKERS: &[&str] = &["bevy", "avian", "wgpu", "winit"];

/// **Crate names that contain the marker `bevy` and are not an engine.** This crate and its two
/// engine-free siblings are all named `bevy_grimoire_*`, so the substring `bevy` alone no longer
/// separates "an engine crate" from "us". Exactly these identifiers are exempt, matched whole (an
/// identifier boundary on both sides) so `bevy`, `bevy_ecs` or a hypothetical `bevy_cauldron`
/// still trip the check. Adding a name here is a design decision, like widening `ALLOWED_DEPS`.
const SIBLING_DEPS: &[&str] = &["bevy_grimoire_rng", "bevy_grimoire_solver"];

/// This crate's own name, which source text (a `use`, a message) may spell.
const OWN_NAME: &str = "bevy_grimoire_core";

/// `line` with every whole-identifier occurrence of this crate's name or a [`SIBLING_DEPS`] name
/// removed.
fn without_sibling_names(line: &str) -> String {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = line.to_owned();
    for name in SIBLING_DEPS.iter().copied().chain([OWN_NAME]) {
        let mut from = 0;
        while let Some(found) = out[from..].find(name) {
            let start = from + found;
            let end = start + name.len();
            let before = out[..start].chars().next_back();
            let after = out[end..].chars().next();
            if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
                from = end;
                continue;
            }
            out.replace_range(start..end, "");
            from = start;
        }
    }
    out
}

fn crate_root() -> PathBuf {
    // Cargo runs a test binary with the cwd set to its own package root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Strip `//` line comments and `/* */` blocks so a doc comment *about* Bevy — of which this crate has
/// several, deliberately — is not mistaken for an import of it.
///
/// Deliberately simple: it does not track string literals, so a `"//"` inside a string would end the
/// line early. That is acceptable here because the only thing this feeds is a substring search for
/// crate names, and an over-eager strip can only ever cause a false *pass* on a line that already
/// contains no dependency.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_block = false;
    for line in src.lines() {
        let mut rest = line;
        loop {
            if in_block {
                match rest.find("*/") {
                    Some(end) => {
                        in_block = false;
                        rest = &rest[end + 2..];
                    }
                    None => break,
                }
            } else {
                let line_at = rest.find("//");
                let block_at = rest.find("/*");
                match (line_at, block_at) {
                    (Some(l), b) if b.is_none_or(|b| l < b) => {
                        out.push_str(&rest[..l]);
                        break;
                    }
                    (_, Some(b)) => {
                        out.push_str(&rest[..b]);
                        in_block = true;
                        rest = &rest[b + 2..];
                    }
                    _ => {
                        out.push_str(rest);
                        break;
                    }
                }
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn no_source_file_reaches_for_an_engine() {
    let src = crate_root().join("src");
    let mut files = Vec::new();
    rust_sources(&src, &mut files);
    assert!(
        files.len() >= 10,
        "expected to scan the whole crate, found only {} file(s) — has the layout moved?",
        files.len()
    );

    let mut offenders = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let code = strip_comments(&text);
        for (n, line) in code.lines().enumerate() {
            let checked = without_sibling_names(line);
            for marker in FORBIDDEN_DEP_MARKERS {
                if checked.contains(marker) {
                    offenders.push(format!(
                        "{}:{} — {}",
                        path.strip_prefix(crate_root()).unwrap_or(path).display(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "bevy_grimoire_core must stay engine-free, but {} line(s) reference an engine crate.\n  {}\n\n\
         This crate is what lets the game, the headless search and the standalone editor share the \
         placement stack without agreeing on a renderer. If a type genuinely needs to cross the \
         boundary, the answer is a plain-data type here and a conversion on the far side — not a \
         dependency. See docs/2026-08-03-emerge-mapper-plan.md.",
        offenders.len(),
        offenders.join("\n  ")
    );
}

#[test]
fn the_dependency_list_stays_closed() {
    let manifest = std::fs::read_to_string(crate_root().join("Cargo.toml"))
        .expect("bevy_grimoire_core must have a Cargo.toml");

    // Only the `[dependencies]` table — a dev-dependency on something heavier would be a different
    // (and much less alarming) conversation.
    let deps = manifest
        .split("[dependencies]")
        .nth(1)
        .expect("bevy_grimoire_core must declare a [dependencies] table");
    let deps = deps.split("\n[").next().unwrap_or(deps);

    for line in deps.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `.` is a separator too, so an inherited dependency written as the dotted key
        // `serde.workspace = true` reads as `serde` rather than as a crate called
        // `serde.workspace`. A crate name cannot contain a dot, so nothing real is truncated —
        // and the check keeps biting on the part that matters, the name before it.
        let name = line.split(['=', ' ', '.']).next().unwrap_or("");
        // A dependency line is `name = ...` or `name.workspace = true`. Anything else inside this
        // table is a CONTINUATION of a multi-line value — a feature array's entries, a closing
        // bracket — and is not a crate name. Reading one as a crate name produced a genuinely
        // baffling failure ("declares `\"bevy_asset\",`"), so continuations are skipped by shape.
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            continue;
        }
        assert!(
            ALLOWED_DEPS.contains(&name),
            "bevy_grimoire_core declares `{name}`, which is not in its allowed set {ALLOWED_DEPS:?}.\n\
             Widening this is a design decision, not a convenience — see \
             docs/2026-08-03-emerge-mapper-plan.md. If it is genuinely warranted, add it to ALLOWED_DEPS in \
             this test in the same commit, so the change is visible in review."
        );
        // The two engine-free siblings are named `bevy_grimoire_*`; the exact names are exempt, nothing
        // that merely starts with `bevy` is.
        if SIBLING_DEPS.contains(&name) {
            continue;
        }
        for marker in FORBIDDEN_DEP_MARKERS {
            assert!(
                !name.contains(marker),
                "bevy_grimoire_core declares `{name}` — the crate exists precisely so it does not depend on \
                 an engine."
            );
        }
    }
}
