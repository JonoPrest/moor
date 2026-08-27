// Events (moor-protocol `events.rs`).

open Ids
open Domain

type eventBody =
  | WorkspaceCreated({workspace: workspace})
  | WorkspaceUpdated({workspaceId: workspaceId, name: string})
  | RepoAttached({workspaceId: workspaceId, repo: repo})
  | RepoDetached({workspaceId: workspaceId, repoId: repoId})
  | ReviewCreated({review: review})
  | ReviewUpdated({reviewId: reviewId, title: string, status: reviewStatus})
  | ReviewDeleted({reviewId: reviewId})
  | ReviewTargetsResolved({reviewId: reviewId, targets: array<resolvedTarget>})
  | CommentCreated({comment: comment})
  | CommentEdited({reviewId: reviewId, commentId: commentId, body: string})
  | CommentDeleted({reviewId: reviewId, commentId: commentId})
  | CommentReanchored({reviewId: reviewId, commentId: commentId, anchor: anchor, state: commentState})
  | ThreadResolved({reviewId: reviewId, threadId: threadId})
  | ThreadUnresolved({reviewId: reviewId, threadId: threadId})
  | FileViewed({reviewId: reviewId, repoId: repoId, path: string, viewer: human, blobOid: option<blobOid>})
  | FileUnviewed({reviewId: reviewId, repoId: repoId, path: string, viewer: human})
  | ReviewRequested({reviewId: reviewId, agent: string, note: string})
  | SuggestionApplied({
      reviewId: reviewId,
      commentId: commentId,
      repoId: repoId,
      path: string,
      resultBlob: blobOid,
    })

let eventBody: S.t<eventBody> = S.union([
  S.object(s => {
    s.tag("type", "WorkspaceCreated")
    WorkspaceCreated({workspace: s.field("workspace", workspace)})
  }),
  S.object(s => {
    s.tag("type", "WorkspaceUpdated")
    WorkspaceUpdated({
      workspaceId: s.field("workspace_id", workspaceId),
      name: s.field("name", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "RepoAttached")
    RepoAttached({workspaceId: s.field("workspace_id", workspaceId), repo: s.field("repo", repo)})
  }),
  S.object(s => {
    s.tag("type", "RepoDetached")
    RepoDetached({
      workspaceId: s.field("workspace_id", workspaceId),
      repoId: s.field("repo_id", repoId),
    })
  }),
  S.object(s => {
    s.tag("type", "ReviewCreated")
    ReviewCreated({review: s.field("review", review)})
  }),
  S.object(s => {
    s.tag("type", "ReviewUpdated")
    ReviewUpdated({
      reviewId: s.field("review_id", reviewId),
      title: s.field("title", S.string),
      status: s.field("status", reviewStatus),
    })
  }),
  S.object(s => {
    s.tag("type", "ReviewDeleted")
    ReviewDeleted({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "ReviewTargetsResolved")
    ReviewTargetsResolved({
      reviewId: s.field("review_id", reviewId),
      targets: s.field("targets", S.array(resolvedTarget)),
    })
  }),
  S.object(s => {
    s.tag("type", "CommentCreated")
    CommentCreated({comment: s.field("comment", comment)})
  }),
  S.object(s => {
    s.tag("type", "CommentEdited")
    CommentEdited({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
      body: s.field("body", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "CommentDeleted")
    CommentDeleted({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
    })
  }),
  S.object(s => {
    s.tag("type", "CommentReanchored")
    CommentReanchored({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
      anchor: s.field("anchor", anchor),
      state: s.field("state", commentState),
    })
  }),
  S.object(s => {
    s.tag("type", "ThreadResolved")
    ThreadResolved({
      reviewId: s.field("review_id", reviewId),
      threadId: s.field("thread_id", threadId),
    })
  }),
  S.object(s => {
    s.tag("type", "ThreadUnresolved")
    ThreadUnresolved({
      reviewId: s.field("review_id", reviewId),
      threadId: s.field("thread_id", threadId),
    })
  }),
  S.object(s => {
    s.tag("type", "FileViewed")
    FileViewed({
      reviewId: s.field("review_id", reviewId),
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      viewer: s.field("viewer", human),
      blobOid: s.field("blob_oid", S.null(blobOid)),
    })
  }),
  S.object(s => {
    s.tag("type", "FileUnviewed")
    FileUnviewed({
      reviewId: s.field("review_id", reviewId),
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      viewer: s.field("viewer", human),
    })
  }),
  S.object(s => {
    s.tag("type", "ReviewRequested")
    ReviewRequested({
      reviewId: s.field("review_id", reviewId),
      agent: s.field("agent", S.string),
      note: s.field("note", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "SuggestionApplied")
    SuggestionApplied({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      resultBlob: s.field("result_blob", blobOid),
    })
  }),
])

type event = {
  seq: seq,
  ts: timestamp,
  author: author,
  clientId: clientId,
  clientSeq: clientSeq,
  body: eventBody,
}
let event: S.t<event> = S.object(s => {
  seq: s.field("seq", seq),
  ts: s.field("ts", timestamp),
  author: s.field("author", author),
  clientId: s.field("client_id", clientId),
  clientSeq: s.field("client_seq", clientSeq),
  body: s.field("body", eventBody),
})
