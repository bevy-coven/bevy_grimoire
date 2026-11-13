//! **The tokenizer RON and Rhai share**: colours, the brackets in order, a string that never ended. It is a scan and
//! not a parser (see the module note of [`super`]), so the errors it names are only the ones a scan can be sure of.

use std::ops::Range;

use bevy_gooey::{CodeSeverity, SyntaxRole};

use super::Finding;

// ── RON and Rhai ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Code {
    Ron,
    Rhai,
}

const RHAI_KEYWORDS: [&str; 26] = [
    "let", "const", "fn", "private", "if", "else", "while", "do", "until", "loop", "for", "in", "break", "continue", "return",
    "switch", "throw", "try", "catch", "true", "false", "import", "export", "as", "this", "global",
];

const RON_KEYWORDS: [&str; 4] = ["true", "false", "None", "Some"];

/// One bracket the scan met: the character, and the line it is on.
#[derive(Clone, Copy)]
struct Bracket {
    ch: u8,
    at: usize,
    line: usize,
}

/// What a scan of RON or Rhai found: the spans, the brackets in order, and a string that never ended.
pub(super) struct Scan {
    brackets: Vec<Bracket>,
    unterminated: Option<usize>,
}

pub(super) fn highlights(text: &str, code: Code) -> (Vec<(Range<usize>, SyntaxRole)>, Scan) {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut scan = Scan { brackets: Vec::new(), unterminated: None };
    let mut line = 0;
    let mut at = 0;
    while at < bytes.len() {
        let b = bytes[at];
        match b {
            b'\n' => {
                line += 1;
                at += 1;
            }
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                let end = text[at..].find('\n').map_or(text.len(), |n| at + n);
                spans.push((at..end, SyntaxRole::Comment));
                at = end;
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                let mut depth = 0;
                let mut end = at;
                while end < bytes.len() {
                    if bytes[end..].starts_with(b"/*") {
                        depth += 1;
                        end += 2;
                    } else if bytes[end..].starts_with(b"*/") {
                        depth -= 1;
                        end += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if bytes[end] == b'\n' {
                            line += 1;
                        }
                        end += 1;
                    }
                }
                spans.push((at..end.min(text.len()), SyntaxRole::Comment));
                at = end.min(text.len());
            }
            // **A character literal is a string of one character** (`'"'`, `'('`, `'\n'`): its quote and what is inside it
            // open no string and no bracket. A lone `'` (a lifetime in a RON enum name, an apostrophe) is only itself.
            b'\'' => {
                match char_literal_end(text, at) {
                    Some(end) => {
                        spans.push((at..end, SyntaxRole::String));
                        at = end;
                    }
                    None => at += 1,
                }
            }
            b'"' | b'`' => {
                let quote = b;
                let begin = at;
                at += 1;
                let mut closed = false;
                while at < bytes.len() {
                    match bytes[at] {
                        // A backslash takes the next byte with it, and a line break it takes is still a line.
                        b'\\' if quote == b'"' => {
                            if bytes.get(at + 1) == Some(&b'\n') {
                                line += 1;
                            }
                            at += 2;
                        }
                        c if c == quote => {
                            at += 1;
                            closed = true;
                            break;
                        }
                        b'\n' if quote == b'"' => break,
                        b'\n' => {
                            line += 1;
                            at += 1;
                        }
                        _ => at += 1,
                    }
                }
                let at_end = at.min(text.len());
                if !closed && scan.unterminated.is_none() {
                    scan.unterminated = Some(begin);
                }
                spans.push((begin..at_end, SyntaxRole::String));
                at = at_end;
            }
            b'(' | b')' | b'[' | b']' | b'{' | b'}' => {
                scan.brackets.push(Bracket { ch: b, at, line });
                at += 1;
            }
            b'0'..=b'9' => {
                let begin = at;
                at += 1;
                while at < bytes.len() {
                    match bytes[at] {
                        b'0'..=b'9' | b'_' | b'.' | b'a'..=b'z' | b'A'..=b'Z' => at += 1,
                        b'+' | b'-' if matches!(bytes[at - 1], b'e' | b'E') => at += 1,
                        _ => break,
                    }
                }
                spans.push((begin..at, SyntaxRole::Number));
            }
            b'-' if bytes.get(at + 1).is_some_and(u8::is_ascii_digit) => {
                let begin = at;
                at += 1;
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || matches!(bytes[at], b'_' | b'.')) {
                    at += 1;
                }
                spans.push((begin..at, SyntaxRole::Number));
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let begin = at;
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                    at += 1;
                }
                let word = &text[begin..at];
                let next = text[at..].trim_start_matches([' ', '\t']);
                let role = if code == Code::Ron && RON_KEYWORDS.contains(&word) || code == Code::Rhai && RHAI_KEYWORDS.contains(&word) {
                    Some(SyntaxRole::Keyword)
                } else if next.starts_with(':') && !next.starts_with("::") {
                    Some(SyntaxRole::Key)
                } else if code == Code::Rhai && next.starts_with('(') {
                    Some(SyntaxRole::Function)
                } else if code == Code::Ron && word.starts_with(|c: char| c.is_ascii_uppercase()) {
                    Some(SyntaxRole::Type)
                } else {
                    None
                };
                if let Some(role) = role {
                    spans.push((begin..at, role));
                }
            }
            _ => at += 1,
        }
    }
    (spans, scan)
}

