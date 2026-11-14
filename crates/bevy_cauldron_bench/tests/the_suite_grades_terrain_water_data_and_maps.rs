//! **The checks a first suite needs beyond a placement** (BM-04, #414): a number in a data file, the
//! height of the ground, a lake, and how many pieces stand where. Each is measured from the saved files
//! with its own arithmetic, like every other check, and none needs an editor.
//!
//! A suite about the editor's job is about more than a bed against a wall: raise a hill, put water in the
//! lowest corner, set a number in the game's data, make a map. Those leave no placement to measure, so
//! without these checks the task either grades nothing or grades the wrong file. Each case below states a
//! passing project and the one change that fails it, so a check that said `true` for everything would be
//! caught by the second half.

use bevy_cauldron_bench::check::{grade, Check};
use bevy_cauldron_bench::{Handback, Snapshot};

/// **A scratch project this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "grimoire-bench-suite-checks-{}-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// **Write one project file** under `assets/grimoire/`.
fn write(root: &std::path::Path, rel: &str, text: &str) {
    let path = root.join("assets/grimoire").join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
    }
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// **A project holding every file these checks read**: a map with five pieces, a terrain with one hill,
/// a plan with one lake, a data file and a second map the task "made".
///
/// The terrain is 5 x 5 cells of 2 m from `(-4, -4)`, so cell `(ix, iz)` is at `(-4 + 2 ix, -4 + 2 iz)`; it
/// is 0 m everywhere except the north-east corner, `(4, -4)`, which is 2 m.
fn project(test: &str) -> std::path::PathBuf {
    let root = scratch(test);
    let piece = |id: &str, descriptor: &str, x: f32, z: f32| {
        format!("- id: {id}\n  descriptor: {descriptor}\n  at:\n  - {x}\n  - {z}\n  yaw: 0.0\n")
    };
    let mut map = String::from(
        "version: 9\nname: m\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 8.0\n- 3.0\n- 8.0\nbash: null\nplacements:\n",
    );
    map.push_str(&piece("plant_a@1", "plant_a", -3.0, 3.5));
    map.push_str(&piece("plant_a@2", "plant_a", -1.5, 3.5));
    map.push_str(&piece("plant_a@3", "plant_a", 0.0, 3.5));
    map.push_str(&piece("table@4", "table", 2.0, 0.0));
    map.push_str("- id: lamp@5\n  descriptor: lamp\n  at:\n  - 2.0\n  - 0.0\n  yaw: 0.0\n  on: table@4\n");
    write(&root, "maps/m.map.yaml", &map);
    let mut heights = [0.0_f32; 25];
    heights[4] = 2.0; // ix 4, iz 0: (4, -4), the north-east corner.
    let heights: Vec<String> = heights.iter().map(|h| format!("- {h}")).collect();
    write(
        &root,
        "furniture/terrain/m.yaml",
        &format!(
            "version: 1\norigin:\n- -4.0\n- -4.0\ncell_m: 2.0\nwidth: 5\ndepth: 5\nheights:\n{}\n",
            heights.join("\n")
        ),
    );
    write(
        &root,
        "furniture/water/m.yaml",
        "version: 1\nlakes:\n- x: -3.0\n  z: 3.0\n  radius_m: 1.5\n  level_m: -0.2\n",
    );
    write(&root, "data/gate.yaml", "gate:\n  health: 40\n  armour: 2.5\n  tags:\n  - stone\n");
    write(&root, "maps/cellar.map.yaml", "version: 9\nname: cellar\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 12.0\n- 3.0\n- 8.0\nbash: null\nplacements: []\n");
    root
}

/// **One check, read the way a task writes it.**
fn check(text: &str) -> Check {
    bevy_grimoire_core::yaml::from_str::<Check>(text, "the test's check")
        .unwrap_or_else(|p| panic!("the check `{text}` reads: {}", p.line()))
}

/// **The verdict of one check on a project**, against itself as the setup.
fn verdict(root: &std::path::Path, text: &str) -> bevy_cauldron_bench::Verdict {
    let snap = Snapshot::read(root, "m", Vec::<Handback>::new(), Default::default());
    grade(&check(text), &snap, &snap)
}

/// **Assert a check holds**, quoting what it measured when it does not.
fn holds(root: &std::path::Path, text: &str) {
    let v = verdict(root, text);
    assert!(v.passed, "`{text}` holds on this project: {}", v.measured);
}

/// **Assert a check fails**, and says what it measured.
fn fails(root: &std::path::Path, text: &str) -> String {
    let v = verdict(root, text);
    assert!(!v.passed, "`{text}` fails on this project: {}", v.measured);
    assert!(!v.measured.is_empty(), "`{text}` says what it measured");
    v.measured
}

