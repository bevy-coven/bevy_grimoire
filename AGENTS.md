# bevy_grimoire

The Cauldron (the editor), learned grammar and WFC for Bevy games. A grimoire is the book the Cauldron writes and a game loads (`bevy_grimoire`, `bevy_grimoire_core`, `assets/grimoire/`). Workspace of crates under `crates/`; fixtures under `assets/`.

## Force pushes and deletions need the user's express permission

Never do any of the following unless the user has approved that specific action, by name, in the current conversation. A loop brief, a skill, a standing instruction such as "commit, push, PR, merge", or an earlier session's approval is not permission.

- Force-push in any form: `git push --force`, `--force-with-lease`, `--force-if-includes`, a `+` refspec, or pushing rewritten history. Deleting a remote branch or tag (`git push --delete`, `gh pr merge --delete-branch`, an API delete).
- Throwing away commits or uncommitted work: `git reset --hard`, `--merge` or `--keep`; `git checkout -- <path>` or `git restore` over changes you did not make; `git clean`; `git stash drop` or `clear`; `git branch -D`; `git worktree remove --force`; `git rebase` or `git commit --amend` on pushed commits; `git filter-repo` or `filter-branch`.
- Deleting files, directories, build caches, target dirs, models, datasets, database rows, issues, PRs, releases or cloud resources that you did not create in this session (`rm`, `rm -rf`, `git rm`, `cargo clean`, an API delete).

When one of these seems necessary, stop and ask. Name the exact command, what it destroys and whose work it touches. A commit that landed on the wrong branch is fixed with a new commit or a question, never by resetting a shared working tree or force-pushing. Files you created yourself in this session (scratch scripts, temp dirs, your own worktrees) you may delete.

## Version pin

Pinned to **Bevy 0.20.0** (`[workspace.dependencies] bevy`). Read the vendored source and migration guides, not bevy.org (which tracks `main`): `~/.cargo/registry/src/index.crates.io-*/bevy-0.20.0/` with `examples/` and `_release-content/migration-guides/`. `bevy_grimoire_anim` and `bevy_devshot` name `0.20.0` themselves because they need `default-features = false`; bump them by hand with the workspace.

## Working with consumers

Games depend on these crates by path and the editor runs on a game as its project. The two evolve together:

- Changes a game needs are made here, in their own commits, with tests here. Never copy a crate into a game or patch it there.
- Every API change lists the consumer callsites it breaks.
- Before pushing, run `cargo test --workspace` here and `cargo test` in each consumer.
- If a consumer change depends on an unpushed commit here, the consumer's agents are told which commit (Sanctuary: `AGENTS_CHAT.md`).

Consumers: Sanctuary (`/Users/ladvien/sanctuary`, decision 0087).

### What a game needs, as an issue

A game asks for what it needs here through GitHub issues, and that is the only coupling between its loop and this repo's: neither loop reads the other's code or backlog to decide what to build, so any game can join the same way.

- **The game files** an issue on `bevy-coven/bevy_grimoire` with the labels `consumer`, `consumer:<game>` (create it the first time) and `bug` or `enhancement`: what the game needs and why, current behaviour, proposed API, edge cases, and acceptance criteria this repo can prove with its own test. It searches its open `consumer:<game>` issues first, so a need is filed once.
- **This repo's loop** (`.omp/commands/build-loop.md`) takes an open `consumer` issue right after bug issues and ahead of the owner's focus. It builds the general form (the game is the example, never a name in the API), proves it with a test here, and closes the issue only through a verified fix (`Fixes #N`). One it cannot work gets a comment saying what it needs.
- **The game waits** with `Needs: bevy_grimoire#N` on its own item and takes it up again when the issue is closed as completed and its checkout of this repo has the fix. Closed as not planned means declined: that is a question for the owner, not a retry.

### Iterating from a consumer

A game links only `bevy_grimoire_core` (with `bevy_grimoire_rng` and `bevy_grimoire_solver`); the editor crates are a separate binary run from this repo on the game's project: `cargo run -p bevy_cauldron -- <game-root> <map> --kit <kit>`, and with `--live` it writes every change through to disk for a game that watches its project (series S-003). So an editor-only change never touches the game's build, while a change to core is picked up by the game's next `cargo` run through its path dependency, with nothing to copy or bump. `cargo run -q -p bevy_cauldron --example boot_headless -- <game-root>` proves a game's project still opens without a window. The per-game loop (what to run, what to commit where) is in the game's `AGENTS.md`; Sanctuary's is under "bevy_grimoire: live editing, evolving with Sanctuary".

## The changelog: a line for every change, a clip for every visible one

The owner reads `CHANGELOG.md` to see what was added, so every shipped feature, enhancement or series step adds one line to it, newest first, written with `tools/changelog/changelog.py` (stdlib; `--help` for the rest):

