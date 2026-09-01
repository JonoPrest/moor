// The Browse tab's ref picker (UI-DESIGN §Browse): `viewing: <ref>`
// accepts any RefSpecText form; empty (or reset) returns to the review's
// head trees.

@react.component
let make = (
  ~browseRef: option<Domain.RefSpec.t>,
  ~repoId: option<Ids.repoId>,
  ~dispatch: Action.t => unit,
) => {
  let (text, setText) = React.useState(() => "")
  let (bad, setBad) = React.useState(() => false)
  let view = () =>
    switch (repoId, RefSpecText.parse(String.trim(text))) {
    | (Some(repoId), Some(spec)) => {
        setBad(_ => false)
        dispatch(SetBrowseRef({repoId, refSpec: Some(spec)}))
      }
    | (_, None) | (None, _) => setBad(_ => true)
    }
  <div className="browse-bar">
    <span className="browse-viewing">
      {React.string("viewing: ")}
      <span className="review-header-ref">
        {React.string(browseRef->Option.mapOr("review head", RefSpecText.print))}
      </span>
    </span>
    <UI.TextInput
      value=text
      placeholder="branch, tag:v1, commit:<oid>, worktree…"
      onChange={v => setText(_ => v)}
      onKey={k =>
        if k == "Enter" {
          view()
        }}
    />
    <UI.Button label="view" onClick=view />
    {switch (browseRef, repoId) {
    | (Some(_), Some(repoId)) =>
      <UI.Button
        label="head" kind=Ghost onClick={() => dispatch(SetBrowseRef({repoId, refSpec: None}))}
      />
    | (Some(_), None) | (None, _) => React.null
    }}
    {bad ? <span className="browse-bad"> {React.string("unrecognised ref")} </span> : React.null}
  </div>
}
