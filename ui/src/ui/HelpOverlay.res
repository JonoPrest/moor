// `?` overlay (§6.4): every binding of the focused context plus Global,
// searchable, showing overrides and conflicts.

open View

@react.component
let make = (~help: HelpView.t, ~dispatch: Action.t => unit) => {
  let (query, setQuery) = React.useState(() => "")
  let q = String.toLowerCase(query)
  let matches = (e: HelpEntry.t) =>
    q == "" || String.includes(String.toLowerCase(e.label), q) || String.includes(e.keys, q)
  <div className="help-overlay" role="dialog" ariaLabel="keyboard help">
    <div className="help-panel panel">
      <header className="panel-header">
        {React.string("Keyboard")}
        <button className="btn" onClick={_ => dispatch(ToggleHelp({}))}> {React.string("close ⎋")} </button>
      </header>
      <input
        className="help-search"
        placeholder="filter…"
        value=query
        onChange={ev => setQuery(_ => ReactEvent.Form.target(ev)["value"])}
        onKeyDown={ev => {
          if ReactEvent.Keyboard.key(ev) == "Escape" {
            dispatch(ToggleHelp({}))
          }
          ReactEvent.Keyboard.stopPropagation(ev)
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
                  <td> <kbd> {React.string(e.keys)} </kbd> </td>
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
                <kbd> {React.string(c.keys)} </kbd>
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
