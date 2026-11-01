# How bevy_grimoire's documentation is written

Research found with Home Still (`distill_search`, 2026-10-23), and the rules it gives this project. Every page under `docs/guide/` and the README follow them. Cite the sources here, never in the user docs.

## What the research says

1. **The README is the homepage, and often the only page read.** Assume it is the only documentation a user reads. It must hold what the project does, how to install and run it, a quickstart, where the full docs are, the licence, and how to test it. The quickstart is a "dating profile": it shows strengths, gives a feel, and entices. Check it by handing it to someone who has never used the tool. (Lee 2018, *Ten simple rules for documenting scientific software*, 10.1371/journal.pcbi.1006561, rules 3 and 4.)
2. **Say why, not only what and how.** A study of 4,226 README sections from 393 repositories found "what" and "how" common, while "why" (purpose, what it does better than alternatives) appeared in only about a quarter, and status ("when") was rarer still. Those are the sections that set a project apart. Readers asked for usage, installation, prerequisites, licence, purpose and audience, known problems, and screenshots. (Prana et al. 2018, *Categorizing the Content of GitHub README Files*, 10.1007/s10664-018-9660-3.)
3. **Show, then tell; examples have no upper limit.** A working example is a starting point a user changes for their own needs. Each example should show a different aspect. (Lee 2018, rule 2.)
4. **Minimalism.** Get people doing a real task at once, lean on what they already know, keep it short (the Minimal Manual was a quarter of the official length and users learned more), treat errors as normal and say how to recover, and leave room to explore. (Carroll 2014, *Creating Minimalist Instruction*, 10.14434/ijdl.v5i2.12887.)
5. **Teach at the point of need, especially in complex tools.** Instructions given when they are needed, instead of in an up-front manual, raised play time by 16% and progress by 40% in Foldit, the complex and unconventional game in the study, and made no difference in two simpler games, which players learned by experimenting. A world editor is the complex case. So the docs are organised by the task in hand, and the editor's own help (holding K shows every key on the tab you are on) is the first help a person meets. (Andersen et al. 2012, *The impact of tutorials on games of varying complexity*, 10.1145/2207676.2207687.)
6. **Short videos, one task each, showing the end state first.** In software-training videos, a preview that showed the task's start and end state was watched over 90% of the time; later, longer or redundant videos lost viewers (first demonstrations about 70%, reviews 42%). Keep each clip well under three minutes, one task, placed beside the task it shows. (van der Meij & van der Meij 2017, 10.1007/s11423-017-9560-z; 2016, 10.1007/s11251-016-9394-9.)
7. **Out-of-date documentation is the most common documentation defect.** Industry bug data traces documentation defects mainly to obsolete, ambiguous or missing content and to code changes the docs did not follow; references to code elements in repository docs go stale often and stay stale for long periods. Fewer, shorter pages that are checked against the code stay true; generated references stay true by construction. (Rajapakse et al. 2023, *Towards identifying and minimizing customer-facing documentation debt*, 10.1109/techdebt59074.2023.00015; Tan et al. 2023, *Detecting outdated code element references in software repository documentation*, 10.1007/s10664-023-10397-6.)
8. **Know the reader and the purpose; use graphics; be clear and precise.** (Alzahrani 2024, *Software Systems Documentation: A Systematic Review*, 10.14569/ijacsa.2024.0150816.)

## The rules for this project

**Readers.** A Bevy game developer who builds levels from a kit of meshes, and the AI agent they work with. They know games, Bevy, and roughly what a level editor does. They don't know this editor's words (door, kit, stamp, composition, bash), so each word is defined the first time a page uses it.

**Structure.**
- `README.md`: what it is and why it is useful (with the hero GIF), what works today and what doesn't, a quickstart that works on a fresh clone, the common tasks as one-line links into the guide, how a game uses the output, the crates, docs, contributing, licence.
- `docs/guide/`: one page per job a person does, named for the job. Each task in a page has the same shape:
  1. **Goal**: one sentence, in the reader's words.
  2. **The GIF or still** that shows the end state, if one exists.
  3. **Steps**: numbered, at most about seven, each one action.
  4. **What you see**: how to tell it worked.
  5. **If it goes wrong**: the likely refusals and how to recover.
- Reference material (keys, data files, MCP tools) lives on its own pages, after the tasks, never in the middle of them. `docs/mcp-tools.md` is generated from the server and must not be hand-edited.

**Voice.** Second person, present tense, active voice, short sentences. Concrete names: the exact key, the exact label on screen, the exact file. No marketing words. No history ("now", "since cycle 6", "was fixed"): the page describes the editor as it is. No internal ids (B-044, E-042), no commit hashes, no source line numbers in user docs; data file paths are fine.

**Truth.**
- Every key, label, operation name, tool name, file name and behaviour on a page is checked against the code before it is written: keys against the `BINDINGS` table in `crates/bevy_cauldron/src/keys.rs`; operations against the registry in `crates/bevy_cauldron/src/ops.rs`; tools against `docs/mcp-tools.md`.
- A limitation is stated plainly where a reader would hit it, not hidden.
- If something can't be checked, leave it out.

**Length.** Short. A task the reader can finish in a minute gets a paragraph and a few steps, not a page. Do not hard-wrap lines.

**Media.** Only the GIFs in `docs/media/` that show exactly what the text next to them says. Each GIF sits beside its task, with alt text that describes what happens in it.
