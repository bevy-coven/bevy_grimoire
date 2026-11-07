//! **The link under a cell of the terminal** (#325): an `http://` or `https://` address in what the shell
//! drew, as VS Code's terminal underlines one and opens it on Cmd+click.
//!
//! Pure over the rows the pane draws, so what a person points at and what is found are the same cells.
//! A row that wraps ([`Line::wraps`]) continues onto the next, so a long address that the pane broke is one
//! link whichever of its rows is pointed at.

use super::grid::Line;

/// The schemes that make a run of text a link.
const SCHEMES: [&str; 2] = ["http://", "https://"];

/// Characters an address may end a sentence on: prose around a link, not part of it.
const TRAILING_PROSE: [char; 7] = ['.', ',', ')', ']', '>', '\'', '"'];

/// Whether a character may be inside an address: printable ASCII, less the brackets and quotes that
/// delimit one in prose and markup.
fn may_hold(c: char) -> bool {
    c.is_ascii_graphic() && !matches!(c, '<' | '>' | '"' | '`' | '{' | '}' | '|' | '\\' | '^')
}

/// Where each scheme begins in `text`, in order.
fn starts_in(text: &[char]) -> Vec<usize> {
    (0..text.len())
        .filter(|&at| {
            SCHEMES.iter().any(|scheme| {
                let scheme: Vec<char> = scheme.chars().collect();
                text.len() >= at + scheme.len()
                    && text[at..at + scheme.len()].iter().zip(&scheme).all(|(a, b)| a.eq_ignore_ascii_case(b))
            })
        })
        .collect()
}

/// **The address under cell (`row`, `col`) of `lines`**, or `None` for a cell that is in no address:
/// plain text, past the end of its row, or a row that is not there. `columns` is the pane's width.
pub fn link_at(lines: &[Line], columns: usize, row: usize, col: usize) -> Option<String> {
    let line = lines.get(row)?;
    if col >= line.cells.len() {
        return None;
    }
    // The logical line this row is part of: back to where it began, forward to where it ends.
    let mut first = row;
    while first > 0 && lines[first - 1].wraps {
        first -= 1;
    }
    let mut last = row;
    while lines[last].wraps && last + 1 < lines.len() {
        last += 1;
    }
    // A wrapped row is as wide as the pane, but its trailing blanks were trimmed from `cells`: put them
    // back, or the space a row wrapped at would vanish and two words would be one address.
    let mut text: Vec<char> = Vec::new();
    let mut pointed = 0;
    for (at, line) in lines[first..=last].iter().enumerate() {
        if first + at == row {
            pointed = text.len() + col;
        }
        text.extend(line.cells.iter().map(|cell| cell.ch));
        if first + at < last {
            text.resize(text.len() + columns.saturating_sub(line.cells.len()), ' ');
        }
    }
    let starts = starts_in(&text);
    for (n, &start) in starts.iter().enumerate() {
        let next = starts.get(n + 1).copied().unwrap_or(text.len());
        let mut end = start;
        while end < next && may_hold(text[end]) {
            end += 1;
        }
        while end > start && TRAILING_PROSE.contains(&text[end - 1]) {
            end -= 1;
        }
        // A scheme with nothing after it is not an address.
        let scheme_len = if text[start..end].iter().take(5).collect::<String>().eq_ignore_ascii_case("https") { 8 } else { 7 };
        if end > start + scheme_len && (start..end).contains(&pointed) {
            return Some(text[start..end].iter().collect());
        }
    }
    None
}
