//! **A lesson: one line an agent learned, for the next agent's guide** (#351, AG-06).
//!
//! A wish (#353) says what the editor lacked. A lesson says what the *task* taught: the one thing the
//! next agent to work in this project should know, written by the agent that learned it and kept by
//! the person who read it. It is appended to the project's own `.dark_factory/AGENTS.md`, under
//! [`LESSONS_HEADING`], because that file is the one an agent is already given.
//!
//! # The agent proposes, the editor writes
//!
//! **The tool's call writes nothing.** It queues the proposal where a person can read it, beside the
//! wishes. Only **Keep**, pressed by a person, appends the line and (when the guide was already
//! allowed) grants the new content. An agent that could write the guide it follows could instruct
//! itself, which is the rule `trust` states for every grant.
//!
//! # One line, refused at the door, never trimmed
//!
//! A lesson is **one line** of at most [`LESSON_MAX_CHARS`] characters, with a newline refused rather
//! than flattened: a "line" that silently became two would be a paragraph in a list of lines, and
//! [`append`] could no longer promise that the kept text is the proposed text. The reason names the
//! field and the limit, as a wish's does, because the agent is still there to shorten what it wrote.
//!
//! # Appending is one place, and it is arithmetic over a string
//!
//! [`append`] is the whole writer's rule — find [`LESSONS_HEADING`], add the line after the section's
//! last line (or make the section at the end) — so the editor's Keep and a test's expectation cannot
//! disagree about what a kept lesson looks like. It takes and returns a `String`: no file is opened
//! here, because **the editor owns the write** and a core module that opened a file would be a second
//! writer of the guide.
//!
//! Engine-free: `std` and `serde` only.

use serde::{Deserialize, Serialize};

/// **The longest a lesson may be**, in characters. The same order as a wish's `tool`: a line read in a
/// list, not a paragraph.
pub const LESSON_MAX_CHARS: usize = 280;

/// **The same limit on `why`**, because it is read in the same row and nothing is gained by a reason
/// longer than the thing it reasons about.
pub const WHY_MAX_CHARS: usize = 280;

/// **The heading a lesson is kept under.** Spelled once: `guide_template` writes it into a new
/// project's guide and this module appends below it, so the two cannot drift.
pub const LESSONS_HEADING: &str = "## Lessons";

/// **One lesson**, as the agent proposes it and as the guide carries it.
///
/// `id`, `after` and `state` are the editor's; `lesson` and `why` are the agent's. `client` is the
/// editor's too: the MCP server knows who is talking and the agent does not type its own name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Lesson {
    /// `lesson-1`, `lesson-2`, … — monotonic for the life of the editor session.
    pub id: String,
    /// **The hand-back the lesson follows**, as the agent named it.
    pub after: String,
    /// The one line the guide gains.
    pub lesson: String,
    /// Why it is worth the line.
    pub why: String,
    /// **The editor's own record of who is talking**, so the line carries a name without the agent
    /// typing one: `omp 0.4.1`, or the empty string when no client introduced itself.
    pub client: String,
}

impl Lesson {
    /// **Is this lesson kept as written, or refused for the field that does not fit?**
    ///
    /// The empty check first, then the newline, then the limit, so an agent that sent a paragraph is
    /// told that rather than that its first line is 40 characters. The refusal names the field every
    /// time: a message that said only "too long" would leave an agent guessing at two fields.
    pub fn validate(&self) -> Result<(), String> {
        for (field, value, limit) in self.text_fields() {
            if value.trim().is_empty() {
                return Err(format!(
                    "`{field}` is empty. A lesson is refused at the door and never trimmed: write \
                     what the task taught, or make no lesson."
                ));
            }
            if value.contains('\n') {
                return Err(format!(
                    "`{field}` has a newline in it. A lesson is one line: a second line would be a \
                     paragraph in a list of lines, so write it as one sentence and propose it again."
                ));
            }
            let len = value.chars().count();
            if len > limit {
                return Err(format!(
                    "`{field}` is {len} characters; its limit is {limit}. A lesson is not trimmed \
                     to fit, so shorten it and propose it again."
                ));
            }
        }
        Ok(())
    }

