# The sample game's benchmark suite: does the style help an agent? (SG-10, #409)

`cauldron-bench` (`bench/README.md`) run against the sample game's own project, with three tasks and two arms. Nothing in the gate runs it, and nothing here is a test of the engine: it is the instrument of the measurement pre-registered in `docs/research/2026-11-14-style-helps-an-agent.md`, which says what is run, how many times, and how the result is read.

| Path | What it is |
|---|---|
| `tasks/<id>/task.yaml` | One of the three tasks: `dress-the-market-area`, `write-the-gatekeepers-greeting`, `light-the-gate-at-night`. Each check is commented with the id of the Do or Don't line it scores (`# line L-Do3`), or `# guard` when it only stops a run that did nothing from scoring. |
| `tasks/variants.yaml` | The two arms. The runner reads the arms from `<project root>/bench/tasks/variants.yaml` whatever `--suite` says, so the tasks sit beside it. |
| `fixture/assets/grimoire/` | What the suite lays over a copy of the game's `assets/`: a market library (stalls, awnings, crates, lanterns, and five pieces that do not belong in the courtyard), the vocabulary those pieces use, and two registered data files, `lighting/gate` and `dialogue/gatekeeper`, which `data_set` edits. |
| `../design/` | The style being measured: the game's `style.md` and the courtyard level's. They are laid over the `with-style` arm's copy and over nothing else. |

```sh
# No model: the pipe, and the tasks. `nothing` must fail every task and `oracle` must pass every one, on a real editor.
cauldron-bench run --project games/sample --suite games/sample/bench/tasks --agent nothing
cauldron-bench run --project games/sample --suite games/sample/bench/tasks --agent oracle
cauldron-bench run --project games/sample --suite games/sample/bench/tasks --agent oracle --variant with-style
cauldron-bench run --project games/sample --suite games/sample/bench/tasks --agent oracle --variant without-style
```

A trial copies only `assets/`, so `design/` reaches the `with-style` arm by its `overlay`. A project's style is withheld from an agent until a person allows it, and a trial has no person: see the pre-registration's "What has to be true before the first run" for how the measurement grants it.
