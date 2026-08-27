//! Line splitting that keeps the original text and a whitespace-free key.

/// One source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Text without its `\n` / `\r\n` terminator, lossily decoded.
    pub text: String,
    /// `text` with all whitespace removed — the diff key under
    /// `ignore_whitespace`.
    pub normalised: String,
}

/// Split into lines. A trailing terminator does not produce an extra empty
/// line; a missing one still yields the last line.
pub fn split_lines(bytes: &[u8]) -> Vec<Line> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut pieces: Vec<&[u8]> = bytes.split(|b| *b == b'\n').collect();
    // `split` yields a final empty piece when the input ends with '\n'.
    if bytes.ends_with(b"\n") {
        pieces.pop();
    }
    pieces
        .into_iter()
        .map(|raw| {
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            let text = String::from_utf8_lossy(raw).into_owned();
            let normalised = text.chars().filter(|c| !c.is_whitespace()).collect();
            Line { text, normalised }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(b: &[u8]) -> Vec<String> {
        split_lines(b).into_iter().map(|l| l.text).collect()
    }

    #[test]
    fn terminators() {
        assert_eq!(texts(b""), Vec::<String>::new());
        assert_eq!(texts(b"a\nb\n"), vec!["a", "b"]);
        assert_eq!(texts(b"a\nb"), vec!["a", "b"]);
        assert_eq!(texts(b"a\r\nb\r\n"), vec!["a", "b"]);
        assert_eq!(texts(b"\n"), vec![""]);
        assert_eq!(texts(b"a\n\n"), vec!["a", ""]);
    }

    #[test]
    fn normalised_strips_all_whitespace() {
        let l = &split_lines(b"  let x =\t1; \n")[0];
        assert_eq!(l.text, "  let x =\t1; ");
        assert_eq!(l.normalised, "letx=1;");
    }
}
