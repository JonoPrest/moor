// Domain types (moor-protocol `domain.rs`, `invariants.rs`). Payload enums
// are `{"type": ...}` tagged objects; unit-only enums are bare strings.

open Ids

type repo = {id: repoId, path: string, displayName: string}
let repo: S.t<repo> = S.object(s => {
  id: s.field("id", repoId),
  path: s.field("path", S.string),
  displayName: s.field("display_name", S.string),
})

type workspace = {id: workspaceId, name: string, repos: array<repo>}
let workspace: S.t<workspace> = S.object(s => {
  id: s.field("id", workspaceId),
  name: s.field("name", S.string),
  repos: s.field("repos", S.array(repo)),
})

type human = {name: string, machine: string}
let human: S.t<human> = S.object(s => {
  name: s.field("name", S.string),
  machine: s.field("machine", S.string),
})

type agentVia = Cli | Mcp
let agentVia: S.t<agentVia> = S.enum([Cli, Mcp])

type author =
  | Human({name: string, machine: string})
  | Agent({name: string, model: string, sessionId: string, invokedBy: option<human>, via: agentVia})
  | Daemon({machine: string})
let author: S.t<author> = S.union([
  S.object(s => {
    s.tag("type", "Human")
    Human({name: s.field("name", S.string), machine: s.field("machine", S.string)})
  }),
  S.object(s => {
    s.tag("type", "Agent")
    Agent({
      name: s.field("name", S.string),
      model: s.field("model", S.string),
      sessionId: s.field("session_id", S.string),
      invokedBy: s.field("invoked_by", S.null(human)),
      via: s.field("via", agentVia),
    })
  }),
  S.object(s => {
    s.tag("type", "Daemon")
    Daemon({machine: s.field("machine", S.string)})
  }),
])

type refSpec =
  | Branch({name: string})
  | Commit({oid: commitOid})
  | Tag({name: string})
  | WorkingTree
  | Upstream
  | Head
let refSpec: S.t<refSpec> = S.union([
  S.object(s => {
    s.tag("type", "Branch")
    Branch({name: s.field("name", S.string)})
  }),
  S.object(s => {
    s.tag("type", "Commit")
    Commit({oid: s.field("oid", commitOid)})
  }),
  S.object(s => {
    s.tag("type", "Tag")
    Tag({name: s.field("name", S.string)})
  }),
  S.object(s => {
    s.tag("type", "WorkingTree")
    WorkingTree
  }),
  S.object(s => {
    s.tag("type", "Upstream")
    Upstream
  }),
  S.object(s => {
    s.tag("type", "Head")
    Head
  }),
])

type resolvedSource = SourceCommit({oid: commitOid}) | SourceWorkingTree({dirty: array<string>})
let resolvedSource: S.t<resolvedSource> = S.union([
  S.object(s => {
    s.tag("type", "Commit")
    SourceCommit({oid: s.field("oid", commitOid)})
  }),
  S.object(s => {
    s.tag("type", "WorkingTree")
    SourceWorkingTree({dirty: s.field("dirty", S.array(S.string))})
  }),
])

type resolvedRef = {tree: treeOid, source: resolvedSource}
let resolvedRef: S.t<resolvedRef> = S.object(s => {
  tree: s.field("tree", treeOid),
  source: s.field("source", resolvedSource),
})

type reviewTarget = {repoId: repoId, base: refSpec, head: refSpec}
let reviewTarget: S.t<reviewTarget> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  base: s.field("base", refSpec),
  head: s.field("head", refSpec),
})

type resolvedTarget = {repoId: repoId, base: resolvedRef, head: resolvedRef}
let resolvedTarget: S.t<resolvedTarget> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  base: s.field("base", resolvedRef),
  head: s.field("head", resolvedRef),
})

type reviewStatus = Open | Archived
let reviewStatus: S.t<reviewStatus> = S.enum([Open, Archived])

type review = {
  id: reviewId,
  workspaceId: workspaceId,
  title: string,
  targets: array<reviewTarget>,
  created: timestamp,
  status: reviewStatus,
}
let review: S.t<review> = S.object(s => {
  id: s.field("id", reviewId),
  workspaceId: s.field("workspace_id", workspaceId),
  title: s.field("title", S.string),
  targets: s.field("targets", S.array(reviewTarget)),
  created: s.field("created", timestamp),
  status: s.field("status", reviewStatus),
})

