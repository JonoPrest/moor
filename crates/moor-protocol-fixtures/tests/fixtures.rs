//! Fixture invariants: every enum variant has a fixture, every fixture
//! round-trips through its type unchanged, and the committed files under
//! `fixtures/protocol/` match what `cargo xtask fixtures` would write.

use std::collections::BTreeSet;
use std::path::PathBuf;

use moor_protocol_fixtures::{all, registry};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/protocol")
        .canonicalize()
        .expect("fixtures/protocol exists; run `cargo xtask fixtures`")
}

#[test]
fn fixtures_cover_every_variant() {
    let mut missing = Vec::new();
    for r in registry() {
        for name in (r.missing_names)().unwrap() {
            missing.push(format!("{}::{name}", r.type_name));
        }
    }
    assert!(
        missing.is_empty(),
        "variants without a fixture: {missing:?}"
    );
}

#[test]
fn fixture_names_are_unique() {
    let mut seen = BTreeSet::new();
    for f in all().unwrap() {
        assert!(
            seen.insert(f.rel_path()),
            "duplicate fixture {}",
            f.rel_path()
        );
    }
}

#[test]
fn every_fixture_roundtrips() {
    for r in registry() {
        for f in (r.fixtures)().unwrap() {
            let back = (r.roundtrip)(&f.value)
                .unwrap_or_else(|e| panic!("{} failed to deserialise: {e}", f.rel_path()));
            assert_eq!(back, f.value, "{} changed through round-trip", f.rel_path());
        }
    }
}

#[test]
fn every_fixture_is_tagged_object_or_scalar() {
    // Enum variants must serialise with a `type` tag (or as a bare string for
    // rename_all enums); structs as objects. Nothing may be an externally
    // tagged `{ "Variant": {...} }` object, which Sury can't mirror cleanly.
    for f in all().unwrap() {
        match &f.value {
            serde_json::Value::Object(map) => {
                let single_capitalised_key = map.len() == 1
                    && map
                        .keys()
                        .next()
                        .is_some_and(|k| k.chars().next().is_some_and(char::is_uppercase));
                assert!(
                    !single_capitalised_key,
                    "{} looks externally tagged: {}",
                    f.rel_path(),
                    f.value
                );
            }
            serde_json::Value::String(_) | serde_json::Value::Number(_) => {}
            other => panic!("{} has unexpected shape: {other}", f.rel_path()),
        }
    }
}

#[test]
fn committed_fixture_files_are_current() {
    let dir = fixtures_dir();
    let mut expected = BTreeSet::new();
    let mut stale = Vec::new();
    for f in all().unwrap() {
        let path = dir.join(f.rel_path());
        expected.insert(path.clone());
        let mut want = serde_json::to_string_pretty(&f.value).unwrap();
        want.push('\n');
        match std::fs::read_to_string(&path) {
            Ok(have) if have == want => {}
            Ok(_) => stale.push(format!("{} differs", f.rel_path())),
            Err(_) => stale.push(format!("{} missing", f.rel_path())),
        }
    }
    for entry in walkdir(&dir) {
        if !expected.contains(&entry) {
            stale.push(format!("{} is orphaned", entry.display()));
        }
    }
    assert!(
        stale.is_empty(),
        "fixtures out of date; run `cargo xtask fixtures`:\n{}",
        stale.join("\n")
    );
}

fn walkdir(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            out.extend(walkdir(&p));
        } else if p.extension().is_some_and(|e| e == "json") {
            out.push(p);
        }
    }
    out
}

#[test]
fn unknown_fields_are_rejected() {
    // deny_unknown_fields must hold on tagged enums and structs alike.
    // (serde does not enforce it on *unit* variants of internally tagged
    // enums — `{"type":"Review","extra":1}` parses — so struct variants are
    // what we check; the ReScript schemas are strict on both.)
    let mut v = serde_json::json!({"type": "File", "repo_id": "01HF6EQ5X00000000000000002",
        "path": "a", "blob_oid": "0000000000000000000000000000000000000000"});
    v["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<moor_protocol::Anchor>(v).is_err());

    let mut v = serde_json::json!({"ignore_whitespace": false, "context_lines": 3});
    v["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<moor_protocol::RenderOpts>(v).is_err());
}
