//! `{tag}` substitution and the count formats tdesktop's `lt_count`,
//! `lt_count_decimal` and `lt_count_short` produce.

/// One piece of a template: literal text or a `{name}` placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece<'a> {
    Text(&'a str),
    Tag(&'a str),
}

/// `template` split at its `{name}` placeholders. An unclosed `{` is text.
pub fn pieces(template: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let (before, from_open) = rest.split_at(open);
        let Some(close) = from_open.find('}') else {
            break;
        };
        if !before.is_empty() {
            out.push(Piece::Text(before));
        }
        let (tag, after) = from_open.split_at(close + 1);
        out.push(Piece::Tag(
            tag.trim_start_matches('{').trim_end_matches('}'),
        ));
        rest = after;
    }
    if !rest.is_empty() {
        out.push(Piece::Text(rest));
    }
    out
}

/// Replaces each `{name}` whose name is in `tags`; others stay literal.
pub(crate) fn substitute(template: &str, tags: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    for piece in pieces(template) {
        match piece {
            Piece::Text(text) => out.push_str(text),
            Piece::Tag(name) => match tags.iter().find(|(n, _)| *n == name) {
                Some((_, value)) => out.push_str(value),
                None => {
                    out.push('{');
                    out.push_str(name);
                    out.push('}');
                }
            },
        }
    }
    out
}

/// `text` without its `**` marks (tdesktop marked text), and the byte
/// ranges they enclosed.
pub fn split_bold(text: &str) -> (String, Vec<std::ops::Range<usize>>) {
    let mut plain = String::with_capacity(text.len());
    let mut bold = Vec::new();
    for (index, part) in text.split("**").enumerate() {
        let start = plain.len();
        plain.push_str(part);
        if index % 2 == 1 && !part.is_empty() {
            bold.push(start..plain.len());
        }
    }
    (plain, bold)
}

/// A name put into a marked string: its own `**` cannot open a bold run.
pub fn unmarked(name: &str) -> String {
    name.replace("**", "*\u{2060}*")
}

/// `1234567` → `1,234,567`.
pub(crate) fn group_digits(count: i64) -> String {
    let digits = count.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if count < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `9999` → `9999`, `12345` → `12.3K`, `2500000` → `2.5M`, `3000000000` →
/// `3B` (one truncated decimal, dropped when zero), as tdesktop's
/// `lt_count_short`.
pub(crate) fn short_count(count: i64) -> String {
    let abs = count.unsigned_abs();
    let (divider, suffix) = if abs >= 1_000_000_000 {
        (1_000_000_000, "B")
    } else if abs >= 1_000_000 {
        (1_000_000, "M")
    } else if abs >= 10_000 {
        (1_000, "K")
    } else {
        return count.to_string();
    };
    let tenths = abs / (divider / 10);
    let sign = if count < 0 { "-" } else { "" };
    let (whole, tenth) = (tenths / 10, tenths % 10);
    if tenth == 0 {
        format!("{sign}{whole}{suffix}")
    } else {
        format!("{sign}{whole}.{tenth}{suffix}")
    }
}
