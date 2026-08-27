//! Small types whose constructors enforce an invariant: non-empty lists,
//! ordered ranges, 1-based line numbers, repo-relative paths.

use core::fmt;
use core::num::NonZeroU32;

use serde::{Deserialize, Serialize};

/// Error constructing an invariant type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvariantError {
    #[error("list must not be empty")]
    Empty,
    #[error("range start {start} is after end {end}")]
    Inverted { start: u32, end: u32 },
    #[error("invalid repo path {0:?}: {1}")]
    Path(String, &'static str),
}

/// A `Vec` with at least one element.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    try_from = "Vec<T>",
    into = "Vec<T>",
    bound(serialize = "T: Serialize + Clone")
)]
pub struct NonEmpty<T>(Vec<T>);

impl<T> NonEmpty<T> {
    pub fn new(items: Vec<T>) -> Result<Self, InvariantError> {
        if items.is_empty() {
            Err(InvariantError::Empty)
        } else {
            Ok(Self(items))
        }
    }

    pub fn singleton(item: T) -> Self {
        Self(vec![item])
    }

    pub fn first(&self) -> &T {
        &self.0[0]
    }

    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false`; provided so clippy's `len_without_is_empty` is honoured
    /// and so callers can read intent.
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn push(&mut self, item: T) {
        self.0.push(item);
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

impl<T> TryFrom<Vec<T>> for NonEmpty<T> {
    type Error = InvariantError;
    fn try_from(v: Vec<T>) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}

impl<T> From<NonEmpty<T>> for Vec<T> {
    fn from(n: NonEmpty<T>) -> Self {
        n.0
    }
}

impl<'a, T> IntoIterator for &'a NonEmpty<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T> IntoIterator for NonEmpty<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// A 1-based line number within a blob.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineNo(NonZeroU32);

impl LineNo {
    /// Returns `None` for 0.
    #[must_use]
    pub const fn new(n: u32) -> Option<Self> {
        match NonZeroU32::new(n) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// Line 1.
    pub const FIRST: LineNo = LineNo(NonZeroU32::MIN);

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Convert a 0-based index to a line number.
    #[must_use]
    pub const fn from_index(i: u32) -> Self {
        match NonZeroU32::new(i + 1) {
            Some(n) => Self(n),
            None => Self::FIRST,
        }
    }

    /// Convert to a 0-based index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0.get() - 1
    }
}

impl fmt::Debug for LineNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}", self.0)
    }
}

impl fmt::Display for LineNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An inclusive range of lines, `start <= end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawLineRange", into = "RawLineRange")]
pub struct LineRange {
    start: LineNo,
    end: LineNo,
}

#[derive(Serialize, Deserialize)]
struct RawLineRange {
    start: LineNo,
    end: LineNo,
}

impl LineRange {
    pub fn new(start: LineNo, end: LineNo) -> Result<Self, InvariantError> {
        if start <= end {
            Ok(Self { start, end })
        } else {
            Err(InvariantError::Inverted {
                start: start.get(),
                end: end.get(),
            })
        }
    }

    #[must_use]
    pub fn single(line: LineNo) -> Self {
        Self {
            start: line,
            end: line,
        }
    }

    #[must_use]
    pub const fn start(self) -> LineNo {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> LineNo {
        self.end
    }

    /// Number of lines covered (at least 1).
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.get() - self.start.get() + 1
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        false
    }

