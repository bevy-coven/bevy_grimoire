# bevy_emerge

Editor, learned grammar and WFC for Bevy games. Workspace of crates under `crates/`; fixtures under `assets/`.

## Force pushes and deletions need the user's express permission

Never do any of the following unless the user has approved that specific action, by name, in the current conversation. A loop brief, a skill, a standing instruction such as "commit, push, PR, merge", or an earlier session's approval is not permission.

- Force-push in any form: `git push --force`, `--force-with-lease`, `--force-if-includes`, a `+` refspec, or pushing rewritten history. Deleting a remote branch or tag (`git push --delete`, `gh pr merge --delete-branch`, an API delete).
- Throwing away commits or uncommitted work: `git reset --hard`, `--merge` or `--keep`; `git checkout -- <path>` or `git restore` over changes you did not make; `git clean`; `git stash drop` or `clear`; `git branch -D`; `git worktree remove --force`; `git rebase` or `git commit --amend` on pushed commits; `git filter-repo` or `filter-branch`.
- Deleting files, directories, build caches, target dirs, models, datasets, database rows, issues, PRs, releases or cloud resources that you did not create in this session (`rm`, `rm -rf`, `git rm`, `cargo clean`, an API delete).

When one of these seems necessary, stop and ask. Name the exact command, what it destroys and whose work it touches. A commit that landed on the wrong branch is fixed with a new commit or a question, never by resetting a shared working tree or force-pushing. Files you created yourself in this session (scratch scripts, temp dirs, your own worktrees) you may delete.

## Version pin

Pinned to **Bevy 0.20.0** (`[workspace.dependencies] bevy`). Read the vendored source and migration guides, not bevy.org (which tracks `main`): `~/.cargo/registry/src/index.crates.io-*/bevy-0.20.0/` with `examples/` and `_release-content/migration-guides/`. `bevy_emerge_anim` and `bevy_devshot` name `0.20.0` themselves because they need `default-features = false`; bump them by hand with the workspace.

## Working with consumers

Games depend on these crates by path and the editor runs on a game as its project. The two evolve together:

- Changes a game needs are made here, in their own commits, with tests here. Never copy a crate into a game or patch it there.
- Every API change lists the consumer callsites it breaks.
- Before pushing, run `cargo test --workspace` here and `cargo test` in each consumer.
- If a consumer change depends on an unpushed commit here, the consumer's agents are told which commit (Sanctuary: `AGENTS_CHAT.md`).

Consumers: Sanctuary (`/Users/ladvien/sanctuary`, decision 0087).

### Iterating from a consumer

A game links only `bevy_emerge_core` (with `bevy_emerge_rng` and `bevy_emerge_solver`); the editor crates are a separate binary run from this repo on the game's project: `cargo run -p bevy_emerge_mapper -- <game-root> <map> --kit <kit>`. So an editor-only change never touches the game's build, while a change to core is picked up by the game's next `cargo` run through its path dependency, with nothing to copy or bump. `cargo run -q -p bevy_emerge_mapper --example boot_headless -- <game-root>` proves a game's project still opens without a window. The per-game loop (what to run, what to commit where) is in the game's `AGENTS.md`; Sanctuary's is under "bevy_emerge: the editor and WFC, evolving with Sanctuary".

## Blender is a tool we own, and it is not rebuilt here

**golem is ours** (`~/golem_mcp`, the MCP server that drives the open Blender: meshes, UVs, bakes,
painting, rigging, kitbashing, the `.glb` export). It is the same family as this repo, so the rule
under "Use the ladvien crate family" applies to it unchanged: **fix it at the source, in its own
repo, in its own commits with its own tests — never vendor, patch or re-implement it here.**

The boundary, measured on 2026-10-25 (`grep` for `kit.yaml`/`kitbash` under `crates/`: 0 hits; for
`bevy_emerge`/`library.ron` under golem: 0 hits):

| | golem | bevy_emerge |
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
- This repo *does* own reading a `.glb` well enough to place it (`bevy_emerge_core/src/glb.rs`,
  `import.rs`): it needs bounds, triangles, a front and a cell footprint at authoring time, inside
  the game's own project, with no Blender open. That is a different question from golem's, and the
  two implementations are allowed to differ — neither imports the other.

## Rules

- `bevy_emerge_core`, `bevy_emerge_rng` and `bevy_emerge_solver` are engine-free. `bevy_emerge_core/tests/engine_free.rs` fails on any `bevy*` dependency except the exact names in `SIBLING_DEPS`.
- Licence is MIT OR Apache-2.0 for every crate. Do not copy GPL code in from foundation_vs_slop's untouched crates.
- One plugin per domain; split by domain, not by kind.
- Do not hard-wrap markdown.
- Close any Bevy app you launch for a test.
