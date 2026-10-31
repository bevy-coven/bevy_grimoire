//! **No code in the editor writes a project file the plain way** (#199, H2).
//!
//! `std::fs::write` truncates the target and then fills it, so a running game, or the editor's own
//! watcher, that reads in between parses an empty or half-written file (#187 was a truncated terrain
//! file that panicked the editor; #158 a file created while open). Every project file goes through
//! [`bevy_grimoire_core::atomic_file`] (or `ron_surgery::save_atomic`, which is a wrapper over it).
//!
//! This reads the non-test source of the two crates that write a project, and fails on a call that
//! opens, truncates, copies or renames a file by any other route. **A new one is a failure with its
//! file and line**; the fix is `atomic_file::write` (or `write_new` where the path comes from a name
//! the author typed, `write_cache` for something that can be rebuilt). A call that really is not a
//! project file goes on [`ALLOWED`] with the reason, in the commit that adds it, where a reviewer
//! sees it. An entry that no longer matches anything fails too, so the list never holds a debt that
//! is already paid.
//!
//! Test code is not scanned (a fixture writes a plain file on purpose), and neither is
//! `bevy_grimoire_mcp`, `bevy_grimoire_snip` or `bevy_grimoire_decide`: they write screenshots,
//! snips and recorded fixtures, which are not project files.

use std::path::{Path, PathBuf};

/// The source directories scanned, relative to the workspace root.
const ROOTS: &[&str] = &["crates/bevy_grimoire_core/src", "crates/bevy_grimoire_mapper/src"];

/// The calls that put bytes in a file, or move one, without going through `atomic_file`.
const PLAIN: &[&str] = &[
    "fs::write(",
    "File::create(",
    "File::create_new(",
    "File::options(",
    "OpenOptions",
    "fs::copy(",
    "fs::rename(",
    "fs::hard_link(",
];

struct Allowed {
    /// A path ending, so `ops.rs` does not allow `ops_data.rs`.
    file: &'static str,
    /// What the line contains; empty allows the whole file.
    needle: &'static str,
    why: &'static str,
}

const ALLOWED: &[Allowed] = &[
    Allowed {
        file: "bevy_grimoire_core/src/atomic_file.rs",
        needle: "",
        why: "the one write path itself",
    },
    Allowed {
        file: "bevy_grimoire_core/src/project_data.rs",
        needle: "std::fs::rename(&old, &new)",
        why: "moves the directory `assets/emerge` to `assets/grimoire` when a project is opened; it writes no bytes",
    },
    Allowed {
        file: "bevy_grimoire_mapper/src/ops.rs",
        needle: "let mut f = std::fs::OpenOptions::new()",
        why: "the operation log is an append (`append(true)`), so it cannot lose an earlier line by being rewritten, and nobody edits it",
    },
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The lines of `text` outside its `#[cfg(test)] mod` bodies, with their numbers. A test module ends at
/// the first later line that is a lone `}` at the module's own indentation (rustfmt's shape, which the
/// workspace is written in), so production code written after a test module is still scanned.
fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut keep = vec![true; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() != "#[cfg(test)]" {
            i += 1;
            continue;
        }
        let Some(offset) = lines[i + 1..].iter().position(|l| !l.trim().starts_with("#[")) else { break };
        let module = i + 1 + offset;
        let header = lines[module];
        // A one-line item (`use ...;`, `fn f() { ... }`, `mod tests;`) is cut alone; anything longer
        // runs to the closing brace at its own indentation.
        let one_line = header.trim_end().ends_with(';') || (header.contains('{') && header.trim_end().ends_with('}'));
        let end = if one_line {
            module
        } else {
            let indent = header.len() - header.trim_start().len();
            let closer = format!("{}}}", " ".repeat(indent));
            (module + 1..lines.len())
                .find(|&k| lines[k].trim_end() == closer)
                .unwrap_or_else(|| panic!("line {}: the closing `}}` of this test item was not found at column {indent}", module + 1))
        };
        keep[i..=end].iter_mut().for_each(|k| *k = false);
        i = end + 1;
    }
    lines.iter().enumerate().filter(|(i, _)| keep[*i]).map(|(i, l)| (i + 1, *l)).collect()
}