    #[must_use]
    pub fn contains(self, line: LineNo) -> bool {
        self.start <= line && line <= self.end
    }
}

impl TryFrom<RawLineRange> for LineRange {
    type Error = InvariantError;
    fn try_from(r: RawLineRange) -> Result<Self, Self::Error> {
        Self::new(r.start, r.end)
    }
}

impl From<LineRange> for RawLineRange {
    fn from(r: LineRange) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

/// A half-open byte-column range `[start, end)` within a line, `start <= end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawColRange", into = "RawColRange")]
pub struct ColRange {
    start: u32,
    end: u32,
}

#[derive(Serialize, Deserialize)]
struct RawColRange {
    start: u32,
    end: u32,
}

impl ColRange {
    pub fn new(start: u32, end: u32) -> Result<Self, InvariantError> {
        if start <= end {
            Ok(Self { start, end })
        } else {
            Err(InvariantError::Inverted { start, end })
        }
    }

    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

impl TryFrom<RawColRange> for ColRange {
    type Error = InvariantError;
    fn try_from(r: RawColRange) -> Result<Self, Self::Error> {
        Self::new(r.start, r.end)
    }
}

impl From<ColRange> for RawColRange {
    fn from(r: ColRange) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

/// A repository-relative, `/`-separated, normalised path.
///
/// Invariants: non-empty, no leading `/`, no `.`/`..`/empty components, no
/// trailing `/`, no NUL.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

impl RepoPath {
    pub fn new(s: impl Into<String>) -> Result<Self, InvariantError> {
        let s: String = s.into();
        let bad = |why| InvariantError::Path(s.clone(), why);
        if s.is_empty() {
            return Err(bad("empty"));
        }
        if s.contains('\0') {
            return Err(bad("contains NUL"));
        }
        if s.starts_with('/') {
            return Err(bad("absolute"));
        }
        if s.ends_with('/') {
            return Err(bad("trailing slash"));
        }
        for comp in s.split('/') {
            match comp {
                "" => return Err(bad("empty component")),
                "." | ".." => return Err(bad("relative component")),
                _ => {}
            }
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Final path component.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Parent path, or `None` at the root.
    #[must_use]
    pub fn parent(&self) -> Option<RepoPath> {
        self.0.rsplit_once('/').map(|(p, _)| RepoPath(p.to_owned()))
    }

    /// Path components in order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// Extension without the dot, if any.
    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name();
        name.rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .map(|(_, ext)| ext)
    }
}

impl fmt::Debug for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RepoPath({:?})", self.0)
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RepoPath {
    type Error = InvariantError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<RepoPath> for String {
    fn from(p: RepoPath) -> Self {
        p.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn non_empty_rejects_empty_on_wire() {
        assert!(serde_json::from_str::<NonEmpty<u8>>("[]").is_err());
        let n: NonEmpty<u8> = serde_json::from_str("[1,2]").unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!(serde_json::to_string(&n).unwrap(), "[1,2]");
    }

    #[test]
    fn line_range_rejects_inverted() {
        let a = LineNo::new(5).unwrap();
        let b = LineNo::new(3).unwrap();
        assert!(LineRange::new(a, b).is_err());
        assert!(serde_json::from_str::<LineRange>(r#"{"start":5,"end":3}"#).is_err());
        assert!(serde_json::from_str::<LineRange>(r#"{"start":0,"end":3}"#).is_err());
        let r: LineRange = serde_json::from_str(r#"{"start":3,"end":5}"#).unwrap();
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn repo_path_normalisation() {
        for bad in ["", "/a", "a/", "a//b", "./a", "a/../b", "a\0"] {
            assert!(RepoPath::new(bad).is_err(), "{bad:?} should be rejected");
        }
        let p = RepoPath::new("src/lib.rs").unwrap();
        assert_eq!(p.file_name(), "lib.rs");
        assert_eq!(p.extension(), Some("rs"));
        assert_eq!(p.parent().unwrap().as_str(), "src");
        assert_eq!(RepoPath::new("Makefile").unwrap().parent(), None);
        assert_eq!(RepoPath::new(".gitignore").unwrap().extension(), None);
    }

    proptest! {
        #[test]
        fn line_range_roundtrip(a in 1u32..10_000, b in 1u32..10_000) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let r = LineRange::new(LineNo::new(lo).unwrap(), LineNo::new(hi).unwrap()).unwrap();
            let json = serde_json::to_string(&r).unwrap();
            prop_assert_eq!(serde_json::from_str::<LineRange>(&json).unwrap(), r);
            prop_assert_eq!(r.len(), hi - lo + 1);
        }

        #[test]
        fn repo_path_roundtrip(comps in prop::collection::vec("[a-z0-9_.-]{1,8}", 1..5)) {
            prop_assume!(comps.iter().all(|c| c != "." && c != ".."));
            let s = comps.join("/");
            let p = RepoPath::new(s.clone()).unwrap();
            prop_assert_eq!(p.components().count(), comps.len());
            let json = serde_json::to_string(&p).unwrap();
            prop_assert_eq!(serde_json::from_str::<RepoPath>(&json).unwrap(), p);
        }
    }
}