- **A change someone can see gets a clip**: at most 20 s, 2 MB and 960 px, an MP4 with its still, in `docs/changelog/media/<date>-<id>.{mp4,png}`, stored by Git LFS (`.gitattributes`). For the editor: a storyboard run with `tools/demo/record.py`, then `python3 tools/demo/render.py <events.json> --out docs/changelog/media/<date>-<id>.mp4`. From any frames or video: `changelog.py clip --out ... --frames DIR` or `--video FILE`. Then `changelog.py add --id <id> --sha <sha> --title "<what it added>" --clip <date>-<id>`. The clip shows the result, not the code: what a person or an agent now sees happen.
- **A change nobody can see says so**: `--no-clip "<an API, a refactor, ...>; proved by <test>"`.
- `changelog.py check` before committing (the loop's gate runs it): every line has a clip or a reason, every clip is capped, has its still, is shown by a line and is in LFS. `changelog.py page --open` plays every clip in one page.
- A machine needs `git-lfs` (and `git lfs install`) before it commits a clip, or the clip goes into git itself; `check` refuses when it is missing.

## Use the ladvien crate family

The owner's Bevy crates are meant to be the heart of this editor (issues #96–#108, labelled `family`). Use them before writing a system or adding a third-party crate for the same job; doing otherwise needs a logged reason.

| Need | Crate |
|---|---|
| Sun, moon, sky, climate, weather, wind, clouds, fog, terrain, water, snow, rocks | `bevy_elements` |
| Procedural trees and forests that grow, with seasons | `bevy_flora` |
| SDF to mesh, caves, overhangs, carving | `isomesh` / `bevy_isomesh` |
| Fires, torches, a budgeted light pool | `bevy_lights` |
| Illuminance field for creature AI | `bevy_light_grid` |
| People with needs, ties, factions, Rhai scripts | `bevy_society` |
| Speech balloons, conversation graphs, dialog box | `bevy_speech_bubbles` |
| Menus, HUD meters, dialogs, tooltips, a UI skin, keyboard and gamepad navigation | `bevy_gooey` |
| Stigmergic fields, crowd avoidance | `bevy_stigmergy`, `bevy_orca` |
| Fracture, wounds, blood | `bevy_carnage` |
| An agent driving a running game over BRP | `bevy_fairy` (successor of `bevy_debugger_mcp`) |
| A frame on demand | `bevy_devshot` |

- **Configure, don't fork.** A crate's own serde types are the data, saved in the game's project and editable live. The editor never defines a second schema for a crate's settings.
- **Each crate is an optional cargo feature** of the editor, pinned by git revision on the crate's `main`: never a path into another checkout, and never a crates.io version, which would need a publish.
- **The two-way street.** A change the editor needs in a crate is made in that crate's repository, in its own commits with its own tests. It is requested there as an issue labelled `consumer` and `consumer:bevy_grimoire`, and the editor takes it by moving its pin once that issue is closed as fixed. Never patch, wrap or vendor a crate here. The loop's procedure is in `.omp/commands/build-loop.md`, "the two-way street".

## Blender is a tool we own, and it is not rebuilt here

**golem is ours** (`~/golem_mcp`, the MCP server that drives the open Blender: meshes, UVs, bakes,
painting, rigging, kitbashing, the `.glb` export). It is the same family as this repo, so the rule
under "Use the ladvien crate family" applies to it unchanged: **fix it at the source, in its own
repo, in its own commits with its own tests — never vendor, patch or re-implement it here.**

The boundary, measured on 2026-10-25 (`grep` for `kit.yaml`/`kitbash` under `crates/`: 0 hits; for
`bevy_grimoire`/`library.ron` under golem: 0 hits):

| | golem | bevy_grimoire |
|---|---|---|
| Subject | a **folder** of `.glb` — a pack | **one** `.glb` inside a project |
| Writes | `kit.yaml`, for kitbashing *in Blender* | `library.ron` `Descriptor`, for placing *on a map* |
| Frame | Blender z-up | Bevy y-up, metres on the map |
| Validation | the gate + the Blender analyzer | `import::measure` + `vocab::validate_library` |

**So the duplication to refuse is a second generator or a second Blender driver, not an importer.**
A golem `.glb` reaching the palette through `piece_import` is a bridge, not a copy: no code is
shared, and neither project knows the other's schema. Concretely:

- Need a mesh, a UV, a bake, a paint, a rig or a kitbash? **Ask golem over MCP.** Do not add Blender
  code, a `.blend` reader, or a glTF writer to this workspace.
- A golem behaviour that is wrong or missing is fixed in `~/golem_mcp`. If it is outside this
  iteration, log it in the backlog with golem named — as the family-crate rule says.
- This repo *does* own reading a `.glb` well enough to place it (`bevy_grimoire_core/src/glb.rs`,
  `import.rs`): it needs bounds, triangles, a front and a cell footprint at authoring time, inside
  the game's own project, with no Blender open. That is a different question from golem's, and the
  two implementations are allowed to differ — neither imports the other.

## Wiggum loops

A loop is one dumb driver (`tools/loop/wiggum.sh`) running one fresh `omp -p` per pass over a repo's own command, with the repository as the only memory. Reading a running loop: `tools/loop/README-wiggum.md`. **Starting a new one, or installing one on another machine: `docs/loop/RUNNING-A-LOOP.md`** — it carries the supervisor templates and the facts that each cost a cycle (macOS has no `timeout(1)`; one Xvfb per loop; `StartLimit*` belong in `[Unit]`; `WIGGUM_MAX` must be large because the driver exits 0 at the cap).

**Send a heavy compile to the device with room, never to the machine that happens to hold the checkout.** `python3 tools/loop/otel_host_pick.py --pick` reads Home Cloud's OTEL (Prometheus `192.168.1.104:9090`, dashboard `192.168.1.104:3000`) and prints the best ssh destination; it exits non-zero when nothing qualifies rather than suggesting a saturated host. Rank by idle cores, not raw load. See the runbook's load-balancing section.

## Rules

- `bevy_grimoire_core`, `bevy_grimoire_rng` and `bevy_grimoire_solver` are engine-free. `bevy_grimoire_core/tests/engine_free.rs` fails on any `bevy*` dependency except the exact names in `SIBLING_DEPS`.
- Licence is MIT OR Apache-2.0 for every crate. Do not copy GPL code in from foundation_vs_slop's untouched crates.
- One plugin per domain; split by domain, not by kind.
- Do not hard-wrap markdown.
- Close any Bevy app you launch for a test.

README and docs follow https://github.com/bevy-coven/.github/blob/main/DOCUMENTATION.md; a feature is not done until its README line and, if visible, its GIF exist.
