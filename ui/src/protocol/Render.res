// Render model (moor-protocol `render.rs`).

open Ids

type spanClass =
  | Keyword
  | String
  | Number
  | Comment
  | Type
  | Function
  | Variable
  | Constant
  | Operator
  | Punctuation
  | Attribute
  | Tag
  | Other
let spanClass: S.t<spanClass> = S.enum([
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
])

/// Byte offsets, end exclusive.
type colRange = {start: int, end: int}
let colRange: S.t<colRange> = S.object(s => {
  start: s.field("start", S.int),
  end: s.field("end", S.int),
})

type span = {range: colRange, class: spanClass}
let span: S.t<span> = S.object(s => {
  range: s.field("range", colRange),
  class: s.field("class", spanClass),
})

type cell = {lineNo: int, text: string, spans: array<span>, changed: array<colRange>}
let cell: S.t<cell> = S.object(s => {
  lineNo: s.field("line_no", S.int),
  text: s.field("text", S.string),
  spans: s.field("spans", S.array(span)),
  changed: s.field("changed", S.array(colRange)),
})

type expandDir = Up | Down | Both
let expandDir: S.t<expandDir> = S.enum([Up, Down, Both])

type row =
  | HunkHeader({text: string})
  | Context({left: cell, right: cell})
  | Removed({left: cell})
  | Added({right: cell})
  | Modified({left: cell, right: cell})
  | Expander({hidden: int, dir: expandDir})
  | WhitespaceOnly
let row: S.t<row> = S.union([
  S.object(s => {
    s.tag("type", "HunkHeader")
    HunkHeader({text: s.field("text", S.string)})
  }),
  S.object(s => {
    s.tag("type", "Context")
    Context({left: s.field("left", cell), right: s.field("right", cell)})
  }),
  S.object(s => {
    s.tag("type", "Removed")
    Removed({left: s.field("left", cell)})
  }),
  S.object(s => {
    s.tag("type", "Added")
    Added({right: s.field("right", cell)})
  }),
  S.object(s => {
    s.tag("type", "Modified")
    Modified({left: s.field("left", cell), right: s.field("right", cell)})
  }),
  S.object(s => {
    s.tag("type", "Expander")
    Expander({hidden: s.field("hidden", S.int), dir: s.field("dir", expandDir)})
  }),
  S.object(s => {
    s.tag("type", "WhitespaceOnly")
    WhitespaceOnly
  }),
])

type chunkIndex = int
let chunkIndex: S.t<chunkIndex> = S.int

type renderTarget = Diff({change: Domain.changeKind}) | Blob({oid: blobOid})
let renderTarget: S.t<renderTarget> = S.union([
  S.object(s => {
    s.tag("type", "Diff")
    Diff({change: s.field("change", Domain.changeKind)})
  }),
  S.object(s => {
    s.tag("type", "Blob")
    Blob({oid: s.field("oid", blobOid)})
  }),
])

type renderContent =
  | Binary
  | Text({
      totalRows: int,
      chunkRows: int,
      chunkCount: int,
      highlighted: bool,
      additions: int,
      deletions: int,
    })
let renderContent: S.t<renderContent> = S.union([
  S.object(s => {
    s.tag("type", "Binary")
    Binary
  }),
  S.object(s => {
    s.tag("type", "Text")
    Text({
      totalRows: s.field("total_rows", S.int),
      chunkRows: s.field("chunk_rows", S.int),
      chunkCount: s.field("chunk_count", S.int),
      highlighted: s.field("highlighted", S.bool),
      additions: s.field("additions", S.int),
      deletions: s.field("deletions", S.int),
    })
  }),
])

type fileRenderHeader = {
  repoId: repoId,
  path: string,
  target: renderTarget,
  opts: Domain.renderOpts,
  lang: option<string>,
  content: renderContent,
}
let fileRenderHeader: S.t<fileRenderHeader> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  path: s.field("path", S.string),
  target: s.field("target", renderTarget),
  opts: s.field("opts", Domain.renderOpts),
  lang: s.field("lang", S.null(S.string)),
  content: s.field("content", renderContent),
})

type renderChunk = {index: chunkIndex, rows: array<row>}
let renderChunk: S.t<renderChunk> = S.object(s => {
  index: s.field("index", chunkIndex),
  rows: s.field("rows", S.array(row)),
})

type fileRender = {header: fileRenderHeader, chunks: array<renderChunk>}
let fileRender: S.t<fileRender> = S.object(s => {
  header: s.field("header", fileRenderHeader),
  chunks: s.field("chunks", S.array(renderChunk)),
})

type fileSummary = {
  repoId: repoId,
  path: string,
  change: Domain.changeKind,
  additions: int,
  deletions: int,
  binary: bool,
}
let fileSummary: S.t<fileSummary> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  path: s.field("path", S.string),
  change: s.field("change", Domain.changeKind),
  additions: s.field("additions", S.int),
  deletions: s.field("deletions", S.int),
  binary: s.field("binary", S.bool),
})

type diffSummary = {files: array<fileSummary>, additions: int, deletions: int}
let diffSummary: S.t<diffSummary> = S.object(s => {
  files: s.field("files", S.array(fileSummary)),
  additions: s.field("additions", S.int),
  deletions: s.field("deletions", S.int),
})
