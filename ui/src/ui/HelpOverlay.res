// `?` overlay (§6.4): every binding of the focused context plus Global,
// searchable, showing overrides and conflicts.

open View

@send external scrollBy: (Dom.element, {"top": int, "behavior": string}) => unit = "scrollBy"

module Keys = {
  type ev
  @get external key: ev => string = "key"
  @send external preventDefault: ev => unit = "preventDefault"
  @val @scope("window") external listen: (string, ev => unit) => unit = "addEventListener"
  @val @scope("window") external unlisten: (string, ev => unit) => unit = "removeEventListener"
}

@react.component
let make = (~help: HelpView.t, ~dispatch: Action.t => unit) => {
  let (query, setQuery) = React.useState(() => "")
  let q = String.toLowerCase(query)
  let matches = (e: HelpEntry.t) =>
    q == "" || String.includes(String.toLowerCase(e.label), q) || String.includes(e.keys, q)
  // j/k and the arrows scroll the open overlay.
  let panelRef = React.useRef(Nullable.null)
  React.useEffect0(() => {
    let handler = ev => {
      let step = switch Keys.key(ev) {
      | "j" | "ArrowDown" => Some(48)
      | "k" | "ArrowUp" => Some(-48)
      | "PageDown" => Some(400)
      | "PageUp" => Some(-400)
      | _ => None
      }
      switch (step, panelRef.current->Nullable.toOption) {
      | (Some(top), Some(el)) => {
          Keys.preventDefault(ev)
          el->scrollBy({"top": top, "behavior": "instant"})
        }
      | _ => ()
      }
    }
    Keys.listen("keydown", handler)
    Some(() => Keys.unlisten("keydown", handler))
  })
  <div className="help-overlay" role="dialog" ariaLabel="keyboard help">
    <div className="help-panel panel" ref={ReactDOM.Ref.domRef(panelRef)}>
      <header className="panel-header">
        {React.string("Keyboard")}
        <UI.Button label="close ⎋" kind=Ghost onClick={() => dispatch(ToggleHelp({}))} />
      </header>
      <UI.TextInput
        value=query
        placeholder="filter…"
        onChange={q => setQuery(_ => q)}
        onKey={key =>
          if key == "Escape" {
            dispatch(ToggleHelp({}))
          }}
      />
      {help.groups
      ->Array.map(g =>
        <section key={(g.context :> string)} className="help-group">
          <h3> {React.string((g.context :> string))} </h3>
          <table>
            <tbody>
              {g.entries
              ->Array.filter(matches)
              ->Array.map(e =>
                <tr key={e.keys ++ e.label} className={e.overridden ? "help-overridden" : ""}>
                  <td> <UI.Kbd keys=e.keys /> </td>
                  <td> {React.string(e.label)} </td>
                  <td> {React.string(e.primary ? "★" : "")} </td>
                </tr>
              )
              ->React.array}
            </tbody>
          </table>
        </section>
      )
      ->React.array}
      {Array.length(help.conflicts) > 0
        ? <section className="help-conflicts">
            <h3> {React.string("Conflicts")} </h3>
            {help.conflicts
            ->Array.map(c =>
              <div key={(c.context :> string) ++ c.keys}>
                <UI.Kbd keys=c.keys />
                {React.string(" in " ++ (c.context :> string) ++ ": ")}
                {React.string(c.commands->Array.map(cmd => (cmd :> string))->Array.join(", "))}
              </div>
            )
            ->React.array}
          </section>
        : React.null}
    </div>
  </div>
}