pub(super) fn bracket_folds(scan: &Scan) -> Vec<Range<usize>> {
    let mut stack: Vec<Bracket> = Vec::new();
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut out: Vec<Range<usize>> = Vec::new();
    for bracket in &scan.brackets {
        match bracket.ch {
            b'(' | b'[' | b'{' => stack.push(*bracket),
            _ => {
                if let Some(open) = stack.pop()
                    && open.line < bracket.line
                    && seen.insert((open.line, bracket.line))
                {
                    out.push(open.line..bracket.line);
                }
            }
        }
    }
    out.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
    out
}

/// **Where a character literal that starts at `at` (a `'`) ends**, past its closing quote, or `None` when what follows
/// is not one: `'x'`, `'"'`, `'\n'`, `'\u{1F600}'` are; a lone quote is not.
fn char_literal_end(text: &str, at: usize) -> Option<usize> {
    let rest = &text[at + 1..];
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if first == '\'' || first == '\n' {
        return None;
    }
    if first == '\\' {
        // An escape: one character after the backslash (which may itself be a quote), or a `u{...}` code, then the closing quote.
        let after = &rest[1..];
        let escaped = after.chars().next()?;
        let body = if escaped == 'u' && after[1..].starts_with('{') {
            let close = after.find('}')?;
            (close <= CHAR_ESCAPE_BYTES).then_some(close + 1)?
        } else {
            escaped.len_utf8()
        };
        return after[body..].starts_with('\'').then_some(at + 1 + 1 + body + 1);
    }
    let (next, c) = chars.next()?;
    (c == '\'').then_some(at + 1 + next + 1)
}

/// The longest an escaped character literal is, in bytes between its backslash and its closing quote (`\u{10FFFF}`).
const CHAR_ESCAPE_BYTES: usize = 10;

/// **What a bracket scan can be sure of** in a RON or Rhai file: a bracket that closes nothing or never closes, a
/// string that never ends.
pub(super) fn bracket_findings(text: &str, code: Code) -> Vec<Finding> {
    let (_, scan) = highlights(text, code);
    let mut out = Vec::new();
    let mut stack: Vec<Bracket> = Vec::new();
    let error = |at: usize, message: String| Finding { range: at..at + 1, severity: CodeSeverity::Error, message };
    for bracket in &scan.brackets {
        match bracket.ch {
            b'(' | b'[' | b'{' => stack.push(*bracket),
            close => {
                let want = match close {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                match stack.pop() {
                    Some(open) if open.ch == want => {}
                    Some(open) => {
                        out.push(error(bracket.at, format!("`{}` does not close `{}` from line {}", close as char, open.ch as char, open.line + 1)));
                    }
                    None => out.push(error(bracket.at, format!("`{}` closes nothing", close as char))),
                }
            }
        }
    }
    for open in stack {
        out.push(error(open.at, format!("`{}` is never closed", open.ch as char)));
    }
    if let Some(at) = scan.unterminated {
        out.push(error(at, "this string never ends".to_owned()));
    }
    out.sort_by_key(|f| f.range.start);
    out
}
