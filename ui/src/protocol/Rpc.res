// RPC frames (moor-protocol `rpc.rs`).

open Ids
open Domain

type buildInfo = {name: string, version: string}
let buildInfo: S.t<buildInfo> = S.object(s => {
  name: s.field("name", S.string),
  version: s.field("version", S.string),
})

type upgradeNotice = {latest: protocolVersion, message: string}
let upgradeNotice: S.t<upgradeNotice> = S.object(s => {
  latest: s.field("latest", protocolVersion),
  message: s.field("message", S.string),
})

type entityKind = Workspace | Repo | Review | Comment | Thread | Ref | Path | Blob | Chunk
let entityKind: S.t<entityKind> = S.enum([
  Workspace,
  Repo,
  Review,
  Comment,
  Thread,
  Ref,
  Path,
  Blob,
  Chunk,
])

type rpcError =
  | NotFound({kind: entityKind, id: string})
  | Invalid({reason: string})
  | Forbidden({reason: string})
  | SeqTooOld({oldest: seq})
  | Cancelled
  | UnsupportedProtocol({requested: protocolVersion, supported: array<protocolVersion>})
  | VersionMismatch({negotiated: protocolVersion, received: protocolVersion})
  | Internal({message: string})
let rpcError: S.t<rpcError> = S.union([
  S.object(s => {
    s.tag("type", "NotFound")
    NotFound({kind: s.field("kind", entityKind), id: s.field("id", S.string)})
  }),
  S.object(s => {
    s.tag("type", "Invalid")
    Invalid({reason: s.field("reason", S.string)})
  }),
  S.object(s => {
    s.tag("type", "Forbidden")
    Forbidden({reason: s.field("reason", S.string)})
  }),
  S.object(s => {
    s.tag("type", "SeqTooOld")
    SeqTooOld({oldest: s.field("oldest", seq)})
  }),
  S.object(s => {
    s.tag("type", "Cancelled")
    Cancelled
  }),
  S.object(s => {
    s.tag("type", "UnsupportedProtocol")
    UnsupportedProtocol({
      requested: s.field("requested", protocolVersion),
      supported: s.field("supported", S.array(protocolVersion)),
    })
  }),
  S.object(s => {
    s.tag("type", "VersionMismatch")
    VersionMismatch({
      negotiated: s.field("negotiated", protocolVersion),
      received: s.field("received", protocolVersion),
    })
  }),
  S.object(s => {
    s.tag("type", "Internal")
    Internal({message: s.field("message", S.string)})
  }),
])

type since = Now | After({seq: seq})
let since: S.t<since> = S.union([
  S.object(s => {
    s.tag("type", "Now")
    Now
  }),
  S.object(s => {
    s.tag("type", "After")
    After({seq: s.field("seq", seq)})
  }),
])

type subscribeScope =
  | All
  | ScopeWorkspace({workspaceId: workspaceId})
  | ScopeReview({reviewId: reviewId})
  | AwaitingAgent({agent: string})
