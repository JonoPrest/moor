//! Syntax highlighting via syntect's parser (no themes): scopes are mapped
//! to the closed [`SpanClass`] set so the UI stylesheet is exhaustive.

use nits_protocol::{ColRange, Span, SpanClass};
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

/// Owns the loaded syntax set. Expensive to build; share one per process.
pub struct Highlighter {
    syntaxes: SyntaxSet,
}

impl std::fmt::Debug for Highlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighter")
            .field("syntaxes", &self.syntaxes.syntaxes().len())
            .finish()
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            syntaxes: SyntaxSet::load_defaults_newlines(),
        }
    }

    /// Syntax name for a path (by extension, then by first line), if any.
    #[must_use]
    pub fn detect(&self, path: &str, first_line: Option<&str>) -> Option<String> {
        let by_path = self.syntaxes.find_syntax_by_path(path).or_else(|| {
            self.syntaxes
                .find_syntax_by_extension(path.rsplit('.').next()?)
        });
        by_path
            .or_else(|| first_line.and_then(|l| self.syntaxes.find_syntax_by_first_line(l)))
            .map(|s| s.name.clone())
    }

    /// Spans per line for `lang`. Unknown `lang` yields empty spans.
    pub fn highlight<'a>(
        &self,
        lang: &str,
        lines: impl Iterator<Item = &'a str>,
    ) -> Vec<Vec<Span>> {
        let Some(syntax) = self.syntaxes.find_syntax_by_name(lang) else {
            return lines.map(|_| Vec::new()).collect();
        };
        let mut state = ParseState::new(syntax);
        let mut stack = ScopeStack::new();
        let mut out = Vec::new();
        for line in lines {
            // syntect's newline syntaxes expect the terminator present.
            let with_nl = format!("{line}\n");
            let Ok(ops) = state.parse_line(&with_nl, &self.syntaxes) else {
                out.push(Vec::new());
                continue;
            };
            let mut spans: Vec<Span> = Vec::new();
            let mut cursor = 0usize;
            let line_len = line.len();
            let mut current = class_for(stack.as_slice());
            for (offset, op) in &ops {
                let offset = (*offset).min(line_len);
                if offset > cursor {
                    push_span(&mut spans, cursor, offset, current);
                    cursor = offset;
                }
                // A failed op leaves the stack as-is; spans stay conservative.
                let _ = stack.apply(op);
                current = class_for(stack.as_slice());
            }
            if line_len > cursor {
                push_span(&mut spans, cursor, line_len, current);
            }
            out.push(spans);
        }
        out
    }
}

/// Convenience: detect the language for `path` using a shared highlighter.
#[must_use]
pub fn detect_lang(hl: &Highlighter, path: &str, first_line: Option<&str>) -> Option<String> {
    hl.detect(path, first_line)
}

fn push_span(spans: &mut Vec<Span>, start: usize, end: usize, class: Option<SpanClass>) {
    let Some(class) = class else { return };
    let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else {
        return;
    };
    if let Some(last) = spans.last_mut()
        && last.class == class
        && last.range.end() == start
        && let Ok(merged) = ColRange::new(last.range.start(), end)
    {
        last.range = merged;
        return;
    }
    if let Ok(range) = ColRange::new(start, end) {
        spans.push(Span { range, class });
    }
}

/// Most specific scope wins: walk from the top of the stack down.
fn class_for(stack: &[Scope]) -> Option<SpanClass> {
    stack.iter().rev().find_map(|s| classify(&s.build_string()))
}

fn classify(scope: &str) -> Option<SpanClass> {
    let first = scope.split('.').next()?;
    Some(match first {
        "comment" => SpanClass::Comment,
        "string" => SpanClass::String,
        "constant" => {
            if scope.starts_with("constant.numeric") {
                SpanClass::Number
            } else {
                SpanClass::Constant
            }
        }
        "keyword" => {
            if scope.starts_with("keyword.operator") {
                SpanClass::Operator
            } else {
                SpanClass::Keyword
            }
        }
        // `storage.type` (`fn`, `let`, `struct`) and `storage.modifier`
        // (`pub`, `mut`) both read as keywords.
        "storage" => SpanClass::Keyword,
        "entity" => {
            if scope.starts_with("entity.name.function") {
                SpanClass::Function
            } else if scope.starts_with("entity.name.type")
                || scope.starts_with("entity.name.class")
                || scope.starts_with("entity.name.struct")
                || scope.starts_with("entity.name.enum")
                || scope.starts_with("entity.name.trait")
            {
                SpanClass::Type
            } else if scope.starts_with("entity.name.tag") {
                SpanClass::Tag
            } else if scope.starts_with("entity.other.attribute-name") {
                SpanClass::Attribute
            } else {
                return None;
            }
        }
        "support" => {
            if scope.starts_with("support.function") {
                SpanClass::Function
            } else if scope.starts_with("support.type") || scope.starts_with("support.class") {
                SpanClass::Type
            } else {
                SpanClass::Constant
            }
        }
        "variable" => SpanClass::Variable,
        "punctuation" => SpanClass::Punctuation,
        "meta" if scope.starts_with("meta.attribute") || scope.starts_with("meta.annotation") => {
            SpanClass::Attribute
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_keywords_and_functions_are_classified() {
        let hl = Highlighter::new();
        let lang = hl.detect("src/lib.rs", None).unwrap();
        assert_eq!(lang, "Rust");
        let spans = hl.highlight(&lang, ["fn main() { let x = 1; // hi", "}"].into_iter());
        let classes: Vec<(u32, u32, SpanClass)> = spans[0]
            .iter()
            .map(|s| (s.range.start(), s.range.end(), s.class))
            .collect();
        assert!(classes.contains(&(0, 2, SpanClass::Keyword)), "{classes:?}");
        assert!(
            classes
                .iter()
                .any(|(s, e, c)| *s == 3 && *e == 7 && *c == SpanClass::Function),
            "{classes:?}"
        );
        assert!(
            classes.iter().any(|(_, _, c)| *c == SpanClass::Comment),
            "{classes:?}"
        );
        assert!(
            classes.iter().any(|(_, _, c)| *c == SpanClass::Number),
            "{classes:?}"
        );
        for w in spans[0].windows(2) {
            assert!(
                w[0].range.end() <= w[1].range.start(),
                "spans overlap: {w:?}"
            );
        }
    }

    #[test]
    fn unknown_language_yields_empty_spans() {
        let hl = Highlighter::new();
        assert!(hl.detect("Makefile.weird-ext-zzz", None).is_none());
        let spans = hl.highlight("NoSuchLang", ["x"].into_iter());
        assert_eq!(spans, vec![Vec::<Span>::new()]);
    }
}
