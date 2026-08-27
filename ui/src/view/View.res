// The client `ViewModel` and everything it contains (moor-client-core
// `view.rs`, `explorer.rs`, `diff.rs`, `focus.rs`, `keymap.rs`). Each enum
// lives in its own module so constructor names never collide.

open Ids

module Layout = {
  type t = Unified | Split
  let schema: S.t<t> = S.enum([Unified, Split])
}

type viewPrefs = {layout: Layout.t, ignoreWhitespace: bool, contextLines: int}
let viewPrefs: S.t<viewPrefs> = S.object(s => {
  layout: s.field("layout", Layout.schema),
  ignoreWhitespace: s.field("ignore_whitespace", S.bool),
  contextLines: s.field("context_lines", S.int),
})

module ConnectionView = {
  type t = Disconnected | Connecting | Subscribed | Rejected({error: Rpc.rpcError})
  let schema: S.t<t> = S.union([
    S.object(s => {
      s.tag("type", "Disconnected")
      Disconnected
    }),
    S.object(s => {
      s.tag("type", "Connecting")
      Connecting
    }),
    S.object(s => {
      s.tag("type", "Subscribed")
      Subscribed
    }),
    S.object(s => {
      s.tag("type", "Rejected")
      Rejected({error: s.field("error", Rpc.rpcError)})
    }),
  ])
}

type draft = {anchor: Domain.anchor, replyTo: option<threadId>}
let draft: S.t<draft> = S.object(s => {
  anchor: s.field("anchor", Domain.anchor),
  replyTo: s.field("reply_to", S.null(threadId)),
})

type pendingEvent = {clientSeq: clientSeq, body: Events.eventBody}
let pendingEvent: S.t<pendingEvent> = S.object(s => {
  clientSeq: s.field("client_seq", clientSeq),
  body: s.field("body", Events.eventBody),
})

type fileRef = {repoId: repoId, path: string}
let fileRef: S.t<fileRef> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  path: s.field("path", S.string),
})

type renderKey = {
  repoId: repoId,
  path: string,
  target: Render.renderTarget,
  opts: Domain.renderOpts,
}
let renderKey: S.t<renderKey> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  path: s.field("path", S.string),
  target: s.field("target", Render.renderTarget),
  opts: s.field("opts", Domain.renderOpts),
})

type openFile = {render: renderKey, firstRow: int, lastRow: int}
let openFile: S.t<openFile> = S.object(s => {
  render: s.field("render", renderKey),
  firstRow: s.field("first_row", S.int),
  lastRow: s.field("last_row", S.int),
})

type openReview = {
  snapshot: Domain.reviewSnapshot,
  pending: array<pendingEvent>,
  trees: array<treeOid>,
  files: array<renderKey>,
  openFile: option<openFile>,
}
let openReview: S.t<openReview> = S.object(s => {
  snapshot: s.field("snapshot", Domain.reviewSnapshot),
  pending: s.field("pending", S.array(pendingEvent)),
  trees: s.field("trees", S.array(treeOid)),
  files: s.field("files", S.array(renderKey)),
  openFile: s.field("open_file", S.null(openFile)),
})

module ViewedState = {
  type t = Viewed | ChangedSinceViewed | Unviewed
  let schema: S.t<t> = S.enum([Viewed, ChangedSinceViewed, Unviewed])
}

module ChangeKindKind = {
  type t = Added | Deleted | Modified | Renamed
  let schema: S.t<t> = S.enum([Added, Deleted, Modified, Renamed])
}

module TreeNode = {
  type rec t =
    | Dir({
        name: string,
        repoId: repoId,
        path: option<string>,
        expanded: bool,
        changedBelow: int,
        children: array<t>,
      })
    | File({
        name: string,
        repoId: repoId,
        path: string,
        change: option<ChangeKindKind.t>,
        viewed: ViewedState.t,
        open_: bool,
      })
  let schema: S.t<t> = S.recursive(self =>
    S.union([
      S.object(s => {
        s.tag("type", "Dir")
        Dir({
          name: s.field("name", S.string),
          repoId: s.field("repo_id", repoId),
          path: s.field("path", S.null(S.string)),
          expanded: s.field("expanded", S.bool),
          changedBelow: s.field("changed_below", S.int),
          children: s.field("children", S.array(self)),
        })
      }),
      S.object(s => {
        s.tag("type", "File")
        File({
          name: s.field("name", S.string),
          repoId: s.field("repo_id", repoId),
          path: s.field("path", S.string),
          change: s.field("change", S.null(ChangeKindKind.schema)),
          viewed: s.field("viewed", ViewedState.schema),
          open_: s.field("open", S.bool),
        })
      }),
    ])
  )
}

type searchHit = {file: fileRef, matched: array<int>, change: option<ChangeKindKind.t>}
let searchHit: S.t<searchHit> = S.object(s => {
  file: s.field("file", fileRef),
  matched: s.field("matched", S.array(S.int)),
  change: s.field("change", S.null(ChangeKindKind.schema)),
})

