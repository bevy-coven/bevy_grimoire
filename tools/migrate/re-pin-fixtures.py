#!/usr/bin/env python3
"""**Re-pin a source file's RON project-file fixtures to the YAML spelling** (GD-03, #365).

Every project file kind moved from RON to YAML, so a test's inline RON literal — a fixture written to a
`.yaml` path — no longer reads. This converts those literals through the type that reads them, using the
library's own migration (`migrate::written_as_yaml`), and never by text substitution: RON and YAML
disagree about `Some(...)`, `None`, a tuple and an enum variant's spelling, so a substitution produces
something that looks like YAML and holds a different value.

    python3 tools/migrate/re-pin-fixtures.py crates/bevy_grimoire_core/src/kits.rs [more files...]

**Which kind a literal is** comes from the type that reads it: a literal inside `Kits::parse(…)` is a
`kits`, one inside `yaml::from_str::<Map>(…)` is a `map`, and one assigned to a name and read later
(`let silent = r#"(…)"#; Kits::parse(silent)`) takes the kind of the first `Type::parse` after it in the
same test. `KIND_OF_TYPE` is the one map from a type to the kind `migrate::write_kind` knows it by, so this
cannot disagree with the migration about a kind. A literal whose kind cannot be found is left alone and
named on stderr — never guessed at.

**A literal that does not convert is reported and nothing is rewritten**, so a file is never left with half
its fixtures moved.

**Every kind's reader parses YAML now** (GD-03, #365): the cutover is complete, so a RON literal at a
project-file path is a fixture this build refuses, of any kind. There is no list of "moved kinds" to consult
and none to drift — one used to be printed by `--moved-kinds` from `migrate::READS_YAML`, and both went with
the cutover.

**A fixture this script cannot see is named.** It reads Rust string literals near a `Type::parse`, and that
misses one written through a helper — `fs::write(root.join("agent.yaml"), "(version: 9)")` names no type at
all. So every `fs::write` whose path ends `<kind>.yaml` for a kind this script knows, with a text that does not read as
YAML, is reported. The two rules differ in what they can do: a literal is converted, and a `fs::write` is
only ever named, because its text is a `format!` expression as often as a literal.
"""

import json
import re
import subprocess
import sys

# **The type a project file is, and the kind `migrate::write_kind` knows it by.**
KIND_OF_TYPE = {
    "Kits": "kits",
    "Vocabularies": "vocab",
    "Library": "library",
    "Policy": "policy",
    "Rules": "rules",
    "Compositions": "compositions",
    "Standards": "standards",
    "RepetitionRules": "repetition",
    "Rigs": "rigs",
    "Map": "map",
    "Notes": "notes",
    "Plan": "plan",
    "References": "board",
    "Relations": "relations",
    "Heightfield": "terrain",
    "WaterPlan": "water_plan",
    "WaterField": "water_field",
    "Volume": "volume",
    "DataFiles": "data_registry",
    "Family": "family_list",
    "AgentSettings": "agent",
    "PlayConfig": "play",
    "Marks": "destructible_marks",
    "Bake": "destructible_bake",
}

# **The path each kind's pattern names**, so the migration finds the kind from the file name it is given. A
# kind whose pattern is `<kit>/…` uses its kit-relative name, which is what a fixture holds.
FILE_OF_KIND = {
    "kits": "kits.ron", "vocab": "vocab.ron", "library": "site/library.ron", "policy": "site/project.ron",
    "rules": "rules.ron", "compositions": "compositions.ron", "standards": "standards.ron",
    "repetition": "repetition.ron", "rigs": "rigs.ron", "map": "maps/yard.map.ron", "notes": "maps/yard.notes.ron",
    "plan": "maps/yard.plan.ron", "board": "maps/yard.references.ron", "relations": "maps/yard.relations.ron",
    "terrain": "site/terrain/yard.ron", "water_plan": "site/water/yard.ron", "water_field": "site/water/yard.field.ron",
    "volume": "site/volume/yard.ron", "data_registry": "site/data.ron", "family_list": "family.ron",
    "agent": "agent.ron", "play": "play.ron", "destructible_marks": "destructible.ron",
    "destructible_bake": "destructible/site__cart.bake.ron",
}

# **The kind a written file's own stem names**, for the writes [`named_writes`] reports: `agent.yaml` is the
# `agent` kind, `editor.yaml` the editor's settings. A stem with no entry is not a kind this script has an
# opinion about, and is skipped rather than guessed at.
KIND_OF_STEM = {
    "kits": "kits", "vocab": "vocab", "library": "library", "project": "policy", "rules": "rules",
    "compositions": "compositions", "standards": "standards", "repetition": "repetition", "rigs": "rigs",
    "notes": "notes", "plan": "plan", "references": "board", "relations": "relations",
    "data": "data_registry", "family": "family_list", "agent": "agent", "play": "play",
    "destructible": "destructible_marks", "editor": "editor_settings", "checks": "checks",
}

# **A Rust string literal**: a raw string of any hash count, or a quoted one with escapes.
LITERAL = re.compile(r'r(?P<hashes>#*)".*?"(?P=hashes)|"(?:[^"\\]|\\.)*"', re.S)

# **A call that reads a project file**: `Type::parse(`, `ron::from_str::<Type>(`, `yaml::from_str::<Type>(`.
READS = re.compile(r'(?P<ty>[A-Za-z_][A-Za-z0-9_]*)::parse\('
                   r'|(?:ron::from_str|[a-z_:]*yaml::from_str)(?:::)?::<(?P<ty2>[A-Za-z_][A-Za-z0-9_]*)>\(')


