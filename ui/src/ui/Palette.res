// The palette (UI-DESIGN §Search): one overlay, `F` opens content
// search, `:` opens actions; `tab` cycles between them. Content results
// come from the daemon (`ViewModel.contentSearch`); actions are the
// keymap chrome — never hand-written.

open View

type mode = Content | Actions

@react.component
let make = (
  ~contentSearch: option<ContentSearchView.t>,
  ~actionPalette: bool,
  ~chrome: array<Hint.t>,
  ~dispatch: Action.t => unit,
) => {
  let mode = actionPalette ? Actions : Content
  let (text, setText) = React.useState(() => "")
  let close = () =>
    switch mode {
    | Content => dispatch(ContentSearch({query: None, allFiles: false}))
    | Actions => dispatch(ActionPalette({open_: false}))
    }
  let switchMode = () => {
    setText(_ => "")
    switch mode {
    | Content => {
        dispatch(ContentSearch({query: None, allFiles: false}))
        dispatch(ActionPalette({open_: true}))
      }
    | Actions => {
        dispatch(ActionPalette({open_: false}))
        dispatch(ContentSearch({query: Some(""), allFiles: false}))
      }
    }
  }
  let allFiles = contentSearch->Option.mapOr(false, c => c.allFiles)
  let actions =
    chrome->Array.filter(h =>
      text == "" || String.includes(String.toLowerCase(h.label), String.toLowerCase(text))
    )
  let submit = () =>
    switch mode {
    | Content => dispatch(ContentSearch({query: Some(text), allFiles}))
    | Actions =>
      switch actions->Array.get(0) {
      | Some(h) => {
          dispatch(ActionPalette({open_: false}))
          dispatch(RunCommand({command: h.command}))
        }
      | None => ()
      }
    }
  let onKey = k =>
    switch k {
    | "Escape" => close()
    | "Enter" => submit()
    | "Tab" => switchMode()
    | _ => ()
    }
  <div className="palette-overlay" role="dialog" ariaLabel="palette">
    <div className="palette">
      <div className="palette-tabs">
        <span className={"palette-tab" ++ (mode == Content ? " active" : "")}>
          {React.string("Content")}
        </span>
        <span className={"palette-tab" ++ (mode == Actions ? " active" : "")}>
          {React.string("Actions")}
        </span>
        <span className="palette-hint">
          <UI.Kbd keys="tab" /> {React.string(" switch · ")} <UI.Kbd keys="esc" /> {React.string(" close")}
        </span>
      </div>
      <UI.TextInput
        value=text
        autoFocus=true
        placeholder={mode == Content ? "search file contents (enter)" : "run a command"}
        onChange={v => setText(_ => v)}
        onKey
        preventKeys=["Tab"]
      />
      {switch mode {
      | Content => {
          let cs = contentSearch
          <div className="palette-results">
            <label className="palette-scope">
              <input
                type_="checkbox"
                checked=allFiles
                onChange={_ =>
                  dispatch(
                    ContentSearch({
                      query: Some(cs->Option.mapOr("", c => c.query)),
                      allFiles: !allFiles,
                    }),
                  )}
              />
              {React.string(" all files (not just changed)")}
            </label>
            {switch cs {
            | Some(c) if c.pending => <UI.Empty text="searching…" />
            | Some(c) if c.query != "" && Array.length(c.hits) == 0 =>
              <UI.Empty text="no matches" />
            | Some(c) =>
              <ul role="list">
                {c.hits
                ->Array.mapWithIndex((h, i) =>
                  <li
                    key={Int.toString(i)}
                    className="search-hit"
                    onClick={_ => {
                      dispatch(ContentSearch({query: None, allFiles: false}))
                      dispatch(
                        Viewport({
                          file: {repoId: h.repoId, path: h.path},
                          firstRow: Int.toFloat(h.line - 30)->Math.max(0.)->Float.toInt,
                          lastRow: h.line + 30,
                        }),
                      )
                    }}>
                    <span className="hit-path">
                      {React.string(h.path ++ ":" ++ Int.toString(h.line))}
                    </span>
                    <span className="hit-text"> {React.string(h.text)} </span>
                  </li>
                )
                ->React.array}
                {c.truncated
                  ? <li className="palette-truncated"> {React.string("more matches not shown")} </li>
                  : React.null}
              </ul>
            | None => React.null
            }}
          </div>
        }
      | Actions =>
        <ul className="palette-results" role="list">
          {actions
          ->Array.map(h =>
            <li
              key={h.keys ++ h.label}
              className="search-hit"
              onClick={_ => {
                dispatch(ActionPalette({open_: false}))
                dispatch(RunCommand({command: h.command}))
              }}>
              <span className="hit-text"> {React.string(h.label)} </span>
              <UI.Kbd keys=h.keys />
            </li>
          )
          ->React.array}
        </ul>
      }}
    </div>
  </div>
}