type searchView = {query: string, hits: array<searchHit>}
let searchView: S.t<searchView> = S.object(s => {
  query: s.field("query", S.string),
  hits: s.field("hits", S.array(searchHit)),
})

type treeView = {roots: array<TreeNode.t>, breadcrumbs: array<string>, search: option<searchView>}
let treeView: S.t<treeView> = S.object(s => {
  roots: s.field("roots", S.array(TreeNode.schema)),
  breadcrumbs: s.field("breadcrumbs", S.array(S.string)),
  search: s.field("search", S.null(searchView)),
})

type progress = {viewed: int, changedSinceViewed: int, total: int}
let progress: S.t<progress> = S.object(s => {
  viewed: s.field("viewed", S.int),
  changedSinceViewed: s.field("changed_since_viewed", S.int),
  total: s.field("total", S.int),
})

type diffRow = {index: int, row: Render.row, threads: array<threadId>}
let diffRow: S.t<diffRow> = S.object(s => {
  index: s.field("index", S.int),
  row: s.field("row", Render.row),
  threads: s.field("threads", S.array(threadId)),
})

type diffView = {
  file: fileRef,
  lang: option<string>,
  content: Render.renderContent,
  firstRow: int,
  lastRow: int,
  rows: array<diffRow>,
  missing: array<Render.chunkIndex>,
  fileThreads: array<threadId>,
}
let diffView: S.t<diffView> = S.object(s => {
  file: s.field("file", fileRef),
  lang: s.field("lang", S.null(S.string)),
  content: s.field("content", Render.renderContent),
  firstRow: s.field("first_row", S.int),
  lastRow: s.field("last_row", S.int),
  rows: s.field("rows", S.array(diffRow)),
  missing: s.field("missing", S.array(Render.chunkIndex)),
  fileThreads: s.field("file_threads", S.array(threadId)),
})

module ThreadPlace = {
  type t =
    | Review
    | File({file: fileRef})
    | Lines({file: fileRef, side: Domain.side, start: int, end: int})
  let schema: S.t<t> = S.union([
    S.object(s => {
      s.tag("type", "Review")
      Review
    }),
    S.object(s => {
      s.tag("type", "File")
      File({file: s.field("file", fileRef)})
    }),
    S.object(s => {
      s.tag("type", "Lines")
      Lines({
        file: s.field("file", fileRef),
        side: s.field("side", Domain.side),
        start: s.field("start", S.int),
        end: s.field("end", S.int),
      })
    }),
  ])
}

type threadView = {
  id: threadId,
  root: commentId,
  author: Domain.author,
  created: timestamp,
  summary: string,
  replies: int,
  resolved: bool,
  place: ThreadPlace.t,
  outdated: bool,
  pending: bool,
}
let threadView: S.t<threadView> = S.object(s => {
  id: s.field("id", threadId),
  root: s.field("root", commentId),
  author: s.field("author", Domain.author),
  created: s.field("created", timestamp),
  summary: s.field("summary", S.string),
  replies: s.field("replies", S.int),
  resolved: s.field("resolved", S.bool),
  place: s.field("place", ThreadPlace.schema),
  outdated: s.field("outdated", S.bool),
  pending: s.field("pending", S.bool),
})

type stepperCommit = {oid: commitOid, subject: string, author: string, time: timestamp}
let stepperCommit: S.t<stepperCommit> = S.object(s => {
  oid: s.field("oid", commitOid),
  subject: s.field("subject", S.string),
  author: s.field("author", S.string),
  time: s.field("time", timestamp),
})

type commitStepper = {repoId: repoId, commits: array<stepperCommit>, selected: option<int>}
let commitStepper: S.t<commitStepper> = S.object(s => {
  repoId: s.field("repo_id", repoId),
  commits: s.field("commits", S.array(stepperCommit)),
  selected: s.field("selected", S.null(S.int)),
})

module Focus = {
  type t =
    | ReviewList({index: int})
    | Tree({index: int})
    | Diff({row: int})
    | Thread({index: int})
    | Composer
    | CommitStepper({index: int})
    | Help
  let schema: S.t<t> = S.union([
    S.object(s => {
      s.tag("type", "ReviewList")
      ReviewList({index: s.field("index", S.int)})
    }),
    S.object(s => {
      s.tag("type", "Tree")
      Tree({index: s.field("index", S.int)})
    }),
    S.object(s => {
      s.tag("type", "Diff")
      Diff({row: s.field("row", S.int)})
    }),
    S.object(s => {
      s.tag("type", "Thread")
      Thread({index: s.field("index", S.int)})
    }),
    S.object(s => {
      s.tag("type", "Composer")
      Composer
    }),
    S.object(s => {
      s.tag("type", "CommitStepper")
      CommitStepper({index: s.field("index", S.int)})
    }),
    S.object(s => {
      s.tag("type", "Help")
      Help
    }),
  ])
}

