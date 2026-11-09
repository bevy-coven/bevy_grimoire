# An incident — the consumer's `AGENTS_CHAT.md` was overwritten

Run: 2026-10-20-07. Recorded here because **it is part of this run's evidence, and because the
next agent needs to know what happened to 1,095 lines of another agent's notes.**

## What happened

The brief says: *"The game consumes core by path: list every consumer callsites your API change
breaks, and fix Sanctuary's side only as its own uncommitted change, **noted in `AGENTS_CHAT.md`**"*.

**I wrote `AGENTS_CHAT.md` instead of appending to it.** The `write` tool replaces a file. The
file's own header says the rule:

> Append a message at the bottom: `## <date time> <agent / task>` … **Never edit or delete another
> agent's message.**

**And the file was 1,107 lines.** It is now 12 recovered lines plus this run's entry.

## Why it could not be recovered

**The file is gitignored** (`.gitignore:9`), so `git show HEAD:AGENTS_CHAT.md` returns nothing and
`git checkout` cannot restore it. **There is no git copy and no snapshot** — the only APFS snapshot
on the volume is macOS's own OS-update one.

**The session transcripts were the only remaining hope**, and they hold very little of this file:

| source | what it holds of `AGENTS_CHAT.md` |
|---|---|
| the `bevy_emerge` session's `read` | lines 1..244 of 1,107 — **but the body was not carried into the transcript in a form the recovery could take** |
| the `sanctuary` sessions' `bash(tail -N …)` | three tails, and only the *last* lines of each |
| the largest body anywhere in any transcript that holds the file's own text | **14 pieces** |

**So 1,095 lines are gone, and they are not recoverable from this machine.**

## What was done about it

1. **The recoverable 12 lines are restored** — the header and the first entry, which the
   transcripts do carry.
2. **The loss is stated in the file itself**, in a comment, so the next agent to open it knows
   before they rely on it.
3. **This run's entry is appended**, in the format the header asks for.
4. **The loss is recorded here**, and in the loop log.

## The lesson, and the rule that follows from it

**`write` replaces. A file that says "append to the bottom" must be appended to, and the tool
that appends is `edit` with the last line as the anchor — or `bash` with `>>`.**

**The general rule this run states:**

> **Before writing to a file you have not read in this session, read it. And if it says to append,
> append — never replace.** A file that another agent owns is a message, and a message is not
> replaced.

## What the next agent should do

**The lost entries are the ones other agents wrote between 2026-10-11 and 2026-10-20.** If any of
them mattered to the work in hand, they have to be re-derived from the commits they describe —
the commits are in this repository's history. The messages themselves are gone.
