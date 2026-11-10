//! `decide_check` — run the project's checks over a directory of captures (#112).
//!
//! The policy lives in [`bevy_oracle::run_checks`]; this binary loads evidence, chooses a transport and
//! maps the verdict to an exit code. It is the half a shell script cannot show and a test cannot drive:
//! the file system and the network.
//!
//! **Without `--ask` nothing leaves the machine.** Every check is listed with the evidence it would send
//! and its calibration, so a pass can prove the plumbing on a host with no model — which is what this
//! repository's machine has most of the time.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy_oracle::{Checks, Config, Evidence, Http, Transport, Verdict, evidence_from, run_checks};

fn usage() -> String {
    "usage: decide_check --captures DIR [--project DIR] [--bar F] [--ask] [--no-builtin] [--json] [--facts FILE]\n\
     \x20      decide_check --calibrate SET.yaml [--check NAME] [--project DIR] [--model NAME]".to_owned()
}

struct Args {
    captures: PathBuf,
    project: PathBuf,
    bar: f64,
    ask: bool,
    no_builtin: bool,
    json: bool,
    /// **The JSON a `facts` check reads**, for a check about words rather than pictures: the What's new
    /// page's own sentences, say. One file for every check in the run, which is how a check about a
    /// page reads the page.
    facts: Option<PathBuf>,
    /// The labelled set to measure against, when this run is a calibration.
    calibrate: Option<PathBuf>,
    /// Which check to calibrate. The clip check unless named.
    check: String,
    model: String,
    /// **Where the calibration's record is written** (#112). Without it the run only prints, and a
    /// number printed to a terminal is a claim nobody can check: `tools/decide/check.py` reads this
    /// file to decide whether a check may block the gate, and
    /// `crates/bevy_oracle/tests/the_clips_calibration_is_recorded.rs` keeps the record's arithmetic
    /// honest. Written as RON, beside the labelled set it measured.
    out: Option<PathBuf>,
}

fn parse(argv: Vec<String>) -> Result<Args, String> {
    let mut a = Args {
        captures: PathBuf::new(),
        project: PathBuf::from("."),
        bar: 0.9,
        ask: false,
        no_builtin: false,
        json: false,
        facts: None,
        calibrate: None,
        out: None,
        check: "clip_shows_its_title".to_owned(),
        model: "clef-flash".to_owned(),
    };
    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--captures" => a.captures = PathBuf::from(it.next().ok_or_else(usage)?),
            "--project" => a.project = PathBuf::from(it.next().ok_or_else(usage)?),
            "--bar" => a.bar = it.next().ok_or_else(usage)?.parse().map_err(|e| format!("--bar: {e}"))?,
            "--ask" => a.ask = true,
            "--no-builtin" => a.no_builtin = true,
            "--json" => a.json = true,
            "--facts" => a.facts = Some(PathBuf::from(it.next().ok_or_else(usage)?)),
            "--calibrate" => a.calibrate = Some(PathBuf::from(it.next().ok_or_else(usage)?)),
            "--check" => a.check = it.next().ok_or_else(usage)?,
            "--model" => a.model = it.next().ok_or_else(usage)?,
            "--out" => a.out = Some(PathBuf::from(it.next().ok_or_else(usage)?)),
            "-h" | "--help" => return Err(usage()),
            other => return Err(format!("unknown argument `{other}`\n{}", usage())),
        }
    }
    // A calibration names the set instead of a capture directory: the set is what it measures.
    if a.captures.as_os_str().is_empty() && a.calibrate.is_none() {
        return Err(usage());
    }
    Ok(a)
}

/// A capture as the data URL the API takes, through the one encoder the crate shares.
///
/// A capture that is not there is `Ok(None)` rather than an error, so the check's own
/// `MissingEvidence` names it: one place decides what a check needs, and it is the check.
fn capture(dir: &Path, name: &str) -> Result<Option<String>, String> {
    let path = dir.join(format!("{name}.png"));
    let Ok(bytes) = std::fs::read(&path) else { return Ok(None) };
    Ok(Some(bevy_oracle::data_url(&bytes)))
}

