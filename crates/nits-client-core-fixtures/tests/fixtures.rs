//! Coverage, round-trip and freshness of `fixtures/client/`, mirroring the
//! protocol fixture tests.

use std::collections::BTreeSet;
use std::path::PathBuf;

use nits_client_core_fixtures::{all, registry};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("client")
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
    for f in all().unwrap() {
        if let serde_json::Value::Object(map) = &f.value {
            let externally_tagged = map.len() == 1
                && map
                    .keys()
                    .next()
                    .is_some_and(|k| k.chars().next().is_some_and(char::is_uppercase));
            assert!(
                !externally_tagged,
                "{} looks externally tagged: {}",
                f.rel_path(),
                f.value
            );
        }
    }
}

#[test]
fn committed_fixture_files_are_current() {
    let dir = fixtures_dir();
    let mut stale = Vec::new();
    for f in all().unwrap() {
        let path = dir.join(f.rel_path());
        let mut want = serde_json::to_string_pretty(&f.value).unwrap();
        want.push('\n');
        if std::fs::read_to_string(&path).ok().as_deref() != Some(want.as_str()) {
            stale.push(f.rel_path());
        }
    }
    assert!(
        stale.is_empty(),
        "run `cargo xtask fixtures`; stale or missing: {stale:?}"
    );
}
