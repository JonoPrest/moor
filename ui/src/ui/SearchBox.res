// Fuzzy file search (§5.5): typing dispatches `FileSearch`, Enter opens
// the top hit, Esc closes. All hits come from the core; nothing is fetched.

open View

@react.component
let make = (~search: SearchView.t, ~dispatch: Action.t => unit) => {
  let onKey = key =>
    switch key {
    | "Escape" => dispatch(FileSearch({query: None}))
    | "Enter" =>
      switch search.hits[0] {
      | Some(hit) => dispatch(Viewport({file: hit.file, firstRow: 0, lastRow: 59}))
      | None => ()
      }
    | _ => ()
    }
  <div className="search-box panel" role="search">
    <UI.TextInput
      autoFocus=true
      placeholder="file…"
      value=search.query
      onChange={q => dispatch(FileSearch({query: Some(q)}))}
      onKey
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
