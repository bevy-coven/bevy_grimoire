use bevy_grimoire_core::data_files::{Bounds, DataFiles, leaves, set_leaf};
use serde_json::json;

const FIX: &str = r#"# Mob tuning — hand written, keep the comments.
conviction_to_act: 100.0  # how sure a mob is before it moves
# a block comment ( with a paren
crowd:
  max: 12  # aligned
  label: 'mob // not a comment'
  panic: false
  leash: 4.5
  cooldown: null
waves: [ 1, 2, 3 ]
"#;

fn diff_span(a: &str, b: &str) -> (usize, usize) {
    let pre = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    let suf = a
        .bytes()
        .rev()
        .zip(b.bytes().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (pre, a.len() - suf.min(a.len() - pre))
}

#[test]
fn edit_changes_exactly_one_token() {
    let cases = [
        ("conviction_to_act", json!(150.0), "150.0"),
        ("crowd.max", json!(20), "20"),
        ("crowd.label", json!("a \"b\" // c"), "a \"b\" // c"),
        ("crowd.panic", json!(true), "true"),
        ("crowd.leash", json!(2.25), "2.25"),
        ("waves[1]", json!(9), "9"),
    ];
    for (path, v, tok) in cases {
        let out = set_leaf(FIX, path, &v, None).unwrap();
        let l = leaves(FIX).unwrap();
        let old = &l.iter().find(|(p, _)| p == path).unwrap().1;
        assert_eq!(
            out.len() as isize - FIX.len() as isize,
            tok.len() as isize - old.len() as isize,
            "{path}"
        );
        let (start, _) = diff_span(FIX, &out);
        assert_eq!(FIX[..start], out[..start]);
        let l2 = leaves(&out).unwrap();
        assert_eq!(l2.iter().find(|(p, _)| p == path).unwrap().1, tok, "{path}");
        // every other leaf and every comment unchanged
        assert_eq!(l.len(), l2.len());
        assert_eq!(out.matches("//").count(), FIX.matches("//").count());
        for ((p, a), (_, b)) in l.iter().zip(&l2) {
            if p != path {
                assert_eq!(a, b);
            }
        }
        assert_eq!(out.replacen(tok, old, 1), FIX, "{path}");
    }
}

#[test]
fn unknown_path_names_neighbours() {
    let e = set_leaf(FIX, "crowd.zzz", &json!(1), None).unwrap_err();
    assert!(
        e.contains("crowd.zzz") && e.contains("`crowd.max`") && e.contains("`crowd.label`"),
        "{e}"
    );
    assert!(!e.contains("conviction_to_act"), "{e}");
}

#[test]
fn kind_changes_are_refused() {
    let cases = [
        ("crowd.max", json!("12")),
        ("crowd.label", json!(3)),
        ("crowd.panic", json!(1)),
        ("conviction_to_act", json!("x")),
        ("crowd.cooldown", json!(3)),
        ("crowd.cooldown", json!(null)),
        ("crowd.leash", json!(null)),
        ("crowd.max", json!(null)),
    ];
    for (p, v) in cases {
        let e = set_leaf(FIX, p, &v, None).unwrap_err();
        assert!(e.contains(p), "{p}: {e}");
    }
}

#[test]
fn fraction_into_integer_is_refused() {
    let e = set_leaf(FIX, "crowd.max", &json!(3.5), None).unwrap_err();
    assert!(e.contains("crowd.max") && e.contains("whole"), "{e}");
    assert!(
        set_leaf(FIX, "crowd.max", &json!(3.0), None)
            .unwrap()
            .contains("max: 3"),
        "a whole number is written for an integer leaf"
    );
}

#[test]
fn out_of_bounds_is_refused() {
    let b = Bounds {
        min: Some(0.0),
        max: Some(200.0),
    };
    let e = set_leaf(FIX, "conviction_to_act", &json!(201.0), Some(&b)).unwrap_err();
    assert!(
        e.contains("conviction_to_act") && e.contains("bounds"),
        "{e}"
    );
    assert!(set_leaf(FIX, "conviction_to_act", &json!(200.0), Some(&b)).is_ok());
    assert!(set_leaf(FIX, "conviction_to_act", &json!(-0.1), Some(&b)).is_err());
}

#[test]
fn a_file_that_does_not_read_is_refused_before_an_edit_lands_in_it() {
    // **A file that does not read has no leaves**, so an edit is refused at the file and never at the leaf:
    // the stronger promise, because a splice into a file the reader cannot read edits a file whose value
    // nobody knows. The refusal names the field the read stopped at (`b`).
    let bad = "a: 5\nb: 'x\n";
    let e = set_leaf(bad, "a", &json!(6), None).unwrap_err();
    assert!(e.contains("did not read") && e.contains('b'), "{e}");
}

#[test]
fn registry_loads_and_reports() {
    let d = std::env::temp_dir().join(format!("grimoire_data_files_{}", std::process::id()));
    std::fs::create_dir_all(d.join("grimoire")).unwrap();
    std::fs::create_dir_all(d.join("data")).unwrap();
    let em = d.join("grimoire");
    assert_eq!(DataFiles::load(&em).unwrap(), None);
    let reg = "version: 1\nfiles:\n- id: tuning/mob\n  path: ../data/mob.yaml\n  note: tune\n  bounds:\n    conviction_to_act:\n      min: 0.0\n      max: 200.0\n";
    std::fs::write(em.join("data.yaml"), reg).unwrap();
    let e = DataFiles::load(&em).unwrap_err();
    assert!(
        e.contains("tuning/mob") && e.contains("../data/mob.yaml"),
        "{e}"
    );
    std::fs::write(d.join("data/mob.yaml"), FIX).unwrap();
    let r = DataFiles::load(&em).unwrap().unwrap();
    let f = r.get("tuning/mob").unwrap();
    assert_eq!(f.bounds["conviction_to_act"].max, Some(200.0));
    assert_eq!(std::fs::read_to_string(r.resolve(f)).unwrap(), FIX);
    std::fs::remove_dir_all(&d).unwrap();
}