    /// The fields the agent writes, with each one's limit, in the order a reader fixes them.
    fn text_fields(&self) -> [(&'static str, &String, usize); 2] {
        [("lesson", &self.lesson, LESSON_MAX_CHARS), ("why", &self.why, WHY_MAX_CHARS)]
    }

    /// **The line the guide gains**, as [`append`] writes it: `- <lesson> (<date>, <client>)`.
    ///
    /// **Built in one place** so the row a person reads and the line the file holds cannot disagree.
    /// A lesson with no client named says `an agent` rather than an empty bracket — the name is
    /// decoration on the line's provenance, and a line that reads `(2026-11-11, )` is a bug a person
    /// would report as one.
    pub fn line(&self, date: &str) -> String {
        let who = if self.client.trim().is_empty() { "an agent" } else { self.client.trim() };
        format!("- {} ({date}, {who})", self.lesson.trim())
    }
}

/// **Where in `text` the Lessons section's body ends**, as a byte offset, or `None` when the text has
/// no [`LESSONS_HEADING`].
///
/// The section runs to the next heading of its own level or higher — `## ` or `# ` for
/// [`LESSONS_HEADING`]'s level — so a `### ` inside it is still inside it and a `## Rules` or a
/// `# Reference` after it is not. A `#Reference` with no space is not a heading, as in CommonMark.
pub fn lessons_end(text: &str) -> Option<usize> {
    let start = find_heading(text, LESSONS_HEADING)?;
    let after = start + LESSONS_HEADING.len();
    let mut at = after;
    for line in text[after..].split_inclusive('\n') {
        // The section ends at a heading of the same level or higher: `# ` and `## `, and not `### `.
        if heading_level(line).is_some_and(|level| level <= LESSONS_LEVEL) {
            return Some(at);
        }
        at += line.len();
    }
    Some(text.len())
}

/// The level of [`LESSONS_HEADING`]: the number of `#` it starts with.
const LESSONS_LEVEL: usize = 2;

/// **The level of the ATX heading `line` is, or `None` when it is not one.**
///
/// One to six `#`, then a space, a tab or the end of the line — so `#Reference` is **not** a heading,
/// as CommonMark has it, and a deeper heading is a larger number.
fn heading_level(line: &str) -> Option<usize> {
    let line = line.trim_start().trim_end_matches(['\n', '\r']);
    let level = line.chars().take_while(|c| *c == '#').count();
    let rest = &line[level..];
    ((1..=6).contains(&level) && (rest.is_empty() || rest.starts_with([' ', '\t']))).then_some(level)
}

/// **The byte offset of `heading` as a whole line**, or `None`.
///
/// Whole line, because `## Lessons` inside a sentence is prose about the heading, and a section
/// appended under one would be a section a person did not write.
fn find_heading(text: &str, heading: &str) -> Option<usize> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == heading {
            return Some(at);
        }
        at += line.len();
    }
    None
}