type sig = {name: string, email: string, time: timestamp, offsetMinutes: int}
let sig: S.t<sig> = S.object(s => {
  name: s.field("name", S.string),
  email: s.field("email", S.string),
  time: s.field("time", timestamp),
  offsetMinutes: s.field("offset_minutes", S.int),
})

type commitInfo = {
  oid: commitOid,
  parents: array<commitOid>,
  tree: treeOid,
  author: sig,
  committer: sig,
  subject: string,
  body: string,
}
let commitInfo: S.t<commitInfo> = S.object(s => {
  oid: s.field("oid", commitOid),
  parents: s.field("parents", S.array(commitOid)),
  tree: s.field("tree", treeOid),
  author: s.field("author", sig),
  committer: s.field("committer", sig),
  subject: s.field("subject", S.string),
  body: s.field("body", S.string),
})

type side = Base | Head
let side: S.t<side> = S.enum([Base, Head])

/// 16 hex chars.
type contextHash = string
let contextHash: S.t<contextHash> = S.string->S.stringLength(16)

/// 1-based, inclusive.
type lineRange = {start: int, end: int}
let lineRange: S.t<lineRange> = S.object(s => {
  start: s.field("start", S.int),
  end: s.field("end", S.int),
})

type anchor =
  | Review
  | File({repoId: repoId, path: string, blobOid: blobOid})
  | Lines({
      repoId: repoId,
      path: string,
      side: side,
      blobOid: blobOid,
      lines: lineRange,
      contextHash: contextHash,
    })
let anchor: S.t<anchor> = S.union([
  S.object(s => {
    s.tag("type", "Review")
    Review
  }),
  S.object(s => {
    s.tag("type", "File")
    File({
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      blobOid: s.field("blob_oid", blobOid),
    })
  }),
  S.object(s => {
    s.tag("type", "Lines")
    Lines({
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      side: s.field("side", side),
      blobOid: s.field("blob_oid", blobOid),
      lines: s.field("lines", lineRange),
      contextHash: s.field("context_hash", contextHash),
    })
  }),
])

type commentKind = Note | Suggestion({patch: string}) | Request
let commentKind: S.t<commentKind> = S.union([
  S.object(s => {
    s.tag("type", "Note")
    Note
  }),
  S.object(s => {
    s.tag("type", "Suggestion")
    Suggestion({patch: s.field("patch", S.string)})
  }),
  S.object(s => {
    s.tag("type", "Request")
    Request
  }),
])

type commentState = Live | Outdated({lastGoodAnchor: anchor}) | Deleted
let commentState: S.t<commentState> = S.union([
  S.object(s => {
    s.tag("type", "Live")
    Live
  }),
  S.object(s => {
    s.tag("type", "Outdated")
    Outdated({lastGoodAnchor: s.field("last_good_anchor", anchor)})
  }),
  S.object(s => {
    s.tag("type", "Deleted")
    Deleted
  }),
])

type comment = {
  id: commentId,
  reviewId: reviewId,
  threadId: threadId,
  author: author,
  kind: commentKind,
  anchor: anchor,
  body: string,
  created: timestamp,
  edited: option<timestamp>,
  state: commentState,
}
let comment: S.t<comment> = S.object(s => {
  id: s.field("id", commentId),
  reviewId: s.field("review_id", reviewId),
  threadId: s.field("thread_id", threadId),
  author: s.field("author", author),
  kind: s.field("kind", commentKind),
  anchor: s.field("anchor", anchor),
  body: s.field("body", S.string),
  created: s.field("created", timestamp),
  edited: s.field("edited", S.null(timestamp)),
  state: s.field("state", commentState),
})

type threadResolution = ResolutionOpen | Resolved({by: author, at: timestamp})
let threadResolution: S.t<threadResolution> = S.union([
  S.object(s => {
    s.tag("type", "Open")
    ResolutionOpen
  }),
  S.object(s => {
    s.tag("type", "Resolved")
    Resolved({by: s.field("by", author), at: s.field("at", timestamp)})
  }),
])

type thread = {
  id: threadId,
  reviewId: reviewId,
  root: commentId,
  replies: array<commentId>,
  resolution: threadResolution,
}
let thread: S.t<thread> = S.object(s => {
  id: s.field("id", threadId),
  reviewId: s.field("review_id", reviewId),
  root: s.field("root", commentId),
  replies: s.field("replies", S.array(commentId)),
  resolution: s.field("resolution", threadResolution),
})

