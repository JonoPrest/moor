// Fuzzy file search (§5.5): typing dispatches `FileSearch`, Down/Up step
// the highlighted hit (core state), Enter opens it, Esc closes. All hits
// come from the core; nothing is fetched.

open View

// jsdom has no scrollIntoView; guard so component tests can mount this.
let scrollNearest: Dom.element => unit = %raw(`el => el.scrollIntoView && el.scrollIntoView({block: "nearest"})`)

@react.component
let make = (~search: SearchView.t, ~dispatch: Action.t => unit) => {
  let onKey = key =>
    switch key {
    | "Escape" => dispatch(FileSearch({query: None}))
    | "ArrowDown" => dispatch(SearchStep({delta: 1}))
    | "ArrowUp" => dispatch(SearchStep({delta: -1}))
    | "Enter" =>
      switch search.hits[search.selected] {
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
      preventKeys=["ArrowDown", "ArrowUp"]
    />
    <ul className="search-hits">
      {search.hits
      ->Array.mapWithIndex((h, i) =>
        <li
          key={h.file.path ++ Int.toString(i)}
          className={"search-hit" ++ (i == search.selected ? " selected" : "")}
          ref={ReactDOM.Ref.callbackDomRef(el => {
            if i == search.selected {
              switch el->Nullable.toOption {
              | Some(el) => el->scrollNearest
              | None => ()
              }
            }
            None
          })}
          onClick={_ => dispatch(Viewport({file: h.file, firstRow: 0, lastRow: 59}))}>
          {React.string(h.file.path)}
        </li>
      )
      ->React.array}
    </ul>
  </div>
}