/// **`text` with `line` added under [`LESSONS_HEADING`]**, which is the whole of what Keep writes.
///
/// **A guide with no [`LESSONS_HEADING`] gets one at its end**, because a guide a person wrote before
/// this slice existed has no such section and refusing their guide would be refusing the feature.
/// The section is separated from the text above it by exactly one blank line, and the line goes after
/// the section's last line — so appending twice puts the second lesson under the first, in the order
/// they were kept.
///
/// **A line is added as it is.** No wrapping, no bullet the caller must remember, no re-encoding: the
/// file's bytes outside the insertion point are the file's bytes.
pub fn append(text: &str, line: &str) -> String {
    match lessons_end(text) {
        Some(end) => {
            // **The line goes after the section's last non-empty line**, and the run of newlines
            // that separated the section from whatever followed it — the person's own blank line,
            // or the file's trailing newline — is kept exactly where it was. So a lesson appended
            // above `## Rules` still leaves the blank line before that heading, and the bytes
            // outside the insertion point are the bytes that were there.
            let body = &text[..end];
            let kept = body.trim_end_matches('\n');
            let gap = &body[kept.len()..];
            let gap = if gap.is_empty() { "\n" } else { gap };
            let mut out = String::with_capacity(text.len() + line.len() + 2);
            out.push_str(kept);
            out.push('\n');
            out.push_str(line);
            out.push_str(gap);
            out.push_str(&text[end..]);
            out
        }
        None => {
            let mut out = String::with_capacity(text.len() + LESSONS_HEADING.len() + line.len() + 4);
            out.push_str(text);
            if !text.is_empty() && !text.ends_with('\n') {
                out.push('\n');
            }
            if !text.is_empty() {
                out.push('\n');
            }
            out.push_str(LESSONS_HEADING);
            out.push('\n');
            out.push_str(line);
            out.push('\n');
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one() -> Lesson {
        Lesson { id: "lesson-1".to_owned(), after: "present-1".to_owned(), lesson: "Measure the walls before placing a door.".to_owned(), why: "a door placed blind overlapped a wall".to_owned(), client: "omp 0.4.1".to_owned() }
    }

    #[test]
    fn a_lesson_is_one_line_with_its_date_and_its_client() {
        assert_eq!(one().line("2026-11-11"), "- Measure the walls before placing a door. (2026-11-11, omp 0.4.1)");
    }

    #[test]
    fn a_lesson_with_no_client_says_an_agent_rather_than_an_empty_bracket() {
        let mut lesson = one();
        lesson.client = String::new();
        assert_eq!(lesson.line("2026-11-11"), "- Measure the walls before placing a door. (2026-11-11, an agent)");
    }

    #[test]
    fn a_lesson_is_refused_past_its_limit_and_for_a_newline_but_never_trimmed() {
        let mut lesson = one();
        lesson.lesson = "x".repeat(LESSON_MAX_CHARS + 1);
        let refused = lesson.validate().expect_err("281 characters is refused");
        assert!(refused.contains("`lesson`") && refused.contains("limit is 280"), "{refused}");
        assert_eq!(lesson.lesson.chars().count(), LESSON_MAX_CHARS + 1, "the field was not trimmed");

        lesson.lesson = "one line\ntwo lines".to_owned();
        let refused = lesson.validate().expect_err("a newline is refused");
        assert!(refused.contains("`lesson`") && refused.contains("newline"), "{refused}");

        lesson.lesson = "a lesson".to_owned();
        lesson.why = " ".to_owned();
        let refused = lesson.validate().expect_err("an empty why is refused");
        assert!(refused.contains("`why`") && refused.contains("empty"), "{refused}");
        // **The longest legal lesson is kept in full**, which is what "never trimmed" means.
        lesson.why = "y".repeat(WHY_MAX_CHARS);
        lesson.lesson = "z".repeat(LESSON_MAX_CHARS);
        assert!(lesson.validate().is_ok());
        assert_eq!(lesson.lesson.chars().count(), LESSON_MAX_CHARS);
    }

    #[test]
    fn a_lesson_lands_under_the_heading_the_template_writes() {
        let guide = format!("# Guide\n\n## This world\n\nA quarry.\n\n{LESSONS_HEADING}\n");
        let kept = append(&guide, "- Measure first. (2026-11-11, omp 0.4.1)");
        assert_eq!(kept, format!("# Guide\n\n## This world\n\nA quarry.\n\n{LESSONS_HEADING}\n- Measure first. (2026-11-11, omp 0.4.1)\n"));
        // **Twice is in the order they were kept**, the second under the first.
        let twice = append(&kept, "- Measure twice. (2026-11-12, omp 0.4.2)");
        let lines: Vec<&str> = twice.lines().collect();
        assert_eq!(lines[lines.len() - 2], "- Measure first. (2026-11-11, omp 0.4.1)");
        assert_eq!(lines[lines.len() - 1], "- Measure twice. (2026-11-12, omp 0.4.2)");
    }

    #[test]
    fn a_lesson_kept_over_a_word_that_names_the_heading_is_prose_not_a_section() {
        let guide = "## Rules\n\nWrite `## Lessons` in every report.\n";
        let kept = append(guide, "- A kept line. (2026-11-11, omp 0.4.1)");
        assert!(kept.starts_with("## Rules\n\nWrite `## Lessons` in every report.\n\n## Lessons\n"), "{kept}");
    }

    #[test]
    fn a_guide_with_no_lessons_section_gains_one_at_its_end() {
        let kept = append("## Rules\n\nAlways close the door.\n", "- A kept line. (2026-11-11, omp 0.4.1)");
        assert_eq!(kept, "## Rules\n\nAlways close the door.\n\n## Lessons\n- A kept line. (2026-11-11, omp 0.4.1)\n");
    }

    #[test]
    fn a_section_that_has_later_headings_keeps_its_lesson_inside_itself() {
        let guide = "## Lessons\n\n*An old line.*\n\n## Rules\n\nAlways close the door.\n";
        let kept = append(guide, "- A kept line. (2026-11-11, omp 0.4.1)");
        assert_eq!(kept, "## Lessons\n\n*An old line.*\n- A kept line. (2026-11-11, omp 0.4.1)\n\n## Rules\n\nAlways close the door.\n");
    }

    #[test]
    fn a_why_with_a_newline_or_past_its_limit_is_refused_like_a_lesson() {
        let mut lesson = one();
        lesson.why = "one line\ntwo lines".to_owned();
        let refused = lesson.validate().expect_err("a newline in `why` is refused");
        assert!(refused.contains("`why`") && refused.contains("newline"), "{refused}");

        lesson.why = "y".repeat(WHY_MAX_CHARS + 1);
        let refused = lesson.validate().expect_err("281 characters of `why` is refused");
        assert!(refused.contains("`why`") && refused.contains("281") && refused.contains("limit is 280"), "{refused}");
    }

    #[test]
    fn a_heading_of_a_higher_level_ends_the_section_too() {
        let guide = "## Lessons\n\n*old*\n\n# Reference\n\nstuff\n";
        let kept = append(guide, "- Measure first. (2026-11-11, omp 0.4.1)");
        assert_eq!(kept, "## Lessons\n\n*old*\n- Measure first. (2026-11-11, omp 0.4.1)\n\n# Reference\n\nstuff\n");
        // A bare `#` or `##` line is an empty heading of its level and ends the section as well.
        for bare in ["#", "##"] {
            let guide = format!("## Lessons\n\n*old*\n{bare}\nstuff\n");
            let kept = append(&guide, "- A kept line. (2026-11-11, omp 0.4.1)");
            assert_eq!(kept, format!("## Lessons\n\n*old*\n- A kept line. (2026-11-11, omp 0.4.1)\n{bare}\nstuff\n"), "`{bare}` did not end the section");
        }
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading_and_a_deeper_heading_stays_inside() {
        // CommonMark: `#Reference` is a paragraph, so it does not end the section.
        let guide = "## Lessons\n\n#Reference\n### A deeper one\n\n## Rules\n";
        let kept = append(guide, "- A kept line. (2026-11-11, omp 0.4.1)");
        assert_eq!(kept, "## Lessons\n\n#Reference\n### A deeper one\n- A kept line. (2026-11-11, omp 0.4.1)\n\n## Rules\n");
    }
}