type viewedMark = {
  reviewId: reviewId,
  repoId: repoId,
  path: string,
  viewer: human,
  blobOid: option<blobOid>,
}
let viewedMark: S.t<viewedMark> = S.object(s => {
  reviewId: s.field("review_id", reviewId),
  repoId: s.field("repo_id", repoId),
  path: s.field("path", S.string),
  viewer: s.field("viewer", human),
  blobOid: s.field("blob_oid", S.null(blobOid)),
})

type renderOpts = {ignoreWhitespace: bool, contextLines: int}
let renderOpts: S.t<renderOpts> = S.object(s => {
  ignoreWhitespace: s.field("ignore_whitespace", S.bool),
  contextLines: s.field("context_lines", S.int),
})

type changeKind =
  | Added({new: blobOid})
  | Deleted({old: blobOid})
  | Modified({old: blobOid, new: blobOid})
  | Renamed({from: string, old: blobOid, new: blobOid})
let changeKind: S.t<changeKind> = S.union([
  S.object(s => {
    s.tag("type", "Added")
    Added({new: s.field("new", blobOid)})
  }),
  S.object(s => {
    s.tag("type", "Deleted")
    Deleted({old: s.field("old", blobOid)})
  }),
  S.object(s => {
    s.tag("type", "Modified")
    Modified({old: s.field("old", blobOid), new: s.field("new", blobOid)})
  }),
  S.object(s => {
    s.tag("type", "Renamed")
    Renamed({
      from: s.field("from", S.string),
      old: s.field("old", blobOid),
      new: s.field("new", blobOid),
    })
  }),
])

type fileChange = {repoId: repoId, path: string, kind: changeKind}
let fileChange: S.t<fileChange> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  path: s.field("path", S.string),
  kind: s.field("kind", changeKind),
})

type treeEntryKind =
  | EntryFile({oid: blobOid, size: float, executable: bool})
  | EntryDir({oid: treeOid})
  | EntrySymlink({oid: blobOid})
  | EntrySubmodule({commit: commitOid})
let treeEntryKind: S.t<treeEntryKind> = S.union([
  S.object(s => {
    s.tag("type", "File")
    EntryFile({
      oid: s.field("oid", blobOid),
      size: s.field("size", S.float),
      executable: s.field("executable", S.bool),
    })
  }),
  S.object(s => {
    s.tag("type", "Dir")
    EntryDir({oid: s.field("oid", treeOid)})
  }),
  S.object(s => {
    s.tag("type", "Symlink")
    EntrySymlink({oid: s.field("oid", blobOid)})
  }),
  S.object(s => {
    s.tag("type", "Submodule")
    EntrySubmodule({commit: s.field("commit", commitOid)})
  }),
])

type treeEntry = {path: string, kind: treeEntryKind}
let treeEntry: S.t<treeEntry> = S.object(s => {
  path: s.field("path", S.string),
  kind: s.field("kind", treeEntryKind),
})

type treeSnapshot = {repoId: repoId, rootOid: treeOid, entries: array<treeEntry>}
let treeSnapshot: S.t<treeSnapshot> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  rootOid: s.field("root_oid", treeOid),
  entries: s.field("entries", S.array(treeEntry)),
})

type treeDelta = {
  repoId: repoId,
  fromRoot: treeOid,
  toRoot: treeOid,
  added: array<treeEntry>,
  removed: array<string>,
  changed: array<treeEntry>,
}
let treeDelta: S.t<treeDelta> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  fromRoot: s.field("from_root", treeOid),
  toRoot: s.field("to_root", treeOid),
  added: s.field("added", S.array(treeEntry)),
  removed: s.field("removed", S.array(S.string)),
  changed: s.field("changed", S.array(treeEntry)),
})

type reviewSnapshot = {
  review: review,
  resolved: option<array<resolvedTarget>>,
  threads: array<thread>,
  comments: array<comment>,
  viewed: array<viewedMark>,
  seq: seq,
}
let reviewSnapshot: S.t<reviewSnapshot> = S.object(s => {
  review: s.field("review", review),
  resolved: s.field("resolved", S.null(S.array(resolvedTarget))),
  threads: s.field("threads", S.array(thread)),
  comments: s.field("comments", S.array(comment)),
  viewed: s.field("viewed", S.array(viewedMark)),
  seq: s.field("seq", seq),
})
