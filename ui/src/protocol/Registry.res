// Every schema by its Rust type name, for the boundary test: a fixture
// directory under `fixtures/protocol/<Type>/` must have an entry here, and
// each fixture must parse and re-serialise to the same JSON.

external erase: S.t<'a> => S.t<unknown> = "%identity"

let schemas: dict<S.t<unknown>> = Dict.fromArray([
  // ids / version
  ("ProtocolVersion", erase(Ids.protocolVersion)),
  ("SchemaVersion", erase(Ids.schemaVersion)),
  // domain
  ("Repo", erase(Domain.repo)),
  ("Workspace", erase(Domain.workspace)),
  ("Human", erase(Domain.human)),
  ("AgentVia", erase(Domain.agentVia)),
  ("Author", erase(Domain.author)),
  ("RefSpec", erase(Domain.refSpec)),
  ("ResolvedSource", erase(Domain.resolvedSource)),
  ("ResolvedRef", erase(Domain.resolvedRef)),
  ("ReviewTarget", erase(Domain.reviewTarget)),
  ("ResolvedTarget", erase(Domain.resolvedTarget)),
  ("ReviewStatus", erase(Domain.reviewStatus)),
  ("Review", erase(Domain.review)),
  ("Sig", erase(Domain.sig)),
  ("CommitInfo", erase(Domain.commitInfo)),
  ("Side", erase(Domain.side)),
  ("Anchor", erase(Domain.anchor)),
  ("CommentKind", erase(Domain.commentKind)),
  ("CommentState", erase(Domain.commentState)),
  ("Comment", erase(Domain.comment)),
  ("ThreadResolution", erase(Domain.threadResolution)),
  ("Thread", erase(Domain.thread)),
  ("ViewedMark", erase(Domain.viewedMark)),
  ("RenderOpts", erase(Domain.renderOpts)),
  ("ChangeKind", erase(Domain.changeKind)),
  ("FileChange", erase(Domain.fileChange)),
  ("TreeEntryKind", erase(Domain.treeEntryKind)),
  ("TreeEntry", erase(Domain.treeEntry)),
  ("TreeSnapshot", erase(Domain.treeSnapshot)),
  ("TreeDelta", erase(Domain.treeDelta)),
  ("ReviewSnapshot", erase(Domain.reviewSnapshot)),
  // render
  ("SpanClass", erase(Render.spanClass)),
  ("Span", erase(Render.span)),
  ("Cell", erase(Render.cell)),
  ("ExpandDir", erase(Render.expandDir)),
  ("Row", erase(Render.row)),
  ("RenderTarget", erase(Render.renderTarget)),
  ("RenderContent", erase(Render.renderContent)),
  ("FileRenderHeader", erase(Render.fileRenderHeader)),
  ("RenderChunk", erase(Render.renderChunk)),
  ("FileRender", erase(Render.fileRender)),
  ("FileSummary", erase(Render.fileSummary)),
  ("DiffSummary", erase(Render.diffSummary)),
  // events
  ("EventBody", erase(Events.eventBody)),
  ("Event", erase(Events.event)),
  // rpc
  ("BuildInfo", erase(Rpc.buildInfo)),
  ("UpgradeNotice", erase(Rpc.upgradeNotice)),
  ("EntityKind", erase(Rpc.entityKind)),
  ("RpcError", erase(Rpc.rpcError)),
  ("Since", erase(Rpc.since)),
  ("SubscribeScope", erase(Rpc.subscribeScope)),
  ("ViewSection", erase(Rpc.viewSection)),
  ("Mutation", erase(Rpc.mutation)),
  ("Request", erase(Rpc.request)),
  ("Response", erase(Rpc.response)),
  ("StreamItem", erase(Rpc.streamItem)),
  ("ClientMsg", erase(Rpc.clientMsg)),
  ("ServerMsg", erase(Rpc.serverMsg)),
  ("EnvelopeClientMsg", erase(Rpc.envelope(Rpc.clientMsg))),
  ("EnvelopeServerMsg", erase(Rpc.envelope(Rpc.serverMsg))),
])

let names: array<string> = Dict.keysToArray(schemas)

/// Parse `json` as `typeName` and serialise it back. `Error` carries the
/// schema error message (or "no schema").
let roundtrip = (typeName: string, json: JSON.t): result<JSON.t, string> =>
  switch Dict.get(schemas, typeName) {
  | None => Error("no schema for " ++ typeName)
  | Some(schema) =>
    try {
      let value = S.parseJsonOrThrow(json, schema)
      Ok(S.reverseConvertToJsonOrThrow(value, schema))
    } catch {
    | exn =>
      Error(
        switch exn {
        | Exn.Error(e) => Exn.message(e)->Option.getOr("unknown error")
        | _ => "unknown error"
        },
      )
    }
  }