/// **A leaf of a data file is read by its path**, a number by its value and a map's size by its index.
#[test]
fn a_file_value_reads_a_leaf_of_a_project_file() {
    let root = project("file_value");
    holds(&root, "file_value: {file: data/gate.yaml, path: gate.health, equals: 40}");
    holds(&root, "file_value: {file: data/gate.yaml, path: gate.armour, min: 2.0, max: 3.0}");
    holds(&root, r#"file_value: {file: data/gate.yaml, path: "gate.tags[0]", equals: stone}"#);
    holds(&root, r#"file_value: {file: maps/cellar.map.yaml, path: "bounds[0]", equals: 12}"#);
    holds(&root, r#"file_value: {file: maps/cellar.map.yaml, path: "bounds[2]", equals: 8}"#);
    // **Each of those, one way wrong**: the value, the bound, the index, the path and the file.
    let said = fails(&root, "file_value: {file: data/gate.yaml, path: gate.health, equals: 100}");
    assert!(said.contains("40"), "the failure says the value it found: {said}");
    fails(&root, "file_value: {file: data/gate.yaml, path: gate.armour, max: 2.0}");
    fails(&root, r#"file_value: {file: data/gate.yaml, path: "gate.tags[1]", equals: stone}"#);
    fails(&root, "file_value: {file: data/gate.yaml, path: gate.mana, equals: 1}");
    let said = fails(&root, r#"file_value: {file: maps/nowhere.map.yaml, path: "bounds[0]", equals: 12}"#);
    assert!(said.contains("nowhere"), "the failure names the file nothing holds: {said}");
    // **A check that asks for nothing would pass on any file**, which is the do-nothing loophole.
    let said = fails(&root, "file_value: {file: data/gate.yaml, path: gate.health}");
    assert!(said.contains("no value"), "an empty question is refused by name: {said}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **The ground's height is read from the terrain file** at a point, with the core's own sampling.
#[test]
fn the_terrain_height_is_read_at_a_point() {
    let root = project("terrain");
    holds(&root, "terrain_height: {map: m, at: [4.0, -4.0], min_m: 1.9}");
    holds(&root, "terrain_height: {map: m, at: [-4.0, 4.0], max_m: 0.1}");
    // Between the hill's cell and its neighbour the ground is halfway, 1 m.
    holds(&root, "terrain_height: {map: m, at: [3.0, -4.0], min_m: 0.9, max_m: 1.1}");
    let said = fails(&root, "terrain_height: {map: m, at: [-4.0, -4.0], min_m: 1.5}");
    assert!(said.contains("0.000"), "the failure says the height it measured: {said}");
    fails(&root, "terrain_height: {map: m, at: [4.0, -4.0], max_m: 1.0}");
    let said = fails(&root, "terrain_height: {map: a_map_with_no_ground, at: [0.0, 0.0], min_m: 1.0}");
    assert!(said.contains("a_map_with_no_ground"), "the failure names the map: {said}");
    let said = fails(&root, "terrain_height: {map: m, at: [0.0, 0.0]}");
    assert!(said.contains("no bound"), "an empty question is refused by name: {said}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **A lake is found by where its centre is**, and how wide it is.
#[test]
fn a_lake_is_found_in_a_region() {
    let root = project("water");
    holds(&root, "water_lake: {map: m, x: [-4.0, -2.0], z: [2.0, 4.0], min_radius_m: 1.0}");
    fails(&root, "water_lake: {map: m, x: [2.0, 4.0], z: [-4.0, -2.0]}");
    let said = fails(&root, "water_lake: {map: m, x: [-4.0, -2.0], z: [2.0, 4.0], min_radius_m: 2.0}");
    assert!(said.contains("1.500"), "the failure says the radius it found: {said}");
    let said = fails(&root, "water_lake: {map: nowhere, x: [-4.0, 4.0], z: [-4.0, 4.0]}");
    assert!(said.contains("nowhere"), "a map with no water plan is named: {said}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **How many pieces stand on the map, and how many of one piece stand in a region.**
#[test]
fn pieces_are_counted_on_the_map_and_in_a_region() {
    let root = project("counts");
    holds(&root, "placement_total: {count: 5}");
    fails(&root, "placement_total: {count: 4}");
    holds(&root, "placements_within: {piece: plant_a, count: 3, x: [-4.0, 4.0], z: [3.0, 4.0]}");
    // The same three plants, asked for in a band they are not in, and for four.
    fails(&root, "placements_within: {piece: plant_a, count: 3, x: [-4.0, 4.0], z: [-4.0, 0.0]}");
    fails(&root, "placements_within: {piece: plant_a, count: 4, x: [-4.0, 4.0], z: [3.0, 4.0]}");
    // **A piece outside the region is a failure even when the count is right**, so six plants where five
    // are in the band and one is across the room is not a hedge.
    fails(&root, "placements_within: {piece: plant_a, count: 3, x: [-2.0, 4.0], z: [3.0, 4.0]}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **A piece can rest on a host named by its piece**, because a host an agent placed has an id the editor
/// minted, and a task cannot know it.
#[test]
fn a_piece_rests_on_a_host_named_by_its_piece() {
    let root = project("rests_on");
    holds(&root, "placement_rests_on: {piece: lamp, host_piece: table}");
    holds(&root, "placement_rests_on: {piece: lamp, host: table@4}");
    let said = fails(&root, "placement_rests_on: {piece: lamp, host_piece: desk}");
    assert!(said.contains("table@4"), "the failure says what it rests on: {said}");
    let said = fails(&root, "placement_rests_on: {piece: lamp}");
    assert!(said.contains("neither"), "naming no host is refused by name: {said}");
    let _ = std::fs::remove_dir_all(&root);
}
