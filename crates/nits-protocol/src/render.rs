//! Diff render model: the flat list of rows the screen shows.
//!
//! Pure function of `(old blob, new blob, opts)`, produced by the daemon,
//! cached on disk, identical for every client. Comment-agnostic: overlays are
//! the client's job. See `docs/ARCHITECTURE.md` §4.6.

use serde::{Deserialize, Serialize};
use strum::{EnumDiscriminants, EnumIter};

use crate::domain::{ChangeKind, RenderOpts};
use crate::ids::{BlobOid, RepoId};
use crate::invariants::{ColRange, LineNo, RepoPath};

/// Syntax-highlight class for a span. A closed set so the UI's stylesheet
/// is exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum SpanClass {
    Keyword,
    String,
    Number,
    Comment,
    Type,
    Function,
    Variable,
    Constant,
    Operator,
    Punctuation,
    Attribute,
    Tag,
    Other,
}

/// A highlighted byte range within a cell's text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub range: ColRange,
    pub class: SpanClass,
}

/// One side of a row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub line_no: LineNo,
    /// Original line text without the trailing newline. Whitespace-ignored
    /// diffs still carry the real text.
    pub text: String,
    pub spans: Vec<Span>,
    /// Intra-line changed ranges (only non-empty on `Modified` rows).
    pub changed: Vec<ColRange>,
}

/// Which direction an expander reveals hidden lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ExpandDir {
    Up,
    Down,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(RowKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Row {
    HunkHeader {
        text: String,
    },
    Context {
        left: Cell,
        right: Cell,
    },
    Removed {
        left: Cell,
    },
    Added {
        right: Cell,
    },
    /// A paired `-`/`+` line with intra-line ranges.
    Modified {
        left: Cell,
        right: Cell,
    },
    /// "show N more lines".
    Expander {
        hidden: u32,
        dir: ExpandDir,
    },
    /// The whole file differs only in whitespace and `ignore_whitespace` is on.
    WhitespaceOnly,
}

/// Index of a chunk within a file's rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct ChunkIndex(u32);

impl ChunkIndex {
    pub const FIRST: ChunkIndex = ChunkIndex(0);

    #[must_use]
    pub const fn new(i: u32) -> Self {
        Self(i)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// What a render is of: a diff between two blobs, or a single blob (explorer).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(RenderTargetKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum RenderTarget {
    Diff { change: ChangeKind },
    Blob { oid: BlobOid },
}

/// Row-level shape of a rendered file, known before any chunk.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(RenderContentKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum RenderContent {
    /// No rows; UI shows a binary placeholder.
    Binary,
    Text {
        total_rows: u32,
        /// Rows per chunk (last chunk may be shorter).
        chunk_rows: u32,
        chunk_count: u32,
        /// `false` when the file exceeded the highlight size cap.
        highlighted: bool,
        additions: u32,
        deletions: u32,
    },
}

/// Header for a rendered file: everything but the rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FileRenderHeader {
    pub repo_id: RepoId,
    pub path: RepoPath,
    pub target: RenderTarget,
    pub opts: RenderOpts,
    /// Language used for highlighting, as a syntect/linguist-style name.
    pub lang: Option<String>,
    pub content: RenderContent,
}

/// A contiguous slice of a file's rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RenderChunk {
    pub index: ChunkIndex,
    pub rows: Vec<Row>,
}

/// A whole rendered file, for non-streamed consumers (CLI, MCP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FileRender {
    pub header: FileRenderHeader,
    pub chunks: Vec<RenderChunk>,
}

/// Per-file line counts, for the tree and progress views.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FileSummary {
    pub repo_id: RepoId,
    pub path: RepoPath,
    pub change: ChangeKind,
    pub additions: u32,
    pub deletions: u32,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiffSummary {
    pub files: Vec<FileSummary>,
    pub additions: u32,
    pub deletions: u32,
}
