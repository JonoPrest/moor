//! Plain-text renderings of protocol values for tool results. Agents read
//! text, so a diff comes back looking like `git diff` with line numbers
//! rather than as row JSON.

use std::fmt::Write as _;

use moor_protocol::{Cell, FileRenderHeader, RenderChunk, RenderContent, Row};

/// A rendered diff or blob as numbered text.
#[must_use]
pub fn render(header: &FileRenderHeader, chunks: &[RenderChunk]) -> String {
    let mut out = String::new();
    if matches!(header.content, RenderContent::Binary) {
        out.push_str("(binary file)\n");
        return out;
    }
    for chunk in chunks {
        for row in &chunk.rows {
            match row {
                Row::HunkHeader { text } => line(&mut out, "", "", '@', text),
                Row::Context { left, right } => {
                    line(&mut out, &no(left), &no(right), ' ', &right.text);
                }
                Row::Removed { left } => line(&mut out, &no(left), "", '-', &left.text),
                Row::Added { right } => line(&mut out, "", &no(right), '+', &right.text),
                Row::Modified { left, right } => {
                    line(&mut out, &no(left), "", '-', &left.text);
                    line(&mut out, "", &no(right), '+', &right.text);
                }
                Row::Expander { hidden, .. } => {
                    line(&mut out, "", "", '~', &format!("{hidden} unchanged lines"));
                }
                Row::WhitespaceOnly => {
                    line(&mut out, "", "", '~', "whitespace-only change");
                }
            }
        }
    }
    out
}

/// A blob as `lineno│text` — blob renders only produce `Context` rows.
#[must_use]
pub fn render_blob(header: &FileRenderHeader, chunks: &[RenderChunk]) -> String {
    let mut out = String::new();
    if matches!(header.content, RenderContent::Binary) {
        out.push_str("(binary file)\n");
        return out;
    }
    for chunk in chunks {
        for row in &chunk.rows {
            if let Row::Context { right, .. } = row {
                let _ = writeln!(out, "{:>5}│{}", right.line_no.get(), right.text);
            }
        }
    }
    out
}

fn no(c: &Cell) -> String {
    c.line_no.get().to_string()
}

fn line(out: &mut String, old: &str, new: &str, mark: char, text: &str) {
    let _ = writeln!(out, "{old:>5} {new:>5} {mark}{text}");
}
