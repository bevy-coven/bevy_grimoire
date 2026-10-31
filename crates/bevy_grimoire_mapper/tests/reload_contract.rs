//! **The reload contract's ratchets, read off the operation registry** (#198, hot-reload H1).
//!
//! `bevy_grimoire_core::reload::KINDS` lists every kind of file the editor writes, with the class a
//! change to it costs a running program. Each operation declares the kinds it writes
//! (`Operation::writes`, no default), so the list is enumerated from the registry and not kept by
//! hand. The rows of `docs/guide/reload-contract.md` are rendered from the same two lists.
//!
//! Bless the guide after an honest change with `UPDATE_RELOAD_CONTRACT=1 cargo test -p
//! bevy_grimoire_mapper --test editor reload_contract::`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire_core::reload::{self, Base, KINDS};
use bevy_grimoire_mapper::harness;
use bevy_grimoire_mapper::ops;

use crate::fixtures;

/// **An operation that writes a kind the contract does not classify fails.** Prove it by removing a
/// row from `KINDS`: the message names the operation and the kind.
#[test]
fn every_written_kind_has_a_reload_class() {
    for op in ops::registry() {
        for id in op.writes() {
            assert!(
                reload::kind_by_id(id).is_some(),
                "`{}` writes the kind `{id}`, and bevy_grimoire_core::reload::KINDS has no such kind, \
                 so nothing says whether a running game picks the file up. Add the kind with a \
                 ReloadClass and a Lifetime (and a row in docs/guide/reload-contract.md).",
                op.name()
            );
        }
    }
}

/// **The list cannot keep a kind nobody writes**: each kind is written by an operation, or says who
/// writes it instead.
#[test]
fn every_kind_is_written_by_an_operation_or_says_who_writes_it() {
    let written: Vec<&str> = ops::registry().iter().flat_map(|op| op.writes().iter().copied()).collect();
    for k in KINDS {
        assert!(
            written.contains(&k.id) || !k.other_writers.is_empty(),
            "the kind `{}` is written by no operation and names no other writer: remove it, or say who \
             writes it in `other_writers`",
            k.id
        );
    }
}

/// **An operation's declaration is true**: run one and every file it changed is a kind it declares.
/// `oplog` is the dispatcher's own, written for every operation.
#[test]
fn a_dispatched_operation_changes_only_the_kinds_it_declares() {
    let root = fixtures::Fixture::new("reload_contract_writes")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    let grimoire = root.join("assets/grimoire");
    let before = stat(&grimoire);
    let answer = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "map_rename", "parameters": {"name": "Town Two"}, "dry_run": false}),
    );
    assert!(answer.get("saved").is_some(), "map_rename answered {answer}");
    let after = stat(&grimoire);

    let declared = ops::find("map_rename").expect("map_rename is registered").writes();
    let mut changed = Vec::new();
    for (path, now) in &after {
        if before.get(path) != Some(now) {
            changed.push(path.clone());
        }
    }
    for path in before.keys().filter(|p| !after.contains_key(*p)) {
        changed.push(path.clone());
    }
    assert!(!changed.is_empty(), "map_rename changed no file, so the check proves nothing");
    for path in changed {
        let rel = path.strip_prefix(&grimoire).expect("under the grimoire directory").to_string_lossy().replace('\\', "/");
        let kind = reload::kind_of(Base::Grimoire, &rel)
            .unwrap_or_else(|| panic!("`{rel}` changed and is no kind in bevy_grimoire_core::reload::KINDS"));
        assert!(
            kind.id == "oplog" || declared.contains(&kind.id),
            "map_rename changed `{rel}` (kind `{}`) and declares only {declared:?}",
            kind.id
        );
    }
}

/// **The guide's table is the registry's, rendered.** A kind, a class or a writer changed without the
/// guide fails here; `UPDATE_RELOAD_CONTRACT=1` rewrites it.
#[test]
fn the_guides_table_is_the_contract_rendered() {
    let mut writers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for op in ops::registry() {
        for id in op.writes() {
            writers.entry((*id).to_owned()).or_default().push(op.name().to_owned());
        }
    }
    let rendered = reload::table(&|id| writers.get(id).cloned().unwrap_or_default());
    let guide = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/guide/reload-contract.md");
    let text = std::fs::read_to_string(&guide).unwrap_or_else(|e| panic!("{}: {e}", guide.display()));
    const START: &str = "<!-- reload-contract:start -->\n";
    const END: &str = "<!-- reload-contract:end -->";
    let (Some(a), Some(b)) = (text.find(START), text.find(END)) else {
        panic!("{} has no `{START}` ... `{END}` block", guide.display());
    };
    let current = &text[a + START.len()..b];
    if current == rendered {
        return;
    }
    if std::env::var_os("UPDATE_RELOAD_CONTRACT").is_some() {
        let new = format!("{}{START}{rendered}{}", &text[..a], &text[b..]);
        std::fs::write(&guide, new).unwrap_or_else(|e| panic!("{}: {e}", guide.display()));
        return;
    }
    panic!(
        "docs/guide/reload-contract.md is out of date with bevy_grimoire_core::reload::KINDS and the \
         operations' `writes`. Regenerate it with UPDATE_RELOAD_CONTRACT=1.\n--- expected ---\n{rendered}"
    );
}

fn stat(dir: &Path) -> BTreeMap<PathBuf, (u64, std::time::SystemTime)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else if let Ok(modified) = meta.modified() {
                out.insert(e.path(), (meta.len(), modified));
            }
        }
    }
    out
}

/// **Through the registered method**, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}