#[test]
fn no_project_file_is_written_the_plain_way() {
    let root = workspace_root();
    let mut files = Vec::new();
    for dir in ROOTS {
        rust_files(&root.join(dir), &mut files);
    }
    assert!(files.len() > 40, "only {} source files were found, so the scan is looking in the wrong place", files.len());

    let mut hits = vec![0usize; ALLOWED.len()];
    let mut plain = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let shown = file.strip_prefix(&root).unwrap_or(file).to_string_lossy().replace('\\', "/");
        for (number, line) in code_lines(&text) {
            let code = line.trim_start();
            if code.starts_with("//") || !PLAIN.iter().any(|p| line.contains(p)) {
                continue;
            }
            let allowed = ALLOWED.iter().position(|a| shown.ends_with(a.file) && line.contains(a.needle));
            match allowed {
                Some(i) => hits[i] += 1,
                None => plain.push(format!("{shown}:{number}: {}", code.trim_end())),
            }
        }
    }

    assert!(
        plain.is_empty(),
        "{} plain write(s) of a project file. Use `bevy_grimoire_core::atomic_file::write` (`write_new` where the \
         path comes from a name the author typed, `write_cache` for a cache), so a reader never sees half a \
         file; or, if it is not a project file, add it to ALLOWED in this test with the reason:\n  {}",
        plain.len(),
        plain.join("\n  ")
    );
    for (allowed, n) in ALLOWED.iter().zip(&hits) {
        assert!(
            *n > 0,
            "ALLOWED has `{}` / `{}` ({}) and nothing matches it any more: remove the entry",
            allowed.file,
            allowed.needle,
            allowed.why
        );
    }
}

#[test]
fn the_scan_sees_a_plain_write_when_there_is_one() {
    // The scan is only worth anything if it can fail: feed it the shapes it exists to catch.
    let text = "fn save(p: &Path) {\n    std::fs::write(p, \"x\").unwrap();\n}\n\n#[cfg(test)]\nmod tests {\n    fn fixture() { std::fs::write(\"a\", \"b\").unwrap(); }\n}\n";
    let lines = code_lines(text);
    assert_eq!(lines.len(), 4, "the test module must be cut off: {lines:?}");
    assert!(lines.iter().any(|(n, l)| *n == 2 && PLAIN.iter().any(|p| l.contains(p))));
    assert!(!lines.iter().any(|(_, l)| l.contains("fixture")), "test code was scanned");
}

/// **Production code after a test module is scanned**: the first version cut each file at its first
/// test module and skipped thousands of lines after it. A nested module, an external `mod tests;` and a
/// `#[cfg(test)]` on one function are each cut by exactly their own extent.
#[test]
fn production_code_after_a_test_module_is_still_scanned() {
    let text = "\
fn one() {}
#[cfg(test)]
mod tests {
    fn fixture() { std::fs::write(\"a\", \"b\").unwrap(); }
}
fn after() { std::fs::write(\"p\", \"q\").unwrap(); }
pub mod picker {
    fn inner() {}
    #[cfg(test)]
    mod tests {
        fn nested() { std::fs::File::create(\"n\").unwrap(); }
    }
    fn inner_after() { std::fs::copy(\"a\", \"b\").unwrap(); }
}
#[cfg(test)]
mod external;
#[cfg(test)]
fn only_for_tests() { std::fs::write(\"t\", \"t\").unwrap(); }
fn last() { std::fs::rename(\"x\", \"y\").unwrap(); }
";
    let hits: Vec<usize> = code_lines(text).into_iter().filter(|(_, l)| PLAIN.iter().any(|p| l.contains(p))).map(|(n, _)| n).collect();
    assert_eq!(hits, [6, 13, 19], "the lines a scan reports: after the first test module, inside a nested module's tail, and the last");
}
