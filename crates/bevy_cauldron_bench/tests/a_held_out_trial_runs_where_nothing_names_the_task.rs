//! **While an agent runs a held-out task, nothing it can read names the task** (BM-04, #414, the manager's review).
//!
//! The agent's working directory is `<run>/held-out-<n>/trial-<k>/project`, and `omp` has `--cwd` and no
//! sandbox, so an agent can read every ancestor up to the run's directory and everything under them. This test
//! runs a real `cauldron-bench` with a held-out task whose id, prompt and check carry markers, and a stand-in
//! `omp` that, when it is launched as the agent, searches the run's directory for the markers and for the id in
//! any path, and writes what it found. The run's validity proofs (which hold the solution) have to be gone by
//! then, the folder has to be an ordinal, and no file may carry a marker.
//!
//! **It starts a real editor** (twice, for the validity proof, then once for the trial), so it is run on demand,
//! like the other tests of the harness that need one, not in the gate:
//! `cargo nextest run -p bevy_cauldron_bench --run-ignored only -E 'binary(a_held_out_trial_runs_where_nothing_names_the_task)'`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The marker in the held-out task's id.
const ID: &str = "zqheld-marker-task";
/// The marker in its prompt.
const PROMPT: &str = "ZQ-PROMPT-MARKER";
/// The marker in its check, which a leaked record would state as what it asked.
const CHECK: &str = "zq-check-marker";
/// How long the run is given to finish, seconds.
const RUN_S: u64 = 900;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|e| panic!("the repository root: {e}"))
}

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-held-run-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// **The editor and the server**, built here because a dev-dependency gives a library and no binary.
fn binaries() -> (PathBuf, PathBuf) {
    let bench = PathBuf::from(env!("CARGO_BIN_EXE_cauldron-bench"));
    let dir = bench.parent().unwrap_or_else(|| panic!("the bench binary has a directory")).to_path_buf();
    let built = Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "bevy_cauldron", "-p", "bevy_cauldron_mcp"])
        .current_dir(repo())
        .output()
        .unwrap_or_else(|e| panic!("cargo could not be run: {e}"));
    assert!(built.status.success(), "the editor and the server built:\n{}", String::from_utf8_lossy(&built.stderr));
    (dir.join("bevy_cauldron"), dir.join("bevy_cauldron_mcp"))
}

/// **A held-out directory holding one valid task**: a bed to add, against the repository's own fixture bedroom.
fn held_dir(dir: &Path) -> PathBuf {
    let held = dir.join("held");
    let task = format!(
        "id: {ID}\ntitle: {ID}-title\nsplit: held_out\ntags: [place]\nmap: bench_bedroom\nprompt: |\n  {PROMPT}: put a double bed in the bedroom.\n  Then hand the result back with grimoire_present.\nsetup: []\nsolution:\n  - op: placement_add\n    params:\n      piece: bed_double\n      at: [0.0, -1.39]\n      yaw: 0.0\n  - op: grimoire_present\n    params:\n      title: a bed\n      summary: A double bed stands in the bedroom. {CHECK}\n      specified:\n        - a bed\nchecks:\n  - placement_exists: {{piece: bed_double, count: 1}}\n  - handback_contains: [{CHECK}]\nbudget: {{wall_s: 600, tool_calls: 20}}\n"
    );
    write(&held.join(ID).join("task.yaml"), &task);
    // The task brings the scene it opens, which is the repository's own bedroom.
    let map = std::fs::read_to_string(repo().join("bench/fixture/assets/grimoire/maps/bench_bedroom.map.yaml"))
        .unwrap_or_else(|e| panic!("the fixture bedroom: {e}"));
    write(&held.join(ID).join("fixture/assets/grimoire/maps/bench_bedroom.map.yaml"), &map);
    held
}

