//! **A global observer's resources stay visible to the signature ratchets.**
//!
//! `the_sweep_is_finished.rs` and `every_resource_says_what_a_door_does_to_it.rs` read `fn`
//! signatures to check that an `on_*` observer takes its resources as `Option<Res<_>>`: a bare `Res`
//! panics when the door that owns it is closed. Both read only the parameter list, so a
//! `#[derive(SystemParam)]` bundle with a bare `Res` field, taken by an observer, passes them while
//! panicking at runtime. This test resolves bundle fields (transitively) for every observer, which
//! is a function registered with `add_observer` (global, firing for any matching event). An entity
//! observer attached with `.observe` only fires for its own entity, which a door's panel owns, so it
//! is out of scope here as it is for the signature ratchets.

use std::collections::{HashMap, HashSet};
use std::path::Path;

fn code_outside_tests(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("#[cfg(test)]") {
            i += 1;
            while i < lines.len() && lines[i] != "}" {
                i += 1;
            }
            i += 1;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    out.join("\n")
}

/// Names that occur as identifiers in `text`.
fn idents(text: &str) -> HashSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// True when `text` has a `Res<` or `ResMut<` not directly inside `Option<`.
fn has_bare_res(text: &str) -> bool {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    for needle in ["Res<", "ResMut<"] {
        let mut from = 0;
        while let Some(at) = compact[from..].find(needle) {
            let at = from + at;
            let before = &compact[..at];
            // Part of a longer name (`MyRes<`, `NonSendRes<`), not the system param.
            let glued = before.chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_');
            if !glued && !before.ends_with("Option<") {
                return true;
            }
            from = at + needle.len();
        }
    }
    false
}

/// `(name, body)` of each `#[derive(..SystemParam..)] struct`.
fn bundles(src: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if !(l.trim_start().starts_with("#[derive(") && l.contains("SystemParam")) {
            continue;
        }
        let Some(j) = (i + 1..lines.len().min(i + 8)).find(|&j| lines[j].contains("struct ")) else {
            continue;
        };
        let head = lines[j];
        let name = head.split("struct ").nth(1).unwrap_or("");
        let name: String = name.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let mut body = String::new();
        let mut k = j;
        loop {
            body.push_str(lines[k]);
            body.push('\n');
            let t = lines[k].trim_end();
            if t.ends_with(';') || (k > j && t.trim_start() == "}") || (k == j && t.ends_with("{}")) {
                break;
            }
            k += 1;
            if k >= lines.len() {
                break;
            }
        }
        out.push((name, body));
    }
    out
}

/// Parameter lists of every `fn` whose parameters contain `On<`.
fn observer_params(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut from = 0;
    while let Some(rel) = src[from..].find("fn ") {
        let at = from + rel;
        from = at + 3;
        let ok_start = at == 0 || !(bytes[at - 1] as char).is_alphanumeric() && bytes[at - 1] != b'_';
        if !ok_start {
            continue;
        }
        let Some(open_rel) = src[from..].find('(') else { break };
        let open = from + open_rel;
        let name = src[from..open].split('<').next().unwrap_or("").trim().to_string();
        if name.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        let mut depth = 0usize;
        let mut close = None;
        for (i, b) in bytes.iter().enumerate().skip(open) {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { break };
        let params = &src[open..=close];
        if params.contains("On<") {
            out.push((name, params.to_string()));
        }
        from = close;
    }
    out
}

/// Last path segment of each `add_observer(path::name)` argument.
fn registered(src: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut from = 0;
    while let Some(rel) = src[from..].find("add_observer(") {
        let start = from + rel + "add_observer(".len();
        let end = src[start..].find(')').map_or(src.len(), |e| start + e);
        let arg = src[start..end].trim();
        out.insert(arg.rsplit("::").next().unwrap_or(arg).trim().to_string());
        from = end;
    }
    out
}

/// Observers (`file::name`) that take a bundle holding a bare `Res`/`ResMut`, directly or nested.
fn offenders(files: &[(String, String)]) -> Vec<String> {
    let mut all: HashMap<String, String> = HashMap::new();
    for (_, src) in files {
        for (n, b) in bundles(src) {
            all.insert(n, b);
        }
    }
    let mut bare: HashSet<String> = all.iter().filter(|(_, b)| has_bare_res(b)).map(|(n, _)| n.clone()).collect();
    loop {
        let more: Vec<String> = all
            .iter()
            .filter(|(n, b)| !bare.contains(*n) && idents(b).iter().any(|w| bare.contains(w)))
            .map(|(n, _)| n.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        bare.extend(more);
    }
    let global: HashSet<String> = files.iter().flat_map(|(_, s)| registered(s)).collect();
    let mut out = Vec::new();
    for (file, src) in files {
        for (name, params) in observer_params(src) {
            if !global.contains(&name) {
                continue;
            }
            let ids = idents(&params);
            let mut hit: Vec<&String> = bare.iter().filter(|b| ids.contains(*b)).collect();
            hit.sort();
            for b in hit {
                out.push(format!("{file}: observer `{name}` takes bundle `{b}`, which holds a bare Res/ResMut"));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_scanner_sees_a_bare_res_inside_a_bundle_an_observer_takes() {
    let src = "#[derive(SystemParam)]\npub struct Hidden<'w> {\n    pub p: Res<'w, Project>,\n}\n\
        #[derive(SystemParam)]\nstruct Outer<'w> {\n    h: Hidden<'w>,\n}\n\
        #[derive(SystemParam)]\nstruct Fine<'w> {\n    p: Option<Res<'w, Project>>,\n}\n\
        fn on_a(activate: On<Activate>, h: Outer) {}\n\
        fn on_b(activate: On<Activate>, f: Fine) {}\n\
        fn on_row(click: On<Click>, h: Outer) {}\n\
        fn plain(h: Outer) {}\n\
        fn build(app: &mut App) { app.add_observer(on_a).add_observer(crate::x::on_b); row.observe(on_row); }\n";
    let got = offenders(&[("fake.rs".into(), src.into())]);
    assert_eq!(
        got,
        vec![
            "fake.rs: observer `on_a` takes bundle `Outer`, which holds a bare Res/ResMut".to_string(),
        ]
    );
}

#[test]
fn no_observer_takes_a_bundle_with_a_bare_resource() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("src/").flatten() {
        let p = entry.path();
        if p.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        files.push((name, code_outside_tests(&src)));
    }
    let bad = offenders(&files);
    assert!(
        bad.is_empty(),
        "a global observer fires for any `Activate`; a bare `Res` inside a bundle it takes panics \
         when its door is closed, and the signature ratchets cannot see it. Make the field \
         `Option<Res<_>>`:\n{}",
        bad.join("\n")
    );
}
