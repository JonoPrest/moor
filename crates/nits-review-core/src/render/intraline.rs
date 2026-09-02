//! Word-level diff within a paired `-`/`+` line, producing `changed` byte
//! ranges for each side.

use nits_protocol::ColRange;

/// A token with its byte range in the source line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Tok<'a> {
    text: &'a str,
    start: u32,
    end: u32,
}

/// Split into runs of word chars, runs of whitespace, and single other chars.
fn tokens(s: &str) -> Vec<Tok<'_>> {
    let mut out = Vec::new();
    let mut iter = s.char_indices().peekable();
    while let Some((start, c)) = iter.next() {
        let class = class_of(c);
        let mut end = start + c.len_utf8();
        if class != Class::Other {
            while let Some(&(i, d)) = iter.peek() {
                if class_of(d) == class {
                    end = i + d.len_utf8();
                    iter.next();
                } else {
                    break;
                }
            }
        }
        out.push(Tok {
            text: &s[start..end],
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(end).unwrap_or(u32::MAX),
        });
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Word,
    Space,
    Other,
}

fn class_of(c: char) -> Class {
    if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else if c.is_whitespace() {
        Class::Space
    } else {
        Class::Other
    }
}

struct TokSource<'a>(&'a [Tok<'a>]);

impl<'a> imara_diff::TokenSource for TokSource<'a> {
    type Token = &'a str;
    type Tokenizer = std::iter::Map<std::slice::Iter<'a, Tok<'a>>, fn(&Tok<'a>) -> &'a str>;
    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter().map(|t| t.text)
    }
    fn estimate_tokens(&self) -> u32 {
        u32::try_from(self.0.len()).unwrap_or(u32::MAX)
    }
}

/// Byte ranges that differ, `(left, right)`. Adjacent ranges are merged.
pub fn changed_ranges(left: &str, right: &str) -> (Vec<ColRange>, Vec<ColRange>) {
    let lt = tokens(left);
    let rt = tokens(right);
    let input = imara_diff::InternedInput::new(TokSource(&lt), TokSource(&rt));
    let diff = imara_diff::Diff::compute(imara_diff::Algorithm::Myers, &input);
    let mut l = Vec::new();
    let mut r = Vec::new();
    for h in diff.hunks() {
        push_range(&mut l, &lt, h.before);
        push_range(&mut r, &rt, h.after);
    }
    (l, r)
}

fn push_range(out: &mut Vec<ColRange>, toks: &[Tok<'_>], range: std::ops::Range<u32>) {
    if range.is_empty() {
        return;
    }
    let start = toks[range.start as usize].start;
    let end = toks[range.end as usize - 1].end;
    if let Some(last) = out.last_mut()
        && last.end() == start
        && let Ok(merged) = ColRange::new(last.start(), end)
    {
        *last = merged;
        return;
    }
    if let Ok(cr) = ColRange::new(start, end) {
        out.push(cr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(v: &[ColRange]) -> Vec<(u32, u32)> {
        v.iter().map(|r| (r.start(), r.end())).collect()
    }

    #[test]
    fn single_word_change() {
        let (l, r) = changed_ranges("let x = 1;", "let y = 1;");
        assert_eq!(ranges(&l), vec![(4, 5)]);
        assert_eq!(ranges(&r), vec![(4, 5)]);
    }

    #[test]
    fn insertion_only_on_right() {
        let (l, r) = changed_ranges("let x = 1;", "let x: u32 = 1;");
        assert_eq!(ranges(&l), Vec::<(u32, u32)>::new());
        assert_eq!(ranges(&r), vec![(5, 10)]);
    }

    #[test]
    fn identical_lines_have_no_ranges() {
        let (l, r) = changed_ranges("same", "same");
        assert!(l.is_empty() && r.is_empty());
    }

    #[test]
    fn unicode_offsets_are_bytes() {
        let (_, r) = changed_ranges("é = 1", "é = 2");
        assert_eq!(ranges(&r), vec![(5, 6)]);
    }
}
