//! Convert a batch of project-file RON literals to YAML, through the library's own migration.
//!
//! Reads JSON `[[id, kind, text], ...]` on stdin and answers JSON `[[id, yaml], ...]` on stdout, plus
//! `errors` for the ones that would not convert. It exists so `tools/migrate/re-pin-fixtures.py` can move a
//! whole file's fixtures in one pass, using `migrate::written_as_yaml` rather than a text substitution.
//!
//! **`--moved-kinds` answers the kinds whose reader already parses YAML** (`migrate::READS_YAML`), one per
//! line, and reads no stdin. The re-pin script asks for it rather than carrying a copy of a list that moves
//! every commit of the cutover.
//!
//! Not a product path: nothing in the editor or the game calls it.

use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize)]
struct Ask {
    id: usize,
    /// **The file's path as its kind names it** (`kits.ron`, `maps/yard.map.ron`, `site/library.ron`): the
    /// kind is found from the path, so a caller names a path a kind's pattern matches.
    kind: String,
    text: String,
}

#[derive(Serialize)]
struct Done {
    id: usize,
    yaml: Option<String>,
    why: Option<String>,
}

fn main() {
    // **One question the script asks before it touches a file**: which kinds have moved. A copy of the list
    // in the script would be a second place a kind has to move, and the two disagreeing rewrites YAML under
    // a reader that still wants RON.
    if std::env::args().skip(1).any(|a| a == "--moved-kinds") {
        for kind in bevy_grimoire_core::migrate::READS_YAML {
            println!("{kind}");
        }
        return;
    }
    let mut input = String::new();
    use std::io::Read as _;
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("the batch could not be read from stdin");
        std::process::exit(2);
    }
    let asks: Vec<Ask> = match serde_json::from_str(&input) {
        Ok(asks) => asks,
        Err(e) => {
            eprintln!("the batch is not the JSON this wants: {e}");
            std::process::exit(2);
        }
    };
    let done: Vec<Done> = asks
        .into_iter()
        .map(|ask| match bevy_grimoire_core::migrate::written_as_yaml(ask.kind.as_str(), &ask.text, bevy_grimoire_core::migrate::COMMAND) {
            Ok((_, yaml)) => Done { id: ask.id, yaml: Some(yaml), why: None },
            Err(problem) => Done { id: ask.id, yaml: None, why: Some(problem.line()) },
        })
        .collect();
    match serde_json::to_string(&done) {
        Ok(text) => println!("{text}"),
        Err(e) => {
            eprintln!("the answer did not write: {e}");
            std::process::exit(2);
        }
    }
}
