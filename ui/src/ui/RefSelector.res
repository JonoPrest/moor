open Domain

let kind = spec =>
  switch spec {
  | RefSpec.Branch(_) => "branch"
  | Tag(_) => "tag"
  | Commit(_) => "commit"
  | WorkingTree(_) => "working tree"
  | Upstream(_) => "upstream"
  | Head(_) => "head"
  }

let value = spec =>
  switch spec {
  | RefSpec.Branch({name}) | Tag({name}) => name
  | Commit({oid}) => String.slice(oid, ~start=0, ~end=8)
  | WorkingTree(_) => "Working tree"
  | Upstream(_) => "@{upstream}"
  | Head(_) => "HEAD"
  }

@react.component
let make = (~selector: View.RefSelectorView.t, ~dispatch: Action.t => unit) => {
  let side = switch selector.side {
  | Base => "Base"
  | Head => "Head"
  }
  let onKey = key =>
    switch key {
    | "Escape" => dispatch(Action.CloseRefSelector({}))
    | "Enter" => dispatch(Action.SelectCurrentRef({}))
    | "ArrowDown" | "j" => dispatch(Action.RefSelectorStep({delta: 1}))
    | "ArrowUp" | "k" => dispatch(Action.RefSelectorStep({delta: -1}))
    | _ => ()
    }
  <div className="palette-overlay" role="dialog" ariaLabel={side ++ " revision selector"}>
    <div className="palette ref-selector">
      <div className="palette-tabs">
        <strong> {React.string(selector.repoName ++ " · " ++ side)} </strong>
        <span className="palette-hint">
          <UI.Kbd keys="↑/↓ or j/k" />
          {React.string(" move · ")}
          <UI.Kbd keys="enter" />
          {React.string(" select · ")}
          <UI.Kbd keys="esc" />
          {React.string(" cancel")}
        </span>
      </div>
      <UI.TextInput
        value=selector.query
        autoFocus=true
        placeholder={"Find a " ++ String.toLowerCase(side) ++ " revision"}
        onChange={(query: string) => dispatch(Action.RefSelectorQuery({query: query}))}
        onKey
        preventKeys=["Enter", "ArrowDown", "ArrowUp", "j", "k"]
      />
      {switch selector.status {
      | Loading(_) => <UI.Empty text="Loading branches, tags, and recent commits…" />
      | Saving(_) => <UI.Empty text="Updating review target…" />
      | InvalidRef({message}) =>
        <div className="ref-selector-error" role="alert">
          {React.string("Invalid ref: " ++ message)}
        </div>
      | DaemonError({message}) =>
        <div className="ref-selector-error" role="alert">
          {React.string("Daemon error: " ++ message)}
        </div>
      | Ready(_) if Array.length(selector.options) == 0 => <UI.Empty text="No matching refs" />
      | Ready(_) =>
        <ul className="ref-selector-results" role="listbox">
          {selector.options
          ->Array.mapWithIndex((option, index) => {
            let prefix = index == selector.selected ? "› " : "  "
            let current = option.current ? " · current" : ""
            let subject = option.subject->Option.mapOr("", text => " · " ++ text)
            let label = prefix ++ value(option.refSpec) ++ subject ++ current
            <li key={kind(option.refSpec) ++ ":" ++ value(option.refSpec)}>
              <span className="ref-selector-kind">
                <UI.Badge text={kind(option.refSpec)} tone=Neutral />
              </span>
              <UI.Button
                label
                kind={index == selector.selected ? Primary : Ghost}
                onClick={() => dispatch(Action.SelectRef({index: index}))}
              />
            </li>
          })
          ->React.array}
        </ul>
      }}
    </div>
  </div>
}
