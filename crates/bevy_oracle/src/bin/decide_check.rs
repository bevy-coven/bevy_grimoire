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
    "usage: decide_check --captures DIR [--project DIR] [--bar F] [--ask] [--no-builtin] [--json]".to_owned()
}

struct Args {
    captures: PathBuf,
    project: PathBuf,
    bar: f64,
    ask: bool,
    no_builtin: bool,
    json: bool,
}

fn parse(argv: Vec<String>) -> Result<Args, String> {
    let mut a = Args {
        captures: PathBuf::new(),
        project: PathBuf::from("."),
        bar: 0.9,
        ask: false,
        no_builtin: false,
        json: false,
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
            "-h" | "--help" => return Err(usage()),
            other => return Err(format!("unknown argument `{other}`\n{}", usage())),
        }
    }
    if a.captures.as_os_str().is_empty() {
        return Err(usage());
    }
    Ok(a)
}

/// A capture as the data URL the API takes: PNG bytes, base64, `data:image/png;base64,…`.
///
/// A capture that is not there is `Ok(None)` rather than an error, so the check's own
/// `MissingEvidence` names it: one place decides what a check needs, and it is the check.
fn capture(dir: &Path, name: &str) -> Result<Option<String>, String> {
    let path = dir.join(format!("{name}.png"));
    let Ok(bytes) = std::fs::read(&path) else { return Ok(None) };
    Ok(Some(format!("data:image/png;base64,{}", b64(&bytes))))
}

/// Base64, standard alphabet with padding — the one the API documents for an embedded image.
fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
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
        Ok(evidence_from(&images, None))
    };

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
        eprintln!("no checks: neither a project checks.ron nor the built-in set");
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