def literal_text(lit):
    """The text a Rust string literal holds: a raw string's body, or a quoted string's unescaped body."""
    if lit.startswith("r"):
        # `r##"body"##`: the opening is `r` + hashes + `"`, the closing is `"` + the same hashes.
        hashes = lit.index('"') - 1
        opening = 1 + hashes + 1
        return lit[opening : len(lit) - 1 - hashes]
    body = lit[1:-1]
    out, i = [], 0
    while i < len(body):
        if body[i] == "\\" and i + 1 < len(body):
            out.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(body[i + 1], body[i + 1]))
            i += 2
            continue
        out.append(body[i])
        i += 1
    return "".join(out)


def as_literal(text):
    """`text` as a Rust raw string, with enough hashes that nothing inside closes it."""
    hashes = ""
    while f'"{hashes}' in text:
        hashes += "#"
    return f'r{hashes}"{text}"{hashes}'


def kind_near(source, start, end):
    """**The kind a literal is**, from the `Type::parse(...)` that reads it.

    A literal *inside* the call is answered by the call's own type. One assigned to a name is answered by
    the first such call after it, within the same test: a fixture is used by the statement that reads it.
    """
    # The enclosing call: a `Type::parse(` opening before `start` with no `;` between it and the literal.
    enclosing = None
    for match in READS.finditer(source, 0, start):
        if ";" in source[match.end() : start]:
            enclosing = None
        else:
            enclosing = match
    if enclosing is not None:
        kind = KIND_OF_TYPE.get(enclosing.group("ty") or enclosing.group("ty2") or "")
        if kind:
            return kind
    window = source[end : end + 4000]
    stop = window.find("#[test]")
    if stop != -1:
        window = window[:stop]
    for match in READS.finditer(window):
        kind = KIND_OF_TYPE.get(match.group("ty") or match.group("ty2") or "")
        if kind:
            return kind
    return None


# **A `fs::write` whose target is one of these names**, so a fixture the literal scan cannot see is at least
# named rather than silently missed. The path is any expression ending in the name.
WRITE = re.compile(r'fs::write\([^,]*?(?P<name>\w[\w.]*\.yaml)"\s*,\s*(?P<text>r?#*"[^"]*"#*)', re.S)


def named_writes(source):
    """**Every `fs::write` of a known kind's `.yaml` file whose text is not YAML**, as `(line, name)`.

    This is the blind spot of [`kind_near`], measured: `send_starts_the_notes_agent.rs` wrote `agent.yaml`
    with the RON text `"(version: 9)"`, which names no type anywhere, so the literal scan left it alone and
    the test failed with "unknown field `(version`" instead. A `fs::write` is never rewritten — its text may
    be a `format!` — but a reader is told where to look.
    """
    suspects = []
    for match in WRITE.finditer(source):
        name = match.group("name")
        # **The kind a written path names**, read from its last path segment with the extension off.
        stem = name.rsplit("/", 1)[-1].removesuffix(".yaml")
        kind = KIND_OF_STEM.get(stem)
        if kind is None:
            continue
        text = literal_text(match.group("text"))
        # A YAML mapping opens a line with `key:`; RON opens with `(`. That is the whole question for a
        # one-line literal, and it is enough to name the file rather than judge it.
        if text.lstrip().startswith("(") or text.lstrip().startswith("#!"):
            line = source[: match.start()].count("\n") + 1
            suspects.append((line, name))
    return suspects


def main(argv):
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    jobs, sites = [], []
    for path in argv[1:]:
        with open(path) as handle:
            source = handle.read()
        for line, name in named_writes(source):
            print(f"{path}:{line}: `{name}` is written with RON text, which names no type and so was not converted; fix it by hand", file=sys.stderr)
        for match in LITERAL.finditer(source):
            text = literal_text(match.group(0))
            # A project fixture opens as a RON struct, possibly after a `#![enable(...)]` attribute.
            if not text.lstrip().startswith(("(", "#!")):
                continue
            kind = kind_near(source, match.start(), match.end())
            if kind is None:
                print(f"{path}: a RON literal at byte {match.start()} names no type; left alone", file=sys.stderr)
                continue
            jobs.append({"id": len(jobs), "kind": FILE_OF_KIND[kind], "text": text})
            sites.append((path, match.start(), match.end(), kind))
    if not jobs:
        print("nothing to convert", file=sys.stderr)
        return 0
    done = subprocess.run(
        ["cargo", "run", "-q", "-p", "bevy_grimoire_migrate", "--example", "fixtures_to_yaml"],
        input=json.dumps(jobs),
        capture_output=True,
        text=True,
    )
    if done.returncode != 0:
        print(done.stderr.strip(), file=sys.stderr)
        return done.returncode
    answers = {a["id"]: a for a in json.loads(done.stdout)}
    # **A literal that does not read as its type is a test's own subject**, not a fixture: a test proving a
    # malformed file is refused holds malformed text on purpose. Those are left as they are and named, so a
    # reader can see the decision rather than wonder why a fixture stayed RON.
    refused = [(i, jobs[i]["kind"], a["why"]) for i, a in answers.items() if a["yaml"] is None]
    for index, kind, why in refused:
        print(f"left alone: a `{kind}` literal at {sites[index][0]}:{sites[index][1]} does not read — {why}", file=sys.stderr)
    keep = {i for i, _, _ in refused}
    by_path = {}
    for index, (path, start, end, _) in enumerate(sites):
        if index in keep:
            continue
        by_path.setdefault(path, []).append((start, end, answers[index]["yaml"]))
    for path, spans in by_path.items():
        with open(path) as handle:
            source = handle.read()
        for start, end, yaml in sorted(spans, reverse=True):
            source = source[:start] + as_literal(yaml) + source[end:]
        with open(path, "w") as handle:
            handle.write(source)
        print(f"{path}: {len(spans)} fixture(s) re-pinned")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