module Context = {
  type t = Global | ReviewList | Tree | Diff | Thread | Composer | CommitStepper | Help
  let schema: S.t<t> = S.enum([Global, ReviewList, Tree, Diff, Thread, Composer, CommitStepper, Help])
}

module Command = {
  type t =
    | MoveDown
    | MoveUp
    | PageDown
    | PageUp
    | GoTop
    | GoBottom
    | NextHunk
    | PrevHunk
    | NextFile
    | PrevFile
    | NextComment
    | PrevComment
    | Open
    | Back
    | NextPanel
    | ToggleViewed
    | Comment
    | Reply
    | Delete
    | ToggleResolved
    | FileSearch
    | ToggleLayout
    | ToggleWhitespace
    | ToggleHelp
    | Connect
    | Disconnect
    | Commits
  let all = [
    MoveDown,
    MoveUp,
    PageDown,
    PageUp,
    GoTop,
    GoBottom,
    NextHunk,
    PrevHunk,
    NextFile,
    PrevFile,
    NextComment,
    PrevComment,
    Open,
    Back,
    NextPanel,
    ToggleViewed,
    Comment,
    Reply,
    Delete,
    ToggleResolved,
    FileSearch,
    ToggleLayout,
    ToggleWhitespace,
    ToggleHelp,
    Connect,
    Disconnect,
    Commits,
  ]
  let schema: S.t<t> = S.enum(all)
}

/// A key sequence in its text form (`"g g"`, `"ctrl+p"`).
type keySeq = string
let keySeq: S.t<keySeq> = S.string

type hint = {keys: keySeq, command: Command.t, label: string}
let hint: S.t<hint> = S.object(s => {
  keys: s.field("keys", keySeq),
  command: s.field("command", Command.schema),
  label: s.field("label", S.string),
})

type helpEntry = {keys: keySeq, command: Command.t, label: string, primary: bool, overridden: bool}
let helpEntry: S.t<helpEntry> = S.object(s => {
  keys: s.field("keys", keySeq),
  command: s.field("command", Command.schema),
  label: s.field("label", S.string),
  primary: s.field("primary", S.bool),
  overridden: s.field("overridden", S.bool),
})

type helpGroup = {context: Context.t, entries: array<helpEntry>}
let helpGroup: S.t<helpGroup> = S.object(s => {
  context: s.field("context", Context.schema),
  entries: s.field("entries", S.array(helpEntry)),
})

type conflict = {context: Context.t, keys: keySeq, commands: array<Command.t>}
let conflict: S.t<conflict> = S.object(s => {
  context: s.field("context", Context.schema),
  keys: s.field("keys", keySeq),
  commands: s.field("commands", S.array(Command.schema)),
})

type helpView = {groups: array<helpGroup>, conflicts: array<conflict>}
let helpView: S.t<helpView> = S.object(s => {
  groups: s.field("groups", S.array(helpGroup)),
  conflicts: s.field("conflicts", S.array(conflict)),
})

type override = {context: Context.t, command: Command.t, keys: option<keySeq>, primary: bool}
let override: S.t<override> = S.object(s => {
  context: s.field("context", Context.schema),
  command: s.field("command", Command.schema),
  keys: s.field("keys", S.null(keySeq)),
  primary: s.field("primary", S.bool),
})

type overrides = {bindings: array<override>}
let overrides: S.t<overrides> = S.object(s => {
  bindings: s.field("bindings", S.array(override)),
})

type viewDelta = {sections: array<Rpc.viewSection>}
let viewDelta: S.t<viewDelta> = S.object(s => {
  sections: s.field("sections", S.array(Rpc.viewSection)),
})

type viewModel = {
  prefs: viewPrefs,
  tree: treeView,
  progress: progress,
  diff: option<diffView>,
  threads: array<threadView>,
  conversation: array<threadView>,
  stepper: option<commitStepper>,
  focus: Focus.t,
  hints: array<hint>,
  help: option<helpView>,
  connection: ConnectionView.t,
  lastError: option<Rpc.rpcError>,
  reviews: array<Domain.review>,
  review: option<openReview>,
  draft: option<draft>,
  pendingRefresh: bool,
}
let viewModel: S.t<viewModel> = S.object(s => {
  prefs: s.field("prefs", viewPrefs),
  tree: s.field("tree", treeView),
  progress: s.field("progress", progress),
  diff: s.field("diff", S.null(diffView)),
  threads: s.field("threads", S.array(threadView)),
  conversation: s.field("conversation", S.array(threadView)),
  stepper: s.field("stepper", S.null(commitStepper)),
  focus: s.field("focus", Focus.schema),
  hints: s.field("hints", S.array(hint)),
  help: s.field("help", S.null(helpView)),
  connection: s.field("connection", ConnectionView.schema),
  lastError: s.field("last_error", S.null(Rpc.rpcError)),
  reviews: s.field("reviews", S.array(Domain.review)),
  review: s.field("review", S.null(openReview)),
  draft: s.field("draft", S.null(draft)),
  pendingRefresh: s.field("pending_refresh", S.bool),
})