/// **The stand-in `omp`**: it answers the harness's questions, and when it is launched as the agent it searches
/// the run's directory (three levels above its `--cwd`) for the markers and writes what it found.
fn stand_in(bin: &Path, report: &Path) {
    let script = format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
         models) echo \"$3\"; exit 0;;\n\
         auth)\n\
         \x20 case \"$2\" in\n\
         \x20 export) echo '{{\"version\":1,\"credentials\":[{{\"provider\":\"stand-in\"}}]}}'; exit 0;;\n\
         \x20 import) exit 0;;\n\
         \x20 esac;;\n\
         esac\n\
         cwd=''; prev=''\n\
         for a in \"$@\"; do if [ \"$prev\" = '--cwd' ]; then cwd=\"$a\"; fi; prev=\"$a\"; done\n\
         root=$(dirname \"$(dirname \"$(dirname \"$cwd\")\")\")\n\
         {{\n\
         echo \"cwd=$cwd\"\n\
         echo \"root=$root\"\n\
         echo '--- paths'\n\
         find \"$root\" | sort\n\
         echo '--- files holding a marker'\n\
         grep -rlF -e '{ID}' -e '{PROMPT}' -e '{CHECK}' \"$root\" 2>/dev/null\n\
         echo '--- end'\n\
         }} > '{}'\n\
         exit 0\n",
        report.display()
    );
    let omp = bin.join("omp");
    write(&omp, &script);
    let made = Command::new("chmod").arg("+x").arg(&omp).status().unwrap_or_else(|e| panic!("chmod: {e}"));
    assert!(made.success(), "the stand-in agent is made executable");
}

#[test]
#[ignore = "starts a Bevy editor; run on demand, not in the gate"]
fn a_held_out_trial_runs_where_nothing_names_the_task() {
    let (editor, server) = binaries();
    let dir = scratch("run");
    let bin = dir.join("bin");
    let report = dir.join("agent-report.txt");
    stand_in(&bin, &report);
    let held = held_dir(&dir);
    let home = dir.join("home");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let mut child = Command::new(env!("CARGO_BIN_EXE_cauldron-bench"))
        .args(["run", "--agent", "omp", "--run", "held-run", "--trials", "1", "--task", ID, "--held-out"])
        .arg(&held)
        .arg("--editor")
        .arg(&editor)
        .arg("--server")
        .arg(&server)
        .current_dir(repo())
        .env("HOME", &home)
        .env("PATH", path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cauldron-bench did not start: {e}"));
    let started = std::time::Instant::now();
    let status = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(RUN_S), "the run did not finish in {RUN_S} s");
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    let mut said = String::new();
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read>)].into_iter().flatten() {
        let mut stream = stream;
        let _ = std::io::Read::read_to_string(&mut stream, &mut said);
    }
    let _ = status;
    let seen = std::fs::read_to_string(&report)
        .unwrap_or_else(|e| panic!("the stand-in agent never ran ({e}), so the run said:\n{said}"));
    assert!(seen.contains("held-out-1"), "the agent's directory is filed by ordinal:\n{seen}");
    let (paths, leaks) = {
        let paths = seen.split("--- paths").nth(1).and_then(|s| s.split("--- files holding a marker").next()).unwrap_or_default();
        let leaks = seen.split("--- files holding a marker").nth(1).and_then(|s| s.split("--- end").next()).unwrap_or_default();
        (paths.to_owned(), leaks.trim().to_owned())
    };
    assert!(!paths.contains(ID), "a path under the run's directory names the task while the agent runs:\n{paths}");
    assert!(!paths.contains("validity"), "the validity proofs (which hold the solution) are gone before the trial:\n{paths}");
    assert!(leaks.is_empty(), "files under the run's directory carry the task's id, prompt or check text while the agent runs:\n{leaks}");
    // And what the run left behind afterwards is no different: the same search over the finished run.
    let run = home.join(".local/state/cauldron-bench/held-run");
    let after = Command::new("grep")
        .args(["-rlF", "-e", ID, "-e", PROMPT, "-e", CHECK])
        .arg(&run)
        .output()
        .unwrap_or_else(|e| panic!("grep: {e}"));
    let after = String::from_utf8_lossy(&after.stdout).into_owned();
    assert!(after.trim().is_empty(), "the finished run's files carry the task:\n{after}");
    let _ = std::fs::remove_dir_all(&dir);
}
