// `moor_client_core::Action`: what the UI dispatches to the host.

open Ids

type t =
  | Connect
  | Disconnect
  | ListReviews({workspaceId: workspaceId})
  | OpenReview({reviewId: reviewId})
  | CloseReview
  | DraftOpened({anchor: Domain.anchor})
  | DraftSubmitted({body: string})
  | DraftDiscarded
  | ReplyOpened({threadId: threadId})
  | SetFocus({focus: View.Focus.t})
  | ToggleHelp
  | Reply({threadId: threadId, body: string})
  | EditComment({commentId: commentId, body: string})
  | DeleteComment({commentId: commentId})
  | ResolveThread({threadId: threadId})
  | UnresolveThread({threadId: threadId})
  | Viewport({file: View.fileRef, firstRow: int, lastRow: int})
  | CloseFile
  | ToggleDir({repoId: repoId, path: option<string>})
  | FileSearch({query: option<string>})
  | SetLayout({layout: View.Layout.t})
  | SetRenderOpts({ignoreWhitespace: bool, contextLines: int})
  | MarkViewed({file: View.fileRef})
  | UnmarkViewed({file: View.fileRef})
  | ListCommits({repoId: repoId})
  | StepCommit({selected: option<int>})

let schema: S.t<t> = S.union([
  S.object(s => {
    s.tag("type", "Connect")
    Connect
  }),
  S.object(s => {
    s.tag("type", "Disconnect")
    Disconnect
  }),
  S.object(s => {
    s.tag("type", "ListReviews")
    ListReviews({workspaceId: s.field("workspace_id", workspaceId)})
  }),
  S.object(s => {
    s.tag("type", "OpenReview")
    OpenReview({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "CloseReview")
    CloseReview
  }),
  S.object(s => {
    s.tag("type", "DraftOpened")
    DraftOpened({anchor: s.field("anchor", Domain.anchor)})
  }),
  S.object(s => {
    s.tag("type", "DraftSubmitted")
    DraftSubmitted({body: s.field("body", S.string)})
  }),
  S.object(s => {
    s.tag("type", "DraftDiscarded")
    DraftDiscarded
  }),
  S.object(s => {
    s.tag("type", "ReplyOpened")
    ReplyOpened({threadId: s.field("thread_id", threadId)})
  }),
  S.object(s => {
    s.tag("type", "SetFocus")
    SetFocus({focus: s.field("focus", View.Focus.schema)})
  }),
  S.object(s => {
    s.tag("type", "ToggleHelp")
    ToggleHelp
  }),
  S.object(s => {
    s.tag("type", "Reply")
    Reply({threadId: s.field("thread_id", threadId), body: s.field("body", S.string)})
  }),
  S.object(s => {
    s.tag("type", "EditComment")
    EditComment({commentId: s.field("comment_id", commentId), body: s.field("body", S.string)})
  }),
  S.object(s => {
    s.tag("type", "DeleteComment")
    DeleteComment({commentId: s.field("comment_id", commentId)})
  }),
  S.object(s => {
    s.tag("type", "ResolveThread")
    ResolveThread({threadId: s.field("thread_id", threadId)})
  }),
  S.object(s => {
    s.tag("type", "UnresolveThread")
    UnresolveThread({threadId: s.field("thread_id", threadId)})
  }),
  S.object(s => {
    s.tag("type", "Viewport")
    Viewport({
      file: s.field("file", View.fileRef),
      firstRow: s.field("first_row", S.int),
      lastRow: s.field("last_row", S.int),
    })
  }),
  S.object(s => {
    s.tag("type", "CloseFile")
    CloseFile
  }),
  S.object(s => {
    s.tag("type", "ToggleDir")
    ToggleDir({repoId: s.field("repo_id", repoId), path: s.field("path", S.null(S.string))})
  }),
  S.object(s => {
    s.tag("type", "FileSearch")
    FileSearch({query: s.field("query", S.null(S.string))})
  }),
  S.object(s => {
    s.tag("type", "SetLayout")
    SetLayout({layout: s.field("layout", View.Layout.schema)})
  }),
  S.object(s => {
    s.tag("type", "SetRenderOpts")
    SetRenderOpts({
      ignoreWhitespace: s.field("ignore_whitespace", S.bool),
      contextLines: s.field("context_lines", S.int),
    })
  }),
  S.object(s => {
    s.tag("type", "MarkViewed")
    MarkViewed({file: s.field("file", View.fileRef)})
  }),
  S.object(s => {
    s.tag("type", "UnmarkViewed")
    UnmarkViewed({file: s.field("file", View.fileRef)})
  }),
  S.object(s => {
    s.tag("type", "ListCommits")
    ListCommits({repoId: s.field("repo_id", repoId)})
  }),
  S.object(s => {
    s.tag("type", "StepCommit")
    StepCommit({selected: s.field("selected", S.null(S.int))})
  }),
])
