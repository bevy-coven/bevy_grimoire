#!/usr/bin/env python3
"""**Re-pin a test's RON fixture to the YAML spelling, through the type that reads it** (GD-03, #365).

The cutover moved every project file kind from RON to YAML. A test's inline RON literal is a fixture, and
re-pinning it is not a job for a search-and-replace: RON and YAML disagree about `Some(...)`, `None`, a
tuple and an enum variant's spelling, so a text substitution produces something that looks like YAML and
holds a different value.

So this converts through the type, with the library's own migration:

    python3 tools/migrate/fixture_to_yaml.py kits <type-path?>   # prints the YAML the fixture becomes

It runs `bevy_grimoire_core::migrate::written_as_yaml` — the same function `cauldron migrate` runs over a
project on disk — on a literal read from stdin, and prints the YAML it writes, or a refusal naming the
file. Nothing is written anywhere: a person or an agent puts the answer where the literal was, so the diff
shows exactly what moved.

    echo '(version: 1, command: ["game"])' | python3 tools/migrate/fixture_to_yaml.py play

**Why a script and not a test helper.** It is a development tool: nothing in the product calls it, and a
test that needed it would be a second way to build a fixture the crate already reads. It lives here so the
re-pin of the fixtures is one reviewed, repeatable step rather than a hand-rewrite.

**The type is named by the kind**, because the kind is what a file is: `--kind play` reads the literal with
`PlayConfig`'s RON reader and writes it as the YAML `play.yaml` holds. `migrate::write_kind` is the one list
of kinds and their types, so this cannot disagree with the migration about a kind.
"""

import os
import subprocess
import sys
import textwrap

# **The example that stands in for the type.** It is compiled into the crate that owns every kind's type, so
# the conversion here is the library's own and not a second implementation of it.
EXAMPLE = "fixture_to_yaml"
EXAMPLE_SOURCE = textwrap.dedent(
    """
    //! Convert one project file's RON text to YAML, through the library's own migration.
    //!
    //! The argument is the kind, and the RON is on stdin; the YAML comes out on stdout. This exists so
    //! `tools/migrate/fixture_to_yaml.py` re-pins a test's fixture with the same conversion a project on
    //! disk goes through (`migrate::written_as_yaml`), rather than with a text substitution that would
    //! change what the fixture holds.
    //!
    //! Not a product path: nothing in the editor or the game calls it.

    fn main() {
        let kind = std::env::args().nth(1).unwrap_or_default();
        let mut text = String::new();
        use std::io::Read as _;
        if std::io::stdin().read_to_string(&mut text).is_err() {
            eprintln!("the RON literal could not be read from stdin");
            std::process::exit(2);
        }
        match bevy_grimoire_core::migrate::written_as_yaml(&format!("{kind}.ron"), &text) {
            Ok((_, yaml)) => print!("{yaml}"),
            Err(problem) => {
                eprintln!("{}", problem.line());
                std::process::exit(1);
            }
        }
    }
    """
)


def main(argv):
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    kind = argv[1]
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True
    ).stdout.strip()
    if not root:
        print("run this inside the repository", file=sys.stderr)
        return 2
    example = os.path.join(root, "crates", "bevy_grimoire_core", "examples", EXAMPLE + ".rs")
    with open(example, "w") as handle:
        handle.write(EXAMPLE_SOURCE)
    done = subprocess.run(
        ["cargo", "run", "-q", "-p", "bevy_grimoire_core", "--example", EXAMPLE, "--", kind],
        input=sys.stdin.read(),
        capture_output=True,
        text=True,
    )
    sys.stdout.write(done.stdout)
    sys.stderr.write(done.stderr)
    return done.returncode


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
