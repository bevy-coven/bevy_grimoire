//! **Does a converted fixture read to the same value as its RON original?** (GD-03, #365; review M6)
//!
//!     git diff --name-status -M origin/main HEAD | awk '$1=="D" && $2 ~ /\.ron$/ {print $2}' |
//!         cargo run -q -p bevy_grimoire_migrate --example same_value_as_the_ron_original -- origin/main
//!
//! Each line on stdin is the path of a RON file at the base commit. The RON is read from `git show <base>:<path>` into a value
//! tree (`ron_tree`, no type), the YAML is the file beside it with `.yaml` and read by the project's one YAML reader, and the two
//! are compared as values: a null equals an absent key, a key only the YAML has is a field the type defaults (a typed write prints it), and `1` is `1.0`; a key only the RON has, or a different value, is a difference.
//! Prints `same` or the first path where they differ, and exits 1 when any differ. A report, not a test: a file whose RON this
//! does not read is named and counted.

use std::io::BufRead as _;
use std::process::Command;

use bevy_grimoire_migrate::ron_tree;
use serde_json::Value;

/// Drop nulls and make integers floats, so only a real difference shows.
fn normal(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().filter(|(_, x)| !x.is_null()).map(|(k, x)| (k.clone(), normal(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(normal).collect()),
        Value::Number(n) => n.as_f64().map_or(Value::Null, Value::from),
        other => other.clone(),
    }
}

fn first_difference(path: &str, a: &Value, b: &Value) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => {
                        if let Some(d) = first_difference(&format!("{path}.{k}"), p, q) {
                            return Some(d);
                        }
                    }
                    // A key only the YAML has is a field the type fills with its default (a typed write prints it): the same value.
                    (None, Some(_)) => {}
                    (Some(_), None) => return Some(format!("{path}.{k} is in the RON and missing from the YAML")),
                    (None, None) => {}
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path} has {} and {} items", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().find_map(|(i, (p, q))| first_difference(&format!("{path}[{i}]"), p, q))
        }
        _ => (a != b).then(|| format!("{path}: RON `{a}` YAML `{b}`")),
    }
}

fn main() {
    let base = std::env::args().nth(1).unwrap_or_else(|| "origin/main".to_owned());
    let (mut same, mut different, mut unread) = (0, 0, 0);
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let ron_path = line.trim();
        if ron_path.is_empty() {
            continue;
        }
        let yaml_path = format!("{}.yaml", ron_path.trim_end_matches(".ron"));
        let shown = Command::new("git").args(["show", &format!("{base}:{ron_path}")]).output();
        let Ok(shown) = shown else { println!("{ron_path}: git is not available"); unread += 1; continue };
        let ron_text = String::from_utf8_lossy(&shown.stdout).into_owned();
        let yaml_text = std::fs::read_to_string(&yaml_path);
        let (Ok(tree), Ok(yaml_text)) = (ron_tree::parse(&ron_text), yaml_text) else {
            println!("{ron_path}: not compared (the RON does not read as a tree, or {yaml_path} is not there)");
            unread += 1;
            continue;
        };
        let Ok(yaml) = bevy_grimoire_core::yaml::to_json(&yaml_text, &yaml_path) else {
            println!("{ron_path}: {yaml_path} does not read");
            different += 1;
            continue;
        };
        match first_difference("", &normal(&tree.to_json()), &normal(&yaml)) {
            None => {
                println!("{ron_path}: same");
                same += 1;
            }
            Some(d) => {
                println!("{ron_path}: DIFFERENT at {d}");
                different += 1;
            }
        }
    }
    println!("{same} same, {different} different, {unread} not compared");
    if different > 0 {
        std::process::exit(1);
    }
}
