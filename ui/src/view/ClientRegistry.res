// Client (view/action) schemas by Rust type name, for the boundary test
// over `fixtures/client/`. Same contract as `Registry` for the protocol.

external erase: S.t<'a> => S.t<unknown> = "%identity"

let schemas: dict<S.t<unknown>> = Dict.fromArray([
  ("ViewModel", erase(View.viewModel)),
  ("ViewPrefs", erase(View.viewPrefs)),
  ("Layout", erase(View.Layout.schema)),
  ("ConnectionView", erase(View.ConnectionView.schema)),
  ("Draft", erase(View.draft)),
  ("PendingEvent", erase(View.pendingEvent)),
  ("OpenReview", erase(View.openReview)),
  ("OpenFile", erase(View.openFile)),
  ("RenderKey", erase(View.renderKey)),
  ("FileRef", erase(View.fileRef)),
  ("TreeView", erase(View.treeView)),
  ("TreeNode", erase(View.TreeNode.schema)),
  ("SearchView", erase(View.searchView)),
  ("SearchHit", erase(View.searchHit)),
  ("Progress", erase(View.progress)),
  ("ViewedState", erase(View.ViewedState.schema)),
  ("ChangeKindKind", erase(View.ChangeKindKind.schema)),
  ("DiffView", erase(View.diffView)),
  ("DiffRow", erase(View.diffRow)),
  ("ThreadView", erase(View.threadView)),
  ("ThreadPlace", erase(View.ThreadPlace.schema)),
  ("CommitStepper", erase(View.commitStepper)),
  ("StepperCommit", erase(View.stepperCommit)),
  ("Focus", erase(View.Focus.schema)),
  ("Hint", erase(View.hint)),
  ("HelpView", erase(View.helpView)),
  ("HelpGroup", erase(View.helpGroup)),
  ("HelpEntry", erase(View.helpEntry)),
  ("Conflict", erase(View.conflict)),
  ("Context", erase(View.Context.schema)),
  ("Command", erase(View.Command.schema)),
  ("Action", erase(Action.schema)),
  ("Override", erase(View.override)),
  ("Overrides", erase(View.overrides)),
  ("ViewDelta", erase(View.viewDelta)),
  ("ViewPatch", erase(View.ViewPatch.schema)),
])

let names: array<string> = Dict.keysToArray(schemas)

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
