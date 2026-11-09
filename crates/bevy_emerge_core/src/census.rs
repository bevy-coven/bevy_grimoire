//! **Every count, computed from the catalog, in one place.**
//!
//! A count is derived data. The build-systems and architecture literature is unanimous that derived
//! quantities are recomputed from one authoritative store rather than cached in parallel copies, and
//! this project has the receipt: three module notes each stated the size of the same library — *"360
//! meshes and 41 are in the library"*, *"131 library entries and ~319 unlabeled candidates"* — against
//! a file holding 75. Nothing was broken by that, because the panels had always computed their numbers
//! at runtime; what drifted was the prose. But prose is what a reader trusts when deciding whether a
//! number on screen looks right.
//!
//! # Why a module rather than a habit
//!
//! Because the pressure is about to increase. One library gave one count. A library plus a set of
//! compositions plus a map that stamps them gives *described*, *composed*, *stamped* and *expanded* —
//! and every panel that wants to say "N of M" is a place to compute one slightly differently. The
//! remedy is the one the key census already uses for keys: one table, everything derived from it, so
//! disagreement is unrepresentable rather than unlikely.
//!
//! # What is deliberately not here
//!
//! Anything that has to *solve* to answer. `stamps` counts the references a map holds; it does not
//! expand them, because expansion can legitimately fail (a member with nowhere to rest) and a counter
//! that can fail is a counter every caller has to branch on. What a map expands to is
//! [`crate::composition::expand`]'s answer, and it is returned with the rows rather than as a number.

use crate::composition::{Composition, Envelope};
use crate::library::Library;
use crate::map::Map;

/// What a project can place, counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    /// Every descriptor the library defines.
    pub descriptors: usize,
    /// Those carrying at least one `kind` token — the judgement half, which measurement cannot supply
    /// and a VLM only proposes. This is the number "N of M described" is about.
    pub described: usize,
    /// Those saying anything at all about their edges. The kit contract, counted.
    pub with_edges: usize,
    /// Every composition.
    pub compositions: usize,
    /// Those claiming a tile, and therefore able to have a derived interface.
    pub bounded: usize,
    /// Members across every composition — before nesting is flattened, so a group inside a group
    /// counts as the one member it is written as.
    pub members: usize,
}

/// What one map holds, counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapCensus {
    /// Rows authored by hand. **Not** what the map draws — stamps add to that.
    pub placements: usize,
    /// Rows an author has taken responsibility for.
    pub owned: usize,
    /// References to compositions. What each expands to is `expand`'s answer, not a count.
    pub stamps: usize,
    /// Locations authored directly on the map, excluding any a stamp brings with it.
    pub locations: usize,
    /// **Rows the stamps brought** — what a consumer of this map will have beyond the
    /// hand-authored ones.
    ///
    /// This is a count of *rows*, not of references, and that is the distinction the module note
    /// used to draw the other way. It said the counter deliberately does not expand, "because
    /// expansion can legitimately fail (a member with nowhere to rest) and a counter that can fail
    /// is a counter every caller has to branch on". That reasoning was right for a counter of
    /// *references* — `map.stamps.len()` cannot fail — and wrong for a counter of *rows*, because
    /// the row count is the number the author must read to know what the game will have. A panel
    /// saying "746 placed" beside a game holding 728 is a panel lying.
    ///
    /// So the two are separate numbers, and the one that can fail is a different counter from the
    /// one that cannot: [`of_map`] returns a `Result`, and the reference count stays a plain
    /// `len()`.
    pub stamped: usize,
}

impl MapCensus {
    /// **The rows the map has** — hand-authored plus what its stamps brought.
    ///
    /// The number a consumer of this map will spawn, and the number the editor's panel must
    /// show. It is a method rather than a field because it is derived from two that are stored,
    /// and a stored third could disagree with them.
    pub fn total(&self) -> usize {
        self.placements + self.stamped
    }
}

/// Count a catalog. Pure arithmetic over the two files a project's palette comes from.
pub fn of_catalog(library: &Library, compositions: &[Composition]) -> Catalog {
    Catalog {
        descriptors: library.descriptors.len(),
        described: library.descriptors.iter().filter(|d| !d.kind.is_empty()).count(),
        with_edges: library
            .descriptors
            .iter()
            .filter(|d| {
                d.subgrid
                    .as_ref()
                    .is_some_and(|g| g.cells.iter().any(|c| c.edge.is_some()))
            })
            .count(),
        compositions: compositions.len(),
        bounded: compositions
            .iter()
            .filter(|c| matches!(c.envelope, Envelope::Bounded { .. }))
            .count(),
        members: compositions.iter().map(|c| c.members.len()).sum(),
    }
}

