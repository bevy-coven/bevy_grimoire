//! **What will always need a rebuild is a checked-in list with a reason for each entry, and the guide
//! shows it** (#202, H5 step 3).
//!
//! `bevy_grimoire_core::reload::REBUILD` is the one answer to "why does changing this need a rebuild".
//! The guide's table is rendered from it, and this test fails when the two differ; bless the guide after
//! an honest change with `UPDATE_RELOAD_CONTRACT=1 cargo test -p bevy_grimoire_core --test
//! the_always_rebuild_list_is_in_the_guide`.

use std::collections::BTreeSet;
use std::path::Path;

use bevy_grimoire_core::reload::{self, KINDS, REBUILD};

#[test]
fn every_entry_says_why_a_file_cannot_hold_it() {
    assert!(!REBUILD.is_empty(), "the list is the answer to what needs a rebuild: it is not empty");
    let mut ids = BTreeSet::new();
    for r in REBUILD {
        assert!(ids.insert(r.id), "the setting `{}` is listed twice", r.id);
        assert!(r.what.split_whitespace().count() >= 4, "`{}` does not say what it is: `{}`", r.id, r.what);
        assert!(
            r.reason.split_whitespace().count() >= 8,
            "`{}` needs a reason, the cause and not the symptom: `{}`",
            r.id,
            r.reason
        );
    }
}

/// **A setting with a file is a kind, not an entry here**: the two lists never name the same thing, so a
/// setting that gains a file form leaves this list in the commit that adds its row to `KINDS`.
#[test]
fn a_setting_is_never_both_a_file_kind_and_a_rebuild() {
    for r in REBUILD {
        assert!(
            reload::kind_by_id(r.id).is_none() && KINDS.iter().all(|k| k.id != r.id),
            "`{}` is on the always-rebuild list and is also a file kind: it has a file, so it is not always a rebuild",
            r.id
        );
    }
}

#[test]
fn the_guides_rebuild_table_is_the_list_rendered() {
    let rendered = reload::rebuild_table();
    let guide = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/guide/reload-contract.md");
    let text = std::fs::read_to_string(&guide).unwrap_or_else(|e| panic!("{}: {e}", guide.display()));
    const START: &str = "<!-- rebuild-list:start -->\n";
    const END: &str = "<!-- rebuild-list:end -->";
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
        "docs/guide/reload-contract.md is out of date with bevy_grimoire_core::reload::REBUILD. Regenerate it with UPDATE_RELOAD_CONTRACT=1.\n--- expected ---\n{rendered}"
    );
}
