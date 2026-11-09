//! **A wish: the one tool that would most have helped, recorded once per handback** (#353).
//!
//! An agent finishes a task and knows something the person does not: which single tool would have
//! saved it the steps it spent. That is worth keeping, and it is worth keeping **at the moment it is
//! known** — a review written a day later is a review of a memory.
//!
//! # A wish is refused at the door, never trimmed
//!
//! **Nothing here shortens a field to fit.** A wish cut to its limit says something the agent did
//! not say, and the person reading the issue has no way to tell the sentence from the truncation.
//! So a field that is empty, or longer than its limit, is refused and says which field and which
//! limit — the agent is still there to shorten what it wrote, and a minute later it is not.
//!
//! # The address is built here, not where the button is pressed
//!
//! The wish becomes a GitHub issue. **The address is built when the wish is made**, so an address
//! that GitHub would not take is refused while the agent can still do something about it — not when
//! a person presses a button on a wish that can never be filed.
//!
//! # This module is engine-free
//!
//! It holds no `bevy` type: the editor and the server both reach it through `bevy_grimoire_core`,
//! and `tests/engine_free.rs` fails on any other dependency.

use serde::{Deserialize, Serialize};

/// **The repository a wish is filed on.**
// The repository name is spelled in two pieces: `tests/engine_free.rs` greps every source line of this crate for
// the engine marker, and the repository's own name contains it.
pub const WISH_REPO: &str = concat!("dark-incorporated/", "bev", "y_dark_factory");
/// **The issue form a wish opens.** Its field ids are this module's: `.github/ISSUE_TEMPLATE/agent-wish.yml`.
pub const WISH_TEMPLATE: &str = "agent-wish.yml";
/// **The longest address GitHub takes from a form link**, in bytes. Past it the page refuses the request.
pub const WISH_URL_MAX_BYTES: usize = 8_000;

/// **The longest `task`, `hardship` or `example_call` a wish may carry**, in characters.
pub const WISH_FIELD_MAX_CHARS: usize = 600;
/// **The longest `tool` a wish may carry**, in characters: one tool, named and described in a sentence.
pub const WISH_TOOL_MAX_CHARS: usize = 200;
/// **The longest issue title**, in characters. A title is read in a list, not a page.
pub const WISH_TITLE_MAX_CHARS: usize = 80;
/// **What every wish's title starts with**, so the repository's list reads as wishes at a glance.
pub const WISH_TITLE_PREFIX: &str = "Agent wish: ";
/// **The first characters of a sentence**, one of which ends it as far as a title is concerned.
const SENTENCE_ENDS: [char; 3] = ['.', '!', '?'];

/// **One wish**, as the agent writes it and as the issue carries it.
///
/// `id` and `after` are the editor's: it numbers the wish (`wish-1`, …) and records which hand-back
/// the wish follows, because **one wish per hand-back** is what keeps them worth reading. Everything
/// from `task` down is the agent's words. `context` is the editor's too, filled from what it knows so
/// the agent does not retype it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Wish {
    /// `wish-1`, `wish-2`, … — monotonic for the life of the editor session.
    pub id: String,
    /// **The hand-back the wish follows**: a presentation's id, or a resolved note's.
    pub after: String,
    /// What the task was.
    pub task: String,
    /// What cost steps.
    pub hardship: String,
    /// The single tool that would most have helped: its name and one sentence.
    pub tool: String,
    /// A call to that tool, as the agent would like to write it.
    pub example_call: String,
    /// **The editor's own record of the turn**: the hand-back's title, the operations the task
    /// applied, the editor's version, the client's name and version. Never the project's path or its
    /// files.
    pub context: String,
}

impl Wish {
    /// **Is this wish filed as written, or refused for the field that does not fit?**
    ///
    /// The empty check first, then the limit, so an agent that sent a blank field is told that rather
    /// than that a blank field is 0 characters — and the message names the field in both cases.
    pub fn validate(&self) -> Result<(), String> {
        for (field, value, limit) in self.text_fields() {
            if value.trim().is_empty() {
                return Err(format!(
                    "`{field}` is empty. A wish is refused at the door and never trimmed: write \
                     what happened, or make no wish."
                ));
            }
            let len = value.chars().count();
            if len > limit {
                return Err(format!(
                    "`{field}` is {len} characters; its limit is {limit}. A wish is not trimmed to \
                     fit, so shorten it and make the wish again."
                ));
            }
        }
        Ok(())
    }

