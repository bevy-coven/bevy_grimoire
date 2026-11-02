//! **The restart list only shrinks, and the reload contract's patterns do not collide** (#198, H1).
//!
//! `bevy_grimoire_core::reload::KINDS` gives every kind of file the editor writes a `ReloadClass`.
//! `Restart` is the one that means *a running program that holds this file cannot take a change to it*,
//! so it is debt, and the debt may be paid down but never run up silently: a kind joins `Restart` only
//! by editing the list below, and a reviewer sees that edit. The operations' half of the contract
//! (every written kind has a class) is `crates/bevy_cauldron/tests/reload_contract.rs`.

use bevy_grimoire_core::reload::{self, Base, Lifetime, ReloadClass, KINDS};

/// **The kinds that cannot be hot today**, checked in. Remove a kind here in the commit that gives it a
/// reload path (`Live` or `Respawn`); never add one without writing down in the commit why it cannot
/// be hot.
const RESTART_KINDS: &[&str] = &[
    // (`rigs` left this list in #202: the animation tab takes an edit of `rigs.ron` at once.)
    // The game's own tuning files, written by `data_set`: this repository ships no reader for them.
    "game_data",
];

/// **A literal, and not `RESTART_KINDS.len()`**: a number that is its own measure can never fail. Lower it
/// with the list; raising it is a decision a reviewer must see.
const RESTART_KINDS_MAX: usize = 1;

#[test]
fn the_restart_list_only_shrinks() {
    assert!(
        RESTART_KINDS.len() == RESTART_KINDS_MAX,
        "the checked-in restart list has {} kinds and the ratchet says {RESTART_KINDS_MAX}: the two move \
         together. Lower both when a kind gains a reload path; raising either is a decision a reviewer \
         must see, and the commit says why the kind cannot be hot.",
        RESTART_KINDS.len()
    );
    for k in reload::kinds_of_class(ReloadClass::Restart) {
        assert!(
            RESTART_KINDS.contains(&k.id),
            "the kind `{}` is `Restart` and is not on the checked-in list in \
             crates/bevy_grimoire_core/tests/reload_contract.rs. The list may shrink and never silently \
             grow: give the kind a reload path (`Live` or `Respawn`), or add it to the list in a commit that \
             says why it cannot be hot.",
            k.id
        );
    }
    // And the list is tight: a kind that gained a reload path comes off it in the same commit, or the
    // list is a claim about debt that is already paid and the next kind to join slips in unseen.
    for id in RESTART_KINDS {
        let k = reload::kind_by_id(id).unwrap_or_else(|| panic!("`{id}` is on the restart list and is no kind"));
        assert_eq!(
            k.class,
            ReloadClass::Restart,
            "`{id}` is on the restart list and is now `{}`: take it off the list and lower RESTART_KINDS_MAX",
            k.class.word()
        );
    }
}

/// **Each pattern answers to exactly its own kind**, so a changed path is classified one way. Every
/// placeholder is filled with every word that is a fixed segment of some pattern (`water`, `library`,
/// `maps`...), not only a sample name, because a collision needs one pattern's placeholder to meet
/// another's literal: a kit named `water` must not make `water/library.ron` two kinds.
#[test]
fn every_kind_is_found_by_its_own_pattern_and_by_no_other() {
    let mut ids = std::collections::BTreeSet::new();
    let mut words = std::collections::BTreeSet::from(["sample".to_owned()]);
    for k in KINDS {
        assert!(ids.insert(k.id), "the kind id `{}` appears twice", k.id);
        for segment in k.pattern.split('/').filter(|s| !s.is_empty() && !s.contains('<')) {
            words.insert(segment.split('.').next().unwrap_or(segment).to_owned());
        }
    }
    let words: Vec<String> = words.into_iter().collect();
    for k in KINDS.iter().filter(|k| !k.pattern.is_empty()) {
        // Every way to fill the pattern's placeholders from `words`.
        let mut paths = vec![String::new()];
        for segment in k.pattern.split('/') {
            let mut next = Vec::new();
            for path in &paths {
                let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
                match segment.find('<') {
                    None => next.push(format!("{prefix}{segment}")),
                    Some(open) => {
                        let close = segment.find('>').expect("a `<` closes with `>`");
                        for w in &words {
                            next.push(format!("{prefix}{}{w}{}", &segment[..open], &segment[close + 1..]));
                        }
                    }
                }
            }
            paths = next;
        }
        for path in paths {
            let claimants: Vec<&str> = KINDS.iter().filter(|o| o.base == k.base && o.matches(&path)).map(|o| o.id).collect();
            assert_eq!(claimants, vec![k.id], "`{path}` should be only `{}`", k.id);
            assert_eq!(reload::kind_of(k.base, &path).map(|f| f.id), Some(k.id));
        }
    }
}

/// **A per-map kind is named after its map**, because that name is how a reload finds the map it is for.
#[test]
fn a_per_map_kind_is_named_after_its_map() {
    for k in KINDS.iter().filter(|k| k.lifetime == Lifetime::PerMap) {
        assert_eq!(k.base, Base::Grimoire, "`{}` is per map and lives outside the grimoire directory", k.id);
        assert!(k.pattern.contains("<name>"), "`{}` is per map and its pattern `{}` has no `<name>`", k.id, k.pattern);
    }
}