/// Count a map — the rows it has **and** the rows its stamps bring.
///
/// **This expands**, and so it can fail. It returns a `Result` rather than swallowing the
/// failure, because a map whose stamps do not expand has no row count to report: the honest
/// answer is the refusal, not a number that quietly omits a house.
pub fn of_map(
    map: &Map,
    compositions: &[Composition],
    library: &Library,
) -> Result<MapCensus, String> {
    // **The refusal is the answer, not a smaller number.** A map whose stamps do not expand has
    // no row count to report, and a count that quietly omitted the rows would be a number
    // nobody could check — which is the thing this module exists to prevent.
    let stamped = crate::composition::expand_map(map, compositions, library)?
        .placements
        .len();
    Ok(MapCensus {
        placements: map.placements.len(),
        owned: map.placements.iter().filter(|p| p.owned).count(),
        stamps: map.stamps.len(),
        locations: map.locations.len(),
        stamped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{Descriptor, Extent, SubCell, Subgrid};
    use crate::library::LIBRARY_VERSION;
    use crate::vocab::{Vocabularies, Vocabulary};

    fn lib() -> Library {
        let plain = Descriptor {
            id: "crate_a".to_owned(),
            extent: Extent { footprint: Some((0.5, 0.5)), height: Some(0.5) },
            kind: vec!["container".to_owned()],
            ..Default::default()
        };
        let tokened = Descriptor {
            id: "wall".to_owned(),
            extent: Extent { footprint: Some((0.5, 0.5)), height: Some(0.5) },
            subgrid: Some(Subgrid {
                cells: vec![SubCell {
                    at: (0, 0, 0),
                    solid: true,
                    edge: Some("wall".to_owned()),
                }],
            }),
            ..Default::default()
        };
        Library { version: LIBRARY_VERSION, note: None, descriptors: vec![plain, tokened] }
    }

    /// **Two independent ways of counting the same thing agree.**
    ///
    /// The collision test's analogue for metrics, and the whole reason this module is not just a
    /// habit. `of_catalog` walks the lattice cells directly; `Vocabularies::masks` resolves the same
    /// tokens into a bitfield through an entirely separate path. If those two ever disagree, one of
    /// them is lying about the corpus and no panel could tell which.
    #[test]
    fn the_edge_count_agrees_with_what_the_vocabulary_resolved() {
        let library = lib();
        let vocab = Vocabularies {
            kind: Vocabulary::of(&[("container", "a thing that holds things")]),
            edge: Vocabulary::of(&[("wall", "a solid run-face")]),
            ..Default::default()
        };
        let by_scan = of_catalog(&library, &[]).with_edges;
        let by_mask = library
            .descriptors
            .iter()
            .filter(|d| vocab.masks(d).expect("resolves").edges != 0)
            .count();
        assert_eq!(by_scan, by_mask, "two ways of counting tokened pieces must agree");
        assert_eq!(by_scan, 1);
    }

    /// A count is about the catalog, not about how the file happens to be ordered.
    #[test]
    fn counting_does_not_depend_on_order() {
        let mut library = lib();
        let one = of_catalog(&library, &[]);
        library.descriptors.reverse();
        assert_eq!(one, of_catalog(&library, &[]));
    }

    /// `described` is the judgement half — measurement never supplies a `kind`.
    #[test]
    fn described_counts_only_the_pieces_somebody_judged() {
        let c = of_catalog(&lib(), &[]);
        assert_eq!(c.descriptors, 2);
        assert_eq!(c.described, 1);
    }

    /// A map's own rows and the stamps it holds are separate numbers, because they are separate
    /// things — a stamp is one reference however many rows it stands for.
    ///
    /// And the third number: the rows the stamps bring, which is what the consumer will have
    /// beyond the hand-authored ones. A stamp naming a group nothing defines cannot be counted —
    /// there is no row count for it — so the counter refuses rather than under-reporting.
    #[test]
    fn a_maps_rows_and_its_stamps_are_counted_apart() {
        let mut map = Map::default();
        map.placements.push(crate::map::Placed {
            id: "a".to_owned(),
            descriptor: "crate_a".to_owned(),
            owned: true,
            owned_because: Some("the only way out".to_owned()),
            ..Default::default()
        });
        map.stamps.push(crate::composition::Stamped {
            id: "s1".to_owned(),
            of: "station".to_owned(),
            ..Default::default()
        });
        let err = of_map(&map, &[], &lib()).expect_err("the group does not exist");
        assert!(
            err.contains("s1"),
            "the refusal must name the stamp: {err}"
        );
    }

    /// **A map whose stamps expand is counted in all three numbers.**
    ///
    /// The hand-authored rows, the references, and the rows the references stand for — and the
    /// total, which is the number the consumer will have and the number the editor's panel must
    /// show. A panel that said "746" beside a game holding 728 would be a panel lying.
    #[test]
    fn a_maps_stamps_are_counted_in_the_rows_they_bring() {
        let mut map = Map::default();
        map.placements.push(crate::map::Placed {
            id: "a".to_owned(),
            descriptor: "crate_a".to_owned(),
            ..Default::default()
        });
        let comp = crate::composition::Composition {
            id: "station".to_owned(),
            envelope: crate::composition::Envelope::Anchored,
            members: vec![crate::composition::Member {
                id: "m0".to_owned(),
                body: crate::composition::Body::Descriptor {
                    id: "crate_a".to_owned(),
                    tip: (0, 0),
                    on: None,
                    patch: None,
                },
                at: (0.0, 0.0),
                yaw: 0.0,
                lift: 0.0,
                paint: 0,
                of_fingerprint: None,
                note: None,
            }],
            locations: Vec::new(),
            note: None,
        };
        map.stamps.push(crate::composition::Stamped {
            id: "s1".to_owned(),
            of: "station".to_owned(),
            ..Default::default()
        });
        let c = of_map(&map, &[comp], &lib()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((c.placements, c.stamps, c.stamped), (1, 1, 1));
        assert_eq!(c.total(), 2, "the consumer will have two rows");
    }
}