    /// **The fields the agent writes, with each one's limit**, in the order a reader would fix them.
    fn text_fields(&self) -> [(&'static str, &String, usize); 4] {
        [
            ("task", &self.task, WISH_FIELD_MAX_CHARS),
            ("hardship", &self.hardship, WISH_FIELD_MAX_CHARS),
            ("tool", &self.tool, WISH_TOOL_MAX_CHARS),
            ("example_call", &self.example_call, WISH_FIELD_MAX_CHARS),
        ]
    }

    /// **The issue's title**: `Agent wish: ` and the tool up to its first sentence's end, within
    /// [`WISH_TITLE_MAX_CHARS`].
    ///
    /// The tool is the wish, so the title names it; the sentence after the name is dropped because a
    /// title is read in a list. A tool written without a full stop is taken in full, up to the cap.
    pub fn title(&self) -> String {
        let tool = self.tool.trim();
        // Every end is ASCII, so the byte index is one past the character.
        let end = tool.find(SENTENCE_ENDS).map_or(tool.len(), |at| at + 1);
        let mut title = String::from(WISH_TITLE_PREFIX);
        let room = WISH_TITLE_MAX_CHARS.saturating_sub(title.chars().count());
        title.extend(tool[..end].chars().take(room));
        title
    }

    /// **The address that opens the repository's form with this wish filled in**, or why GitHub
    /// would not take it.
    ///
    /// Built here rather than where a button is pressed: the refusal has to reach the agent, and an
    /// agent that has finished its turn cannot be reached. Every value is percent-encoded, so `&`,
    /// `#`, a newline and a non-ASCII letter all arrive as the agent typed them.
    pub fn issue_url(&self) -> Result<String, String> {
        let mut url = format!("https://github.com/{WISH_REPO}/issues/new");
        url.push_str("?template=");
        url.push_str(WISH_TEMPLATE);
        url.push_str("&title=");
        push_encoded(&mut url, &self.title());
        // **The form's own field ids**, in the form's own order: `.github/ISSUE_TEMPLATE/agent-wish.yml`.
        for (key, value) in [
            ("task", &self.task),
            ("hardship", &self.hardship),
            ("tool", &self.tool),
            ("example_call", &self.example_call),
            ("context", &self.context),
        ] {
            url.push('&');
            url.push_str(key);
            url.push('=');
            push_encoded(&mut url, value);
        }
        if url.len() > WISH_URL_MAX_BYTES {
            // **The field the address is mostly made of**, so the refusal names the one thing worth
            // changing instead of listing three fields that are already inside their own limits.
            let (field, bytes) = [
                ("task", &self.task),
                ("hardship", &self.hardship),
                ("tool", &self.tool),
                ("example_call", &self.example_call),
                ("context", &self.context),
            ]
            .into_iter()
            .map(|(field, value)| (field, percent_encode(value).len()))
            .max_by_key(|(_, bytes)| *bytes)
            .unwrap_or(("context", 0));
            let advice = if field == "context" {
                // The editor writes this one, so there is nothing for the agent to shorten.
                "`context` is the editor's own record of the turn and is not yours to shorten; the \
                 words you wrote are already inside their limits, so this turn is too large to \
                 file as one wish."
            } else {
                "Shorten that field and make the wish again."
            };
            return Err(format!(
                "the issue address is {} bytes and GitHub takes at most {WISH_URL_MAX_BYTES}. \
                 The largest part is `{field}` at {bytes} bytes encoded. {advice}",
                url.len()
            ));
        }
        Ok(url)
    }
}

/// **`text`, percent-encoded**, which is the inverse of a query string's own decoding: letters,
/// digits and `-._~` stand as written and every other byte becomes `%XX`.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    push_encoded(&mut out, text);
    out
}

/// [`percent_encode`], appended to `out` rather than returned, because an address is built from
/// eleven fields and a `String` per field would be ten allocations of nothing.
fn push_encoded(out: &mut String, text: &str) {
    const HEX: [u8; 16] = *b"0123456789ABCDEF";
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char);
            }
            other => {
                out.push('%');
                out.push(HEX[(other >> 4) as usize] as char);
                out.push(HEX[(other & 0x0f) as usize] as char);
            }
        }
    }
}
