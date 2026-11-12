#!/usr/bin/env python3
"""Run the editor's checks over a directory of captures (#112).

    python3 tools/decide/check.py captures/ --project . [--bar 0.9] [--ask]

Each check in the project's `checks.yaml` (and the built-in set, unless `--no-builtin`) asks a yes/no
question about the captures and answers with a probability. **The policy is not here**: it is
`bevy_oracle::run_checks`, so a test can prove it (`crates/bevy_oracle/tests/checks_that_earn_their_authority.rs`).
This script loads the evidence, calls the binary that holds that rule, and maps the verdict to an exit
code.

* exit 0 — no calibrated check found anything, or nothing was found at all;
* exit 1 — a check at or above `--bar` found something (the only failure a gate may act on);
* exit 2 — the run could not be made (no `checks.yaml`, a bad argument).

A capture directory holds `<check's image name>.png` for each name a check asks for. A missing one is
reported and never fails the run: an error is not evidence of a problem in a picture.

`--ask` is the only path that contacts a model. Without it the script lists the checks and the evidence it
would send, which is what a pass uses to prove the plumbing without a GPU.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("captures", help="directory of captures, named after the images each check asks for")
    ap.add_argument("--project", default=".", help="the project whose checks.yaml is read (default: .)")
    ap.add_argument("--bar", type=float, default=0.9, help="the agreement a check must have to block (default 0.9)")
    ap.add_argument("--ask", action="store_true", help="ask the model; without it, say what would be asked")
    ap.add_argument("--no-builtin", action="store_true", help="the project's checks only")
    ap.add_argument("--json", action="store_true", help="print the verdict as JSON")
    args = ap.parse_args(argv)

    checks = os.path.join(args.project, "checks.yaml")
    if not os.path.exists(checks) and args.no_builtin:
        print(f"no {checks} and --no-builtin was given: nothing to check", file=sys.stderr)
        return 2

    # **The runner is the crate that holds the rule.** `cargo run` keeps one implementation; a second
    # reading of `checks.yaml` here is the drift this repository refuses.
    cmd = [
        "cargo", "run", "-q", "-p", "bevy_oracle", "--bin", "decide_check", "--",
        "--captures", args.captures,
        "--project", args.project,
        "--bar", str(args.bar),
    ]
    if args.ask:
        cmd.append("--ask")
    if args.no_builtin:
        cmd.append("--no-builtin")
    if args.json:
        cmd.append("--json")
    if shutil.which("cargo") is None:
        print("cargo is not on PATH, so bevy_oracle's checker cannot run", file=sys.stderr)
        return 2
    return subprocess.call(cmd)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