let subscribeScope: S.t<subscribeScope> = S.union([
  S.object(s => {
    s.tag("type", "All")
    All
  }),
  S.object(s => {
    s.tag("type", "Workspace")
    ScopeWorkspace({workspaceId: s.field("workspace_id", workspaceId)})
  }),
  S.object(s => {
    s.tag("type", "Review")
    ScopeReview({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "AwaitingAgent")
    AwaitingAgent({agent: s.field("agent", S.string)})
  }),
])

type viewSection =
  | Connection
  | ReviewList
  | Tree
  | Diff
  | Threads
  | Conversation
  | CommitStepper
  | Progress
  | Focus
  | Hints
  | Help
  | Draft
let viewSection: S.t<viewSection> = S.enum([
  Connection,
  ReviewList,
  Tree,
  Diff,
  Threads,
  Conversation,
  CommitStepper,
  Progress,
  Focus,
  Hints,
  Help,
  Draft,
])

type mutation =
  | CreateWorkspace({workspaceId: workspaceId, name: string})
  | RenameWorkspace({workspaceId: workspaceId, name: string})
  | AttachRepo({workspaceId: workspaceId, repoId: repoId, path: string, displayName: string})
  | DetachRepo({workspaceId: workspaceId, repoId: repoId})
  | CreateReview({
      reviewId: reviewId,
      workspaceId: workspaceId,
      title: string,
      targets: array<reviewTarget>,
    })
  | UpdateReview({reviewId: reviewId, title: string, status: reviewStatus})
  | DeleteReview({reviewId: reviewId})
  | AddComment({
      reviewId: reviewId,
      commentId: commentId,
      kind: commentKind,
      anchor: anchor,
      body: string,
    })
  | Reply({
      reviewId: reviewId,
      threadId: threadId,
      commentId: commentId,
      kind: commentKind,
      body: string,
    })
  | EditComment({reviewId: reviewId, commentId: commentId, body: string})
  | DeleteComment({reviewId: reviewId, commentId: commentId})
  | ResolveThread({reviewId: reviewId, threadId: threadId})
  | UnresolveThread({reviewId: reviewId, threadId: threadId})
  | MarkViewed({reviewId: reviewId, repoId: repoId, path: string})
  | UnmarkViewed({reviewId: reviewId, repoId: repoId, path: string})
  | RequestReview({reviewId: reviewId, agent: string, note: string})
  | ApplySuggestion({reviewId: reviewId, commentId: commentId})
let mutation: S.t<mutation> = S.union([
  S.object(s => {
    s.tag("type", "CreateWorkspace")
    CreateWorkspace({
      workspaceId: s.field("workspace_id", workspaceId),
      name: s.field("name", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "RenameWorkspace")
    RenameWorkspace({
      workspaceId: s.field("workspace_id", workspaceId),
      name: s.field("name", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "AttachRepo")
    AttachRepo({
      workspaceId: s.field("workspace_id", workspaceId),
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      displayName: s.field("display_name", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "DetachRepo")
    DetachRepo({
      workspaceId: s.field("workspace_id", workspaceId),
      repoId: s.field("repo_id", repoId),
    })
  }),
  S.object(s => {
    s.tag("type", "CreateReview")
    CreateReview({
      reviewId: s.field("review_id", reviewId),
      workspaceId: s.field("workspace_id", workspaceId),
      title: s.field("title", S.string),
      targets: s.field("targets", S.array(reviewTarget)),
    })
  }),
  S.object(s => {
    s.tag("type", "UpdateReview")
    UpdateReview({
      reviewId: s.field("review_id", reviewId),
      title: s.field("title", S.string),
      status: s.field("status", reviewStatus),
    })
  }),
  S.object(s => {
    s.tag("type", "DeleteReview")
    DeleteReview({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "AddComment")
    AddComment({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
      kind: s.field("kind", commentKind),
      anchor: s.field("anchor", anchor),
      body: s.field("body", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "Reply")
    Reply({
      reviewId: s.field("review_id", reviewId),
      threadId: s.field("thread_id", threadId),
      commentId: s.field("comment_id", commentId),
      kind: s.field("kind", commentKind),
      body: s.field("body", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "EditComment")
    EditComment({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
      body: s.field("body", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "DeleteComment")
    DeleteComment({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
    })
  }),
  S.object(s => {
    s.tag("type", "ResolveThread")
    ResolveThread({
      reviewId: s.field("review_id", reviewId),
      threadId: s.field("thread_id", threadId),
    })
  }),
  S.object(s => {
    s.tag("type", "UnresolveThread")
    UnresolveThread({
      reviewId: s.field("review_id", reviewId),
      threadId: s.field("thread_id", threadId),
    })
  }),
  S.object(s => {
    s.tag("type", "MarkViewed")
    MarkViewed({
      reviewId: s.field("review_id", reviewId),
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "UnmarkViewed")
    UnmarkViewed({
      reviewId: s.field("review_id", reviewId),
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "RequestReview")
    RequestReview({
      reviewId: s.field("review_id", reviewId),
      agent: s.field("agent", S.string),
      note: s.field("note", S.string),
    })
  }),
  S.object(s => {
    s.tag("type", "ApplySuggestion")
    ApplySuggestion({
      reviewId: s.field("review_id", reviewId),
      commentId: s.field("comment_id", commentId),
    })
  }),
])

type request =
  | ListWorkspaces
  | ListReviews({workspaceId: workspaceId})
  | GetReview({reviewId: reviewId})
  | ReviewSnapshot({reviewId: reviewId})
  | ListFiles({reviewId: reviewId})
  | OpenReview({reviewId: reviewId, opts: renderOpts})
  | ResolveTargets({reviewId: reviewId})
  | ListCommits({reviewId: reviewId, repoId: repoId})
  | TreeSnapshot({repoId: repoId, ref: refSpec})
  | FileRender({
      reviewId: reviewId,
      repoId: repoId,
      path: string,
      opts: renderOpts,
      firstChunk: Render.chunkIndex,
    })
  | BlobRender({repoId: repoId, path: string, blobOid: blobOid, firstChunk: Render.chunkIndex})
  | RenderChunk({
      repoId: repoId,
      path: string,
      target: Render.renderTarget,
      opts: renderOpts,
      index: Render.chunkIndex,
    })
  | Subscribe({scope: subscribeScope, since: since})
  | Unsubscribe({scope: subscribeScope})
  | Mutate({clientSeq: clientSeq, mutation: mutation})
  | Shutdown
let request: S.t<request> = S.union([
  S.object(s => {
    s.tag("type", "ListWorkspaces")
    ListWorkspaces
  }),
  S.object(s => {
    s.tag("type", "ListReviews")
    ListReviews({workspaceId: s.field("workspace_id", workspaceId)})
  }),
  S.object(s => {
    s.tag("type", "GetReview")
    GetReview({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "ReviewSnapshot")
    ReviewSnapshot({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "ListFiles")
    ListFiles({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "OpenReview")
    OpenReview({reviewId: s.field("review_id", reviewId), opts: s.field("opts", renderOpts)})
  }),
  S.object(s => {
    s.tag("type", "ResolveTargets")
    ResolveTargets({reviewId: s.field("review_id", reviewId)})
  }),
  S.object(s => {
    s.tag("type", "ListCommits")
    ListCommits({reviewId: s.field("review_id", reviewId), repoId: s.field("repo_id", repoId)})
  }),
  S.object(s => {
    s.tag("type", "TreeSnapshot")
    TreeSnapshot({repoId: s.field("repo_id", repoId), ref: s.field("ref", refSpec)})
  }),
  S.object(s => {
    s.tag("type", "FileRender")
    FileRender({
      reviewId: s.field("review_id", reviewId),
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      opts: s.field("opts", renderOpts),
      firstChunk: s.field("first_chunk", Render.chunkIndex),
    })
  }),
  S.object(s => {
    s.tag("type", "BlobRender")
    BlobRender({
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      blobOid: s.field("blob_oid", blobOid),
      firstChunk: s.field("first_chunk", Render.chunkIndex),
    })
  }),
  S.object(s => {
    s.tag("type", "RenderChunk")
    RenderChunk({
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      target: s.field("target", Render.renderTarget),
      opts: s.field("opts", renderOpts),
      index: s.field("index", Render.chunkIndex),
    })
  }),
  S.object(s => {
    s.tag("type", "Subscribe")
    Subscribe({scope: s.field("scope", subscribeScope), since: s.field("since", since)})
  }),
  S.object(s => {
    s.tag("type", "Unsubscribe")
    Unsubscribe({scope: s.field("scope", subscribeScope)})
  }),
  S.object(s => {
    s.tag("type", "Mutate")
    Mutate({clientSeq: s.field("client_seq", clientSeq), mutation: s.field("mutation", mutation)})
  }),
  S.object(s => {
    s.tag("type", "Shutdown")
    Shutdown
  }),
])

type response =
  | Workspaces({workspaces: array<workspace>})
  | Reviews({reviews: array<review>})
  | Review({review: review})
  | ReviewSnapshot({snapshot: reviewSnapshot})
  | Files({files: array<fileChange>})
  | Resolved({targets: array<resolvedTarget>, changed: bool})
  | Commits({commits: array<commitInfo>})
  | TreeSnapshot({snapshot: treeSnapshot})
  | RenderChunk({chunk: Render.renderChunk})
  | Subscribed({seq: seq})
  | Unsubscribed
  | Committed({event: Events.event})
  | ShuttingDown
let response: S.t<response> = S.union([
  S.object(s => {
    s.tag("type", "Workspaces")
    Workspaces({workspaces: s.field("workspaces", S.array(workspace))})
  }),
  S.object(s => {
    s.tag("type", "Reviews")
    Reviews({reviews: s.field("reviews", S.array(review))})
  }),
  S.object(s => {
    s.tag("type", "Review")
    Review({review: s.field("review", review)})
  }),
  S.object(s => {
    s.tag("type", "ReviewSnapshot")
    ReviewSnapshot({snapshot: s.field("snapshot", reviewSnapshot)})
  }),
  S.object(s => {
    s.tag("type", "Files")
    Files({files: s.field("files", S.array(fileChange))})
  }),
  S.object(s => {
    s.tag("type", "Resolved")
    Resolved({
      targets: s.field("targets", S.array(resolvedTarget)),
      changed: s.field("changed", S.bool),
    })
  }),
  S.object(s => {
    s.tag("type", "Commits")
    Commits({commits: s.field("commits", S.array(commitInfo))})
  }),
  S.object(s => {
    s.tag("type", "TreeSnapshot")
    TreeSnapshot({snapshot: s.field("snapshot", treeSnapshot)})
  }),
  S.object(s => {
    s.tag("type", "RenderChunk")
    RenderChunk({chunk: s.field("chunk", Render.renderChunk)})
  }),
  S.object(s => {
    s.tag("type", "Subscribed")
    Subscribed({seq: s.field("seq", seq)})
  }),
  S.object(s => {
    s.tag("type", "Unsubscribed")
    Unsubscribed
  }),
  S.object(s => {
    s.tag("type", "Committed")
    Committed({event: s.field("event", Events.event)})
  }),
  S.object(s => {
    s.tag("type", "ShuttingDown")
    ShuttingDown
  }),
])

type streamItem =
  | ItemReviewSnapshot({snapshot: reviewSnapshot})
  | ItemTreeSnapshot({snapshot: treeSnapshot})
  | ItemHeader({header: Render.fileRenderHeader})
  | ItemChunk({repoId: repoId, path: string, chunk: Render.renderChunk})
let streamItem: S.t<streamItem> = S.union([
  S.object(s => {
    s.tag("type", "ReviewSnapshot")
    ItemReviewSnapshot({snapshot: s.field("snapshot", reviewSnapshot)})
  }),
  S.object(s => {
    s.tag("type", "TreeSnapshot")
    ItemTreeSnapshot({snapshot: s.field("snapshot", treeSnapshot)})
  }),
  S.object(s => {
    s.tag("type", "Header")
    ItemHeader({header: s.field("header", Render.fileRenderHeader)})
  }),
  S.object(s => {
    s.tag("type", "Chunk")
    ItemChunk({
      repoId: s.field("repo_id", repoId),
      path: s.field("path", S.string),
      chunk: s.field("chunk", Render.renderChunk),
    })
  }),
])

type clientMsg =
  | Hello({clientId: clientId, protocol: protocolVersion, client: buildInfo, author: author})
  | ClientRequest({id: requestId, request: request})
  | Cancel({id: requestId})
let clientMsg: S.t<clientMsg> = S.union([
  S.object(s => {
    s.tag("type", "Hello")
    Hello({
      clientId: s.field("client_id", clientId),
      protocol: s.field("protocol", protocolVersion),
      client: s.field("client", buildInfo),
      author: s.field("author", author),
    })
  }),
  S.object(s => {
    s.tag("type", "Request")
    ClientRequest({id: s.field("id", requestId), request: s.field("request", request)})
  }),
  S.object(s => {
    s.tag("type", "Cancel")
    Cancel({id: s.field("id", requestId)})
  }),
])

type serverMsg =
  | Welcome({
      protocol: protocolVersion,
      daemon: buildInfo,
      schema: schemaVersion,
      upgrade: option<upgradeNotice>,
    })
  | Rejected({error: rpcError})
  | ServerResponse({id: requestId, response: response})
  | StreamItem({id: requestId, item: streamItem})
  | StreamEnd({id: requestId})
  | Error({id: requestId, error: rpcError})
  | Event({event: Events.event})
  | TreeDelta({delta: treeDelta})
let serverMsg: S.t<serverMsg> = S.union([
  S.object(s => {
    s.tag("type", "Welcome")
    Welcome({
      protocol: s.field("protocol", protocolVersion),
      daemon: s.field("daemon", buildInfo),
      schema: s.field("schema", schemaVersion),
      upgrade: s.field("upgrade", S.null(upgradeNotice)),
    })
  }),
  S.object(s => {
    s.tag("type", "Rejected")
    Rejected({error: s.field("error", rpcError)})
  }),
  S.object(s => {
    s.tag("type", "Response")
    ServerResponse({id: s.field("id", requestId), response: s.field("response", response)})
  }),
  S.object(s => {
    s.tag("type", "StreamItem")
    StreamItem({id: s.field("id", requestId), item: s.field("item", streamItem)})
  }),
  S.object(s => {
    s.tag("type", "StreamEnd")
    StreamEnd({id: s.field("id", requestId)})
  }),
  S.object(s => {
    s.tag("type", "Error")
    Error({id: s.field("id", requestId), error: s.field("error", rpcError)})
  }),
  S.object(s => {
    s.tag("type", "Event")
    Event({event: s.field("event", Events.event)})
  }),
  S.object(s => {
    s.tag("type", "TreeDelta")
    TreeDelta({delta: s.field("delta", treeDelta)})
  }),
])

/// A frame on the wire: `{ "v": protocol version, "msg": ... }`.
type envelope<'msg> = {v: protocolVersion, msg: 'msg}
let envelope = (msg: S.t<'msg>): S.t<envelope<'msg>> =>
  S.object(s => {
    v: s.field("v", protocolVersion),
    msg: s.field("msg", msg),
  })