fn main() {
    let args = match parse(std::env::args().skip(1).collect()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    // **The built-in set, then the project's, on top.** A project may re-point a built-in check's
    // threshold without losing the rest, which is what `merged` is for.
    //
    // **`for_project` already merges**, so `--no-builtin` cannot be honoured by calling it: it would
    // return the built-in set either way, and a flag that silently does nothing is the defect this
    // crate refuses elsewhere. The project file is therefore read here, and the merge is stated once.
    let project = match Checks::for_project(&args.project) {
        Ok(all) if args.no_builtin => {
            // The project's own entries are the tail `for_project` added; without a file it added none.
            let builtin = Checks::builtin();
            let own: Vec<_> =
                all.checks.into_iter().filter(|c| !builtin.checks.iter().any(|b| b.name == c.name)).collect();
            if own.is_empty() {
                Checks { version: bevy_oracle::CHECKS_VERSION, checks: Vec::new() }
            } else {
                Checks { version: bevy_oracle::CHECKS_VERSION, checks: own }
            }
        }
        Ok(all) => all,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    let transport = Http::new(Config::from_env());
    let evidence_for = |check: &bevy_oracle::Check| -> Result<Evidence, String> {
        let mut images = BTreeMap::new();
        for name in &check.images {
            match capture(&args.captures, name)? {
                Some(url) => {
                    images.insert(name.clone(), url);
                }
                None => {
                    return Err(format!("no {name}.png in {}", args.captures.display()));
                }
            }
        }
        // **A `facts` check is handed the facts**, which is the whole point of the flag: without it a
        // check about words had no evidence to read and could only report `MissingEvidence`.
        let facts = match &args.facts {
            Some(path) => Some(
                serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
                    .map_err(|e| format!("{} is not JSON: {e}", path.display()))?,
            ),
            None => None,
        };
        Ok(evidence_from(&images, facts.clone()))
    };

    // **A calibration run** (#112): measure the check against a labelled set and print the figure the
    // check's `calibration` field takes. This is the one path that needs a served model, and the reason
    // the agreement was "not measured yet" for so long: `score_set` had no caller.
    if let Some(set) = &args.calibrate {
        let samples = match bevy_oracle::load_labelled(set) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        };
        let check = bevy_oracle::clip_check(&args.check);
        let scored = match bevy_oracle::score_set(&check, &samples, &transport) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        };
        // **A calendar date, not a day count.** This wrote `format!("+{today} days")` from the days
        // since the epoch, which RON parses as a *duration* and which reads in the record as
        // `date: "+20760 days"` — caught by
        // `tests/the_clips_calibration_is_recorded.rs`, the test this record exists for.
        let date = {
            let days = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() / 86_400)
                .unwrap_or(0) as i64;
            // A civil date from days since the epoch (Howard Hinnant's `civil_from_days`), so the
            // record carries no dependency for one line of arithmetic.
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
        };
        let calibration = match bevy_oracle::calibrate(&args.check, &args.model, &date, &scored) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        };
        println!(
            "{:22} agreement {:.1}% on {} samples, threshold {:.3}, model {}",
            calibration.check,
            calibration.agreement * 100.0,
            calibration.set_size,
            calibration.threshold,
            calibration.model
        );
        // **The record, when one was asked for.** See `Args::out`: the printed line is for the person
        // running the calibration, and the file is what a test and a gate can read afterwards.
        if let Some(out) = args.out.as_deref() {
            let write = || -> Result<(), String> {
                let text = serde_yaml_ng::to_string(&calibration).map_err(|e| e.to_string())?;
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                std::fs::write(out, format!("{text}\n"))
                    .map_err(|e| format!("{}: {e}", out.display()))
            };
            match write() {
                Ok(()) => println!("recorded to {}", out.display()),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            }
        }
        if args.json {
            match serde_json::to_string_pretty(&calibration) {
                Ok(text) => println!("{text}"),
                Err(e) => eprintln!("{e}"),
            }
        }
        std::process::exit(0);
    }

    if args.ask {
        let verdict = run_checks(&project, &transport, &evidence_for, args.bar);
        print_verdict(&verdict, args.json);
        std::process::exit(if verdict.fails() { 1 } else { 0 });
    }

    // **What would be asked, and what may block.** No network: this is the pass a host without a model
    // runs, and it is also what makes the "uncalibrated checks only print" rule visible.
    let mut listed = 0;
    for check in &project.checks {
        let authority = if check.calibration.as_ref().is_some_and(|c| c.is_blocking(args.bar)) {
            "blocking"
        } else {
            "advisory"
        };
        let images = if check.images.is_empty() { "facts".to_owned() } else { check.images.join(", ") };
        println!("{:10} {}: {images} — {}", authority, check.name, check.question);
        listed += 1;
    }
    if listed == 0 {
        eprintln!("no checks: neither a project checks.yaml nor the built-in set");
        std::process::exit(2);
    }
    println!("\n{} check(s); nothing was asked. Pass --ask to send them to {}", listed, transport.endpoint());
}

fn print_verdict(verdict: &Verdict, json: bool) {
    if json {
        let findings: Vec<_> = verdict
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "check": f.check, "p_yes": f.p_yes, "threshold": f.threshold,
                    "found": f.found,
                    "authority": if verdict.blocking.iter().any(|b| b.check == f.check) { "blocking" } else { "advisory" },
                })
            })
            .collect();
        let errors: Vec<_> =
            verdict.errors.iter().map(|(n, w)| serde_json::json!({"check": n, "why": w})).collect();
        println!(
            "{}",
            serde_json::json!({"fails": verdict.fails(), "bar": verdict.bar, "findings": findings, "errors": errors})
        );
    } else {
        print!("{}", verdict.report());
    }
}
