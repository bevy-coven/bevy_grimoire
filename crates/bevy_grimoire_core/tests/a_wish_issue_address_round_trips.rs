//! **A wish's issue address carries its words unchanged** (#353).
//!
//! The address is built where the wish is made, not where the person presses the button: an address
//! GitHub will not take has to be refused while the agent is still there to shorten what it wrote,
//! and an agent that has finished its turn is not.
//!
//! # What is being proved
//!
//! 1. **Every field survives the journey.** `task`, `hardship`, `tool` and `example_call`, and the
//!    context the editor filled in, decode back exactly — including `&`, `#`, newlines and non-ASCII,
//!    which are the characters a form address gets wrong first.
//! 2. **An address too long for GitHub is refused**, naming the limit.
//! 3. **The issue form takes the ids the address writes**, and is labelled `agent-wish` and
//!    `enhancement` — a form whose field is called something else silently drops that field.

use bevy_grimoire_core::wish::{WISH_REPO, WISH_TEMPLATE, WISH_URL_MAX_BYTES, Wish};

/// **A wish carrying the characters a query string gets wrong.**
fn wish() -> Wish {
    Wish {
        id: "wish-1".to_owned(),
        after: "present-1".to_owned(),
        task: "place a wall & a gate #3\non the east side — “as drawn”".to_owned(),
        hardship: "three calls to grimoire_state; two wasted\non 東京's piece ids".to_owned(),
        tool: "grimoire_find_piece: searches the library by label. Answers ids.".to_owned(),
        example_call: "grimoire_find_piece({\"label\": \"wall &= 1\"})\n// 100% done".to_owned(),
        context: "Handback: the courtyard & its gate\nEditor: 0.5.4\nClient: omp 18.16.0".to_owned(),
    }
}

/// **The value of a query parameter, percent-decoded**, from an address this test built.
fn parameter(url: &str, key: &str) -> String {
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or_else(|| panic!("{url} has no query"));
    for pair in query.split('&') {
        let Some((name, value)) = pair.split_once('=') else { continue };
        if name == key {
            return percent_decode(value);
        }
    }
    panic!("`{key}` is not in {url}");
}

/// **Percent-decode, byte by byte**, which is the inverse [`bevy_grimoire_core::wish::percent_encode`] claims.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).expect("two hex digits");
            out.push(u8::from_str_radix(hex, 16).expect("a hex byte"));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("the decoder produced UTF-8, as the encoder wrote it")
}

/// **1. Every field decodes back exactly.**
#[test]
fn the_address_decodes_to_the_words_the_agent_wrote() {
    let w = wish();
    let url = w.issue_url().unwrap_or_else(|e| panic!("a wish inside the limits is addressable: {e}"));
    assert!(
        url.starts_with(&format!("https://github.com/{WISH_REPO}/issues/new?template={WISH_TEMPLATE}&")),
        "the address is not the repository's own new-issue form: {url}"
    );
    for (key, want) in [
        ("task", &w.task),
        ("hardship", &w.hardship),
        ("tool", &w.tool),
        ("example_call", &w.example_call),
        ("context", &w.context),
    ] {
        assert_eq!(&parameter(&url, key), want, "`{key}` did not survive the address");
    }
    assert!(
        parameter(&url, "title").starts_with("Agent wish: "),
        "the title does not read as an agent's wish: {}",
        parameter(&url, "title")
    );
    assert!(
        parameter(&url, "title").contains("grimoire_find_piece"),
        "the title does not name the tool: {}",
        parameter(&url, "title")
    );
}

/// **2. An address GitHub would not take is refused, naming the limit.**
#[test]
fn an_address_over_the_limit_is_refused() {
    let mut w = wish();
    // **Long but legal**: every field the agent writes is inside its own limit, and the address is
    // what does not fit — the editor's context has no limit of its own, which is exactly why the
    // address is measured rather than assumed.
    w.context = "c".repeat(WISH_URL_MAX_BYTES);
    assert_eq!(w.validate(), Ok(()), "the test needs a wish whose fields are all inside their limits");
    let refused = w.issue_url().expect_err("an address over the byte limit must be refused");
    assert!(
        refused.contains(&WISH_URL_MAX_BYTES.to_string()),
        "the refusal does not name the {WISH_URL_MAX_BYTES}-byte limit: {refused:?}"
    );
}

/// **3. The issue form takes the ids the address writes, and is labelled for this.**
#[test]
fn the_issue_form_takes_the_ids_the_address_writes() {
    let form = include_str!("../../../.github/ISSUE_TEMPLATE/agent-wish.yml");
    // The ids, one per line, as the form spells them.
    let ids: Vec<&str> = form
        .lines()
        .filter_map(|line| line.trim().strip_prefix("id:"))
        .map(|id| id.trim())
        .collect();
    let mut want = vec!["task", "hardship", "tool", "example_call", "context"];
    let mut have = ids.clone();
    want.sort_unstable();
    have.sort_unstable();
    assert_eq!(
        have, want,
        "the form's field ids are {ids:?}. The address writes title, task, hardship, tool, \
         example_call and context: a form whose id is spelled differently drops that field without \
         a word."
    );
    // And the address writes nothing the form does not take.
    let url = wish().issue_url().expect("the wish is addressable");
    for key in ["task", "hardship", "tool", "example_call", "context"] {
        assert!(url.contains(&format!("&{key}=")), "the address does not write `{key}`");
    }
    assert!(
        form.contains("labels: [agent-wish, enhancement]"),
        "the form is not labelled `agent-wish` and `enhancement`, so a wish never reaches that \
         queue:\n{form}"
    );
    // `example_call` is a call as it would be written, so it is shown as code rather than as prose.
    let example_at = form.find("id: example_call").expect("the form has example_call");
    let after = &form[example_at..];
    let block = after.split("\n  - type:").next().unwrap_or(after);
    assert!(
        block.contains("render: code"),
        "example_call is not rendered as code in the form:\n{block}"
    );
}
