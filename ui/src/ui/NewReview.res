// The review-create form (§4.4): a title, a workspace, and one target per
// chosen repo — any base against any head, across repos.

open Domain

type target = {repoId: string, base: string, head: string}

let firstRepo = (ws: Workspace.t): option<target> =>
  ws.repos[0]->Option.map(r => {repoId: r.id, base: "main", head: "worktree"})

@react.component
let make = (~workspaces: array<Workspace.t>, ~dispatch: Action.t => unit) => {
  let (open_, setOpen) = React.useState(() => false)
  let (title, setTitle) = React.useState(() => "")
  let (workspaceId, setWorkspace) = React.useState(() =>
    workspaces[0]->Option.map(w => w.id)->Option.getOr("")
  )
  let workspace = workspaces->Array.find(w => w.id == workspaceId)
  let (targets, setTargets) = React.useState(() =>
    workspace->Option.flatMap(firstRepo)->Option.map(t => [t])->Option.getOr([])
  )
  let parsed: array<option<ReviewTarget.t>> = targets->Array.map(t =>
    switch (RefSpecText.parse(t.base), RefSpecText.parse(t.head)) {
    | (Some(base), Some(head)) => Some({ReviewTarget.repoId: t.repoId, base, head})
    | _ => None
    }
  )
  let valid = String.trim(title) != "" && Array.length(parsed) > 0 && parsed->Array.every(Option.isSome)
  let submit = () =>
    if valid {
      dispatch(
        CreateReview({
          workspaceId,
          title: String.trim(title),
          targets: parsed->Array.filterMap(t => t),
        }),
      )
      setOpen(_ => false)
      setTitle(_ => "")
    }
  let update = (i, f: target => target) =>
    setTargets(ts => ts->Array.mapWithIndex((t, j) => i == j ? f(t) : t))
  if !open_ {
    <div className="new-review">
      <UI.Button label="New review" kind=Primary onClick={() => setOpen(_ => true)} />
    </div>
  } else {
    <form
      className="new-review panel"
      ariaLabel="new review"
      onSubmit={ev => {
        ReactEvent.Form.preventDefault(ev)
        submit()
      }}>
      <UI.TextInput
        value=title
        placeholder="Title"
        autoFocus=true
        onChange={t => setTitle(_ => t)}
        onKey={key =>
          if key == "Escape" {
            setOpen(_ => false)
          }}
      />
      {Array.length(workspaces) > 1
        ? <UI.Select
            ariaLabel="workspace"
            value=workspaceId
            options={workspaces->Array.map(w => (w.id, w.name))}
            onChange={id => setWorkspace(_ => id)}
          />
        : React.null}
      {targets
      ->Array.mapWithIndex((t, i) =>
        <div key={Int.toString(i)} className="new-review-target">
          <UI.Select
            ariaLabel="repo"
            value=t.repoId
            options={workspace
            ->Option.map(w => w.repos->Array.map(r => (r.id, r.displayName)))
            ->Option.getOr([])}
            onChange={id => update(i, t => {...t, repoId: id})}
          />
          <UI.TextInput value=t.base placeholder="base (main, tag:v1, commit:…)" onChange={b => update(i, t => {...t, base: b})} />
          <UI.TextInput value=t.head placeholder="head (worktree, head, branch)" onChange={h => update(i, t => {...t, head: h})} />
          <UI.Button
            label="−"
            kind=Ghost
            title="remove target"
            onClick={() => setTargets(ts => ts->Array.filterWithIndex((_, j) => j != i))}
          />
        </div>
      )
      ->React.array}
      <UI.Box direction=Row gap=Sm>
        <UI.Button
          label="+ target"
          onClick={() =>
            switch workspace->Option.flatMap(firstRepo) {
            | Some(t) => setTargets(ts => ts->Array.concat([t]))
            | None => ()
            }}
        />
        {switch workspace->Option.flatMap(firstRepo) {
        | Some(_) => <UI.Button label="Create" kind=Primary onClick=submit />
        | None => React.null
        }}
        <UI.Button label="Cancel" kind=Ghost onClick={() => setOpen(_ => false)} />
      </UI.Box>
      {switch workspace->Option.flatMap(firstRepo) {
      | Some(_) => React.null
      | None =>
        <p className="new-review-hint">
          {React.string(
            "No repositories to review. Attach one first: `moor workspace add <name>` (prints an id) then `moor workspace attach <id> [path]`, and refresh (R).",
          )}
        </p>
      }}
    </form>
  }
}
