// Fuzzy file search (§5.5): typing dispatches `FileSearch`, Enter opens
// the top hit, Esc closes. All hits come from the core; nothing is fetched.

open View

@react.component
let make = (~search: SearchView.t, ~dispatch: Action.t => unit) => {
  let onKeyDown = (ev: ReactEvent.Keyboard.t) => {
    switch ReactEvent.Keyboard.key(ev) {
    | "Escape" => dispatch(FileSearch({query: None}))
    | "Enter" =>
      switch search.hits[0] {
      | Some(hit) => dispatch(Viewport({file: hit.file, firstRow: 0, lastRow: 59}))
      | None => ()
      }
    | _ => ()
    }
    ReactEvent.Keyboard.stopPropagation(ev)
  }
  <div className="search-box panel" role="search">
    <input
      className="search-input"
      autoFocus=true
      placeholder="file…"
      value=search.query
      onChange={ev => dispatch(FileSearch({query: Some(ReactEvent.Form.target(ev)["value"])}))}
      onKeyDown
    />
    <ul className="search-hits">
      {search.hits
      ->Array.mapWithIndex((h, i) =>
        <li
          key={h.file.path ++ Int.toString(i)}
          className="search-hit"
          onClick={_ => dispatch(Viewport({file: h.file, firstRow: 0, lastRow: 59}))}>
          {React.string(h.file.path)}
        </li>
      )
      ->React.array}
    </ul>
  </div>
}
