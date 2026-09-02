#![allow(clippy::format_collect, clippy::naive_bytecount)] // test data builders
//! Render model: snapshot corpus + invariants.

use nits_protocol::{ChunkIndex, ExpandDir, RenderContent, RenderOpts, Row};
use nits_review_core::render::{CHUNK_ROWS, Highlighter, Rendered, render_blob, render_file};
use proptest::prelude::*;
use std::sync::LazyLock;

static HL: LazyLock<Highlighter> = LazyLock::new(Highlighter::new);

fn opts(ignore_ws: bool) -> RenderOpts {
    RenderOpts {
        ignore_whitespace: ignore_ws,
        context_lines: 3,
    }
}

const RUST_OLD: &str = "\
use std::fmt;

fn main() {
    let x = 1;
    let y = 2;
    println!(\"{}\", x + y);
}

fn helper(a: u32) -> u32 {
    a * 2
}

fn unchanged_1() {}
fn unchanged_2() {}
fn unchanged_3() {}
fn unchanged_4() {}
fn unchanged_5() {}
fn unchanged_6() {}
fn unchanged_7() {}
fn unchanged_8() {}

fn tail() -> bool {
    true
}
";

const RUST_NEW: &str = "\
use std::fmt;

fn main() {
    let x: u32 = 1;
    let y = 2;
    let z = 3;
    println!(\"{}\", x + y + z);
}

fn helper(a: u32) -> u32 {
    a * 2
}

fn unchanged_1() {}
fn unchanged_2() {}
fn unchanged_3() {}
fn unchanged_4() {}
fn unchanged_5() {}
fn unchanged_6() {}
fn unchanged_7() {}
fn unchanged_8() {}

fn tail() -> bool {
    false
}
";

struct Case {
    name: &'static str,
    old: Option<&'static [u8]>,
    new: Option<&'static [u8]>,
    lang: Option<&'static str>,
}

fn corpus() -> Vec<Case> {
    vec![
        Case {
            name: "added",
            old: None,
            new: Some(RUST_NEW.as_bytes()),
            lang: Some("Rust"),
        },
        Case {
            name: "deleted",
            old: Some(RUST_OLD.as_bytes()),
            new: None,
            lang: Some("Rust"),
        },
        Case {
            name: "modified",
            old: Some(RUST_OLD.as_bytes()),
            new: Some(RUST_NEW.as_bytes()),
            lang: Some("Rust"),
        },
        Case {
            name: "unhighlighted",
            old: Some(RUST_OLD.as_bytes()),
            new: Some(RUST_NEW.as_bytes()),
            lang: None,
        },
        Case {
            name: "whitespace_only",
            old: Some(b"fn a() {\n    x();\n}\n"),
            new: Some(b"fn a() {\n\tx();\n}\n"),
            lang: Some("Rust"),
        },
        Case {
            name: "reindented_block_plus_real_change",
            old: Some(b"a\nb\n  c\n  d\ne\nf\n"),
            new: Some(b"a\nb\n    c\n    d\ne\nF\n"),
            lang: None,
        },
        Case {
            name: "binary",
            old: Some(b"\x00\x01\x02"),
            new: Some(b"\x00\x01\x03"),
            lang: None,
        },
        Case {
            name: "no_trailing_newline",
            old: Some(b"one\ntwo"),
            new: Some(b"one\ntwo\n"),
            lang: None,
        },
        Case {
            name: "crlf",
            old: Some(b"a\r\nb\r\nc\r\n"),
            new: Some(b"a\r\nB\r\nc\r\n"),
            lang: None,
        },
        Case {
            name: "identical",
            old: Some(b"a\nb\nc\n"),
            new: Some(b"a\nb\nc\n"),
            lang: None,
        },
        Case {
            name: "empty_to_empty",
            old: Some(b""),
            new: Some(b""),
            lang: None,
        },
    ]
}

#[test]
fn snapshot_corpus() {
    for case in corpus() {
        for ignore_ws in [false, true] {
            let r = render_file(&HL, case.old, case.new, case.lang, opts(ignore_ws));
            check_invariants(&r, case.old, case.new);
            let suffix = if ignore_ws { "ignore_ws" } else { "exact" };
            insta::assert_json_snapshot!(format!("{}__{suffix}", case.name), r);
        }
    }
}

#[test]
fn blob_render_is_all_context() {
    let r = render_blob(&HL, RUST_NEW.as_bytes(), Some("Rust"));
    assert!(r.rows.iter().all(|row| matches!(row, Row::Context { .. })));
    assert_eq!(r.rows.len(), RUST_NEW.lines().count());
    insta::assert_json_snapshot!("blob_rust", r);
}

#[test]
fn ignore_whitespace_hides_reindent_but_keeps_text() {
    let old = b"fn a() {\n    x();\n    y();\n}\n";
    let new = b"fn a() {\n        x();\n        y();\n}\n";
    let exact = render_file(&HL, Some(old), Some(new), None, opts(false));
    assert_eq!(
        exact
            .rows
            .iter()
            .filter(|r| matches!(r, Row::Modified { .. }))
            .count(),
        2
    );
    let ws = render_file(&HL, Some(old), Some(new), None, opts(true));
    assert_eq!(ws.rows, vec![Row::WhitespaceOnly]);

    // With a real change elsewhere, the reindented lines are plain context
    // and still carry their real (reindented) text.
    let new2 = b"fn a() {\n        x();\n        y();\n}\nextra\n";
    let ws2 = render_file(&HL, Some(old), Some(new2), None, opts(true));
    assert_eq!(
        ws2.rows
            .iter()
            .filter(|r| matches!(r, Row::Modified { .. }))
            .count(),
        0
    );
    let ctx_texts: Vec<&str> = ws2
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::Context { right, .. } => Some(right.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(ctx_texts.contains(&"        x();"));
}

#[test]
fn chunks_are_fixed_size_and_concatenate() {
    let old: String = (0..1234).map(|i| format!("line {i}\n")).collect();
    let new: String = (0..1234)
        .map(|i| {
            if i % 100 == 0 {
                format!("LINE {i}\n")
            } else {
                format!("line {i}\n")
            }
        })
        .collect();
    let r = render_file(
        &HL,
        Some(old.as_bytes()),
        Some(new.as_bytes()),
        None,
        RenderOpts {
            ignore_whitespace: false,
            context_lines: 1000,
        },
    );
    let RenderContent::Text {
        total_rows,
        chunk_rows,
        chunk_count,
        ..
    } = r.content
    else {
        panic!()
    };
    assert_eq!(chunk_rows, CHUNK_ROWS);
    assert_eq!(total_rows as usize, r.rows.len());
    assert_eq!(chunk_count, r.chunk_count());
    let joined: Vec<Row> = r.chunks().flat_map(|c| c.rows).collect();
    assert_eq!(joined, r.rows);
    assert!(r.chunks().all(|c| c.rows.len() <= CHUNK_ROWS as usize));
    assert!(r.chunk(ChunkIndex::new(chunk_count)).is_none());
}

#[test]
fn large_file_renders_quickly_and_without_highlight_above_cap() {
    let old: String = (0..100_000)
        .map(|i| format!("fn f{i}() {{ let v = {i}; }}\n"))
        .collect();
    let mut new = old.clone();
    new.push_str("fn extra() {}\n");
    let start = std::time::Instant::now();
    let r = render_file(
        &HL,
        Some(old.as_bytes()),
        Some(new.as_bytes()),
        Some("Rust"),
        opts(false),
    );
    let elapsed = start.elapsed();
    let RenderContent::Text {
        highlighted,
        additions,
        ..
    } = r.content
    else {
        panic!()
    };
    assert!(!highlighted, "100k lines is above the highlight cap");
    assert_eq!(additions, 1);
    assert!(elapsed.as_secs() < 5, "took {elapsed:?}");
}

// ---- invariants ------------------------------------------------------------

fn check_invariants(r: &Rendered, old: Option<&[u8]>, new: Option<&[u8]>) {
    let RenderContent::Text {
        total_rows,
        chunk_count,
        ..
    } = r.content
    else {
        assert!(r.rows.is_empty());
        return;
    };
    assert_eq!(total_rows as usize, r.rows.len());
    assert_eq!(chunk_count, r.chunk_count());
    if r.rows == vec![Row::WhitespaceOnly] {
        return;
    }
    let count_lines = |b: Option<&[u8]>| {
        b.map_or(0usize, |b| {
            let n = b.iter().filter(|c| **c == b'\n').count();
            if b.is_empty() || b.ends_with(b"\n") {
                n
            } else {
                n + 1
            }
        })
    };
    let (old_n, new_n) = (count_lines(old), count_lines(new));

    let (mut left_seen, mut right_seen, mut hidden) = (0usize, 0usize, 0usize);
    let (mut last_left, mut last_right) = (0u32, 0u32);
    for row in &r.rows {
        let cells: Vec<(&nits_protocol::Cell, bool)> = match row {
            Row::Context { left, right } | Row::Modified { left, right } => {
                vec![(left, true), (right, false)]
            }
            Row::Removed { left } => vec![(left, true)],
            Row::Added { right } => vec![(right, false)],
            Row::Expander { hidden: h, .. } => {
                hidden += *h as usize;
                vec![]
            }
            Row::HunkHeader { .. } | Row::WhitespaceOnly => vec![],
        };
        for (cell, is_left) in cells {
            let n = cell.line_no.get();
            if is_left {
                assert!(n > last_left, "left line numbers not increasing at {n}");
                last_left = n;
                left_seen += 1;
            } else {
                assert!(n > last_right, "right line numbers not increasing at {n}");
                last_right = n;
                right_seen += 1;
            }
            let len = u32::try_from(cell.text.len()).unwrap();
            let mut prev_end = 0;
            for s in &cell.spans {
                assert!(
                    s.range.start() >= prev_end && s.range.end() <= len,
                    "span {s:?} out of bounds/overlapping in {:?}",
                    cell.text
                );
                prev_end = s.range.end();
            }
            for c in &cell.changed {
                assert!(
                    c.end() <= len,
                    "changed {c:?} out of bounds in {:?}",
                    cell.text
                );
            }
        }
    }
    assert_eq!(
        left_seen + hidden,
        old_n,
        "every old line appears once (visible or hidden)"
    );
    assert_eq!(
        right_seen + hidden,
        new_n,
        "every new line appears once (visible or hidden)"
    );
    if let Some(Row::Expander { dir, .. }) = r.rows.first() {
        assert!(matches!(dir, ExpandDir::Up | ExpandDir::Both));
    }
    if let Some(Row::Expander { dir, .. }) = r.rows.last() {
        assert!(matches!(dir, ExpandDir::Down | ExpandDir::Both));
    }
}

fn text_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            Just("a"),
            Just("b"),
            Just("c"),
            Just("  a"),
            Just("\tb"),
            Just(""),
            Just("x y")
        ],
        0..40,
    )
    .prop_map(|lines| {
        let mut s = lines.join("\n");
        if !s.is_empty() {
            s.push('\n');
        }
        s.into_bytes()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn invariants_hold(old in text_strategy(), new in text_strategy(), ctx in 0u32..5, ws in any::<bool>()) {
        let r = render_file(&HL, Some(&old), Some(&new), None, RenderOpts { ignore_whitespace: ws, context_lines: ctx });
        check_invariants(&r, Some(&old), Some(&new));
    }

    #[test]
    fn edits_that_only_change_whitespace_never_produce_modified_rows(
        lines in prop::collection::vec("[a-z]{1,5}", 1..20),
        indents in prop::collection::vec(0usize..4, 1..20),
    ) {
        let old: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let new: String = lines.iter().zip(indents.iter().cycle()).map(|(l, i)| format!("{}{l}\n", " ".repeat(*i))).collect();
        let r = render_file(&HL, Some(old.as_bytes()), Some(new.as_bytes()), None, opts(true));
        prop_assert!(r.rows.iter().all(|row| !matches!(row, Row::Modified { .. } | Row::Added { .. } | Row::Removed { .. })), "{:?}", r.rows);
    }
}
