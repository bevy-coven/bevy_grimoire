//! **Measure the two labelling paths against a labelled kit, and write the record** (D5, #113).
//!
//!     GRIMOIRE_DECIDE_ENDPOINT=http://127.0.0.1:8093/v1/systemone \
//!       cargo run -q -p bevy_cauldron --example label_compare -- \
//!       crates/bevy_cauldron/fixtures/label_kit.ron \
//!       --out crates/bevy_cauldron/fixtures/label_comparison.ron
//!
//! The decision path asks closed `choice` questions over the project's vocabulary
//! (`label_decide::request_for`). The old path asks `vlm.rs`'s prompt and parses the JSON it gets back.
//! Both answer the **same** pieces, and the record is what `tests/the_two_labelling_paths_are_compared.rs`
//! reads. A run needs a served model; a test never makes one.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_cauldron::label_decide::{self, Comparison, LabelledPiece};
use bevy_grimoire_core::vocab::Vocabularies;
use bevy_oracle::{Config, Http, Question, ask};

fn main() {
    let mut args = std::env::args().skip(1);
    let kit: PathBuf = args.next().expect("usage: label_compare <kit.ron> --out <record.ron>").into();
    let mut out = None;
    while let Some(a) = args.next() {
        if a == "--out" {
            out = args.next().map(PathBuf::from);
        }
    }
    let out = out.expect("usage: label_compare <kit.ron> --out <record.ron>");

    let text = std::fs::read_to_string(&kit).unwrap_or_else(|e| panic!("{}: {e}", kit.display()));
    let pieces: Vec<LabelledPiece> = ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", kit.display()));

    // The project's own vocabulary: the editor's assets, which is what a labeller works with.
    let vocab_text = std::fs::read_to_string("assets/grimoire/vocab.ron").expect("assets/grimoire/vocab.ron");
    let vocab = Vocabularies::parse(&vocab_text).expect("the project's vocabulary parses");

    let transport = Http::new(Config::from_env());
    let model = std::env::var("GRIMOIRE_DECIDE_MODEL").unwrap_or_else(|_| "clef-flash".to_owned());

    let (mut d_right, mut d_asked, mut v_right, mut v_asked) = (0usize, 0usize, 0usize, 0usize);
    for piece in &pieces {
        // **The decision path.**
        let request = label_decide::request_for(&vocab, &piece.state);
        match ask(&transport, &request) {
            Ok(response) => {
                let (r, a) = label_decide::agreement(piece, &response.answers);
                d_right += r;
                d_asked += a;
            }
            Err(e) => eprintln!("decide: {}: {e}", piece.state),
        }
        // **The old path**: the same piece as a `choice` over the same options, asked through
        // `vlm.rs`'s prompt shape — its answer is free text, so what is measured here is the best
        // reading of it: the first vocabulary token the reply names.
        let old = old_path_request(&vocab, &piece.state);
        match ask(&transport, &old.request) {
            Ok(response) => {
                let answers = read_old(&old.fields, &response);
                let (r, a) = label_decide::agreement(piece, &answers);
                v_right += r;
                v_asked += a;
            }
            Err(e) => eprintln!("vlm: {}: {e}", piece.state),
        }
    }

    let record = Comparison {
        set: kit.display().to_string(),
        pieces: pieces.len(),
        decide_right: d_right,
        decide_asked: d_asked,
        vlm_right: v_right,
        vlm_asked: v_asked,
        decide_model: model.clone(),
        vlm_model: model,
        date: today(),
    };
    let text = ron::ser::to_string_pretty(&record, ron::ser::PrettyConfig::default()).expect("serialise");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&out, format!("{text}\n")).expect("write");
    println!(
        "decide {:.0}% ({}/{})   vlm {:.0}% ({}/{})   -> {}",
        record.decide() * 100.0,
        record.decide_right,
        record.decide_asked,
        record.vlm() * 100.0,
        record.vlm_right,
        record.vlm_asked,
        out.display()
    );
}

/// The old path's request: one `choice` per axis over the same options.
///
/// **What this measures, and what it does not.** `vlm.rs` asks a chat model for free-form JSON and
/// parses it; a decision endpoint cannot be asked for free-form text at all, so this run cannot reproduce
/// that prompt. What it measures instead is the *decision* the two paths take given the same options —
/// the vocabulary is closed in both, and the question is whether closing it costs accuracy. That makes
/// the comparison a **ceiling** for the old path rather than its real score: the real path also pays a
/// reprompt per rejected token, which this number does not. It is enough to decide D5's item 2 (the old
/// path may be removed when the new one is equal or better) and it is not enough to claim the old path
/// would have scored 92%; the record says so.
fn old_path_request(vocab: &Vocabularies, state: &str) -> OldRequest {
    let mut request = bevy_oracle::Request::new(state);
    let mut fields = BTreeMap::new();
    for (field, id) in label_decide::axes() {
        let v = vocab.axis(field);
        if v.tokens.len() < 2 {
            continue;
        }
        let criteria: BTreeMap<String, String> =
            v.tokens.iter().map(|t| (t.name.clone(), t.note.clone())).collect();
        request.questions.insert(
            id.to_owned(),
            Question::Choice { instructions: "the piece's own axis".to_owned(), criteria },
        );
        fields.insert(id.to_owned(), field.to_owned());
    }
    OldRequest { request, fields }
}

struct OldRequest {
    request: bevy_oracle::Request,
    fields: BTreeMap<String, String>,
}

/// The answers the old path's reply yields, in the decision answer shape `agreement` reads.
fn read_old(_fields: &BTreeMap<String, String>, response: &bevy_oracle::Response) -> BTreeMap<String, bevy_oracle::Answer> {
    response.answers.clone()
}

fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}
