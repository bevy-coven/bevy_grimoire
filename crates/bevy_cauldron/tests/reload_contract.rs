//! **The reload contract's ratchets, read off the operation registry** (#198, hot-reload H1).
//!
//! `bevy_grimoire_core::reload::KINDS` lists every kind of file the editor writes, with the class a
//! change to it costs a running program. Each operation declares the kinds it writes
//! (`Operation::writes`, no default), so the list is enumerated from the registry and not kept by
//! hand. The rows of `docs/guide/reload-contract.md` are rendered from the same two lists.
//!
//! Bless the guide after an honest change with `UPDATE_RELOAD_CONTRACT=1 cargo test -p
//! bevy_cauldron --test editor reload_contract::`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy_grimoire_core::reload::{self, Base, KINDS};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

/// **Operations registered only with a cargo feature.** The guide names them in the kind's `other_writers`,
/// so its table is the same with and without the features.
const FEATURE_GATED: &[&str] = &["destructible_set", "destructible_hit", "terrain_preset"];

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

/// **An operation's declaration is true**: run some and every file each one changed is a kind it
/// declares. `oplog` is the dispatcher's own, written for every operation. `terrain_stroke` writes into
/// the authoring kit's directory and `map_rename` into the project's, so both bases are exercised.
#[test]
fn a_dispatched_operation_changes_only_the_kinds_it_declares() {
    let root = fixtures::Fixture::new("reload_contract_writes").descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    let mut ctx = Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() };

    let stroke = serde_json::json!({
        "map": "m",
        "create": {"origin": [0.0, 0.0], "cell_m": 1.0, "width": 11, "depth": 11, "height_m": 0.0},
        "stroke": {"brush": "Raise", "radius_m": 2.0, "strength": 1.5, "points": [[5.0, 5.0]]}
    });
    let rename = serde_json::json!({"name": "Town Two"});
    for (op, params) in [("terrain_stroke", stroke), ("map_rename", rename)] {
        let before = stat(&root);
        match ops::dispatch(&mut ctx, op, &params) {
            Ok(Applied::Done { .. }) => {}
            other => panic!("{op} was not applied: {other:?}"),
        }
        let after = stat(&root);
        let declared = ops::find(op).expect("a registered operation").writes();
        let mut changed: Vec<&PathBuf> = after.iter().filter(|(p, now)| before.get(*p) != Some(*now)).map(|(p, _)| p).collect();
        changed.extend(before.keys().filter(|p| !after.contains_key(*p)));
        assert!(!changed.is_empty(), "{op} changed no file, so the check proves nothing");
        for path in changed {
            let rel = path.strip_prefix(&root).expect("under the project root").to_string_lossy().replace('\\', "/");
            let kind = match rel.strip_prefix("assets/grimoire/") {
                Some(under) => reload::kind_of(Base::Grimoire, under),
                None => reload::kind_of(Base::Root, &rel),
            }
            .unwrap_or_else(|| panic!("{op} changed `{rel}`, which is no kind in bevy_grimoire_core::reload::KINDS"));
            assert!(
                kind.id == "oplog" || declared.contains(&kind.id),
                "{op} changed `{rel}` (kind `{}`) and declares only {declared:?}",
                kind.id
            );
        }
    }
}

/// **The guide's table is the registry's, rendered.** A kind, a class or a writer changed without the
/// guide fails here; `UPDATE_RELOAD_CONTRACT=1` rewrites it.
#[test]
fn the_guides_table_is_the_contract_rendered() {
    let mut writers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for op in ops::registry().iter().filter(|op| !FEATURE_GATED.contains(&op.name())) {
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
