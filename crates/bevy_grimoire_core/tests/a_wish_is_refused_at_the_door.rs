//! **A wish is refused at the door, never trimmed** (#353).
//!
//! A wish is the one tool that would most have helped with a task, recorded once per handback. An
//! agent writes it in a hurry at the end of a turn, so the four fields are checked where they are
//! built — **nothing is silently cut to fit**, because a wish trimmed to its limit is a wish that
//! says something the agent did not say, and the person reading the issue has no way to tell.
//!
//! # What is being proved
//!
//! 1. **An empty field is refused, naming the field.**
//! 2. **A field over its limit is refused, naming the field and the limit.** `task`, `hardship` and
//!    `example_call` hold [`WISH_FIELD_MAX_CHARS`] characters; `tool` holds [`WISH_TOOL_MAX_CHARS`],
//!    because it names one tool and one sentence rather than a paragraph.

use bevy_grimoire_core::wish::{WISH_FIELD_MAX_CHARS, WISH_TOOL_MAX_CHARS, Wish};

/// **A wish with every field filled in**, so each test varies the one field it is about.
fn wish() -> Wish {
    Wish {
        id: "wish-1".to_owned(),
        after: "present-1".to_owned(),
        task: "place a wall along the courtyard's east side".to_owned(),
        hardship: "three calls to grimoire_state to find the wall's piece id".to_owned(),
        tool: "grimoire_find_piece: searches the library by label and answers the ids.".to_owned(),
        example_call: "grimoire_find_piece({\"label\": \"wall\"})".to_owned(),
        context: "Handback: the courtyard".to_owned(),
    }
}

/// **1. An empty field names itself.**
#[test]
fn an_empty_field_is_refused_by_name() {
    for field in ["task", "hardship", "tool", "example_call"] {
        let mut w = wish();
        match field {
            "task" => w.task = "   ".to_owned(),
            "hardship" => w.hardship = String::new(),
            "tool" => w.tool = "\n".to_owned(),
            _ => w.example_call = String::new(),
        }
        let refused = w.validate().expect_err(&format!("an empty `{field}` must be refused"));
        assert!(
            refused.contains(field),
            "the refusal does not name `{field}`, so the agent cannot tell which one is empty: {refused:?}"
        );
    }
}

/// **2. A field over its limit names the field and the limit.**
#[test]
fn a_field_over_its_limit_is_refused_by_name_and_limit() {
    // One character past each limit, so the refusal is about the limit and not about size.
    let too_long = |n: usize| "x".repeat(n + 1);
    let cases = [
        ("task", too_long(WISH_FIELD_MAX_CHARS), WISH_FIELD_MAX_CHARS),
        ("hardship", too_long(WISH_FIELD_MAX_CHARS), WISH_FIELD_MAX_CHARS),
        ("example_call", too_long(WISH_FIELD_MAX_CHARS), WISH_FIELD_MAX_CHARS),
        ("tool", too_long(WISH_TOOL_MAX_CHARS), WISH_TOOL_MAX_CHARS),
    ];
    for (field, text, limit) in cases {
        let mut w = wish();
        match field {
            "task" => w.task = text,
            "hardship" => w.hardship = text,
            "tool" => w.tool = text,
            _ => w.example_call = text,
        }
        let refused = w
            .validate()
            .expect_err(&format!("a `{field}` of {} characters must be refused", limit + 1));
        assert!(
            refused.contains(field),
            "the refusal does not name `{field}`: {refused:?}"
        );
        assert!(
            refused.contains(&limit.to_string()),
            "the refusal does not say `{field}`'s limit is {limit}: {refused:?}"
        );
    }
}

/// **A field exactly at its limit is kept whole.** The limit is a limit, not a target.
#[test]
fn a_field_at_its_limit_is_accepted_whole() {
    let mut w = wish();
    w.task = "t".repeat(WISH_FIELD_MAX_CHARS);
    w.tool = "u".repeat(WISH_TOOL_MAX_CHARS);
    assert_eq!(w.validate(), Ok(()), "a wish inside the limits must be accepted");
    assert_eq!(
        w.task.chars().count(),
        WISH_FIELD_MAX_CHARS,
        "the field was trimmed to fit, which a wish must never be"
    );
}
