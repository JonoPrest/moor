// The palette (UI-DESIGN §Search): one overlay, `F` opens content
// search, `:` opens actions; `tab` cycles between them. Content results
// come from the daemon (`ViewModel.contentSearch`); actions are the
// keymap chrome — never hand-written.

open View

type mode = Content | Actions

// jsdom has no scrollIntoView; guard so component tests can mount this.
let scrollNearest: Dom.element => unit = %raw(`el => el.scrollIntoView && el.scrollIntoView({block: "nearest"})`)

@react.component
let make = (
  ~contentSearch: option<ContentSearchView.t>,
  ~actionPalette: bool,
  ~chrome: array<Hint.t>,
  ~dispatch: Action.t => unit,
) => {
  let mode = actionPalette ? Actions : Content
  let (text, setText) = React.useState(() => "")
  // Actions-mode selection is UI state: the filtered list lives here.
  let (actionSel, setActionSel) = React.useState(() => 0)
  let close = () =>
    switch mode {
    | Content => dispatch(ContentSearch({query: None, allFiles: false}))
    | Actions => dispatch(ActionPalette({open_: false}))
    }
  let switchMode = () => {
    setText(_ => "")
    setActionSel(_ => 0)
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
  // Fuzzy subsequence match over the action's config name
  // (`toggle_layout`) and its label; lower score (earlier hits) first.
  let toSnake = (name: string) =>
    name->String.replaceRegExp(/([a-z0-9])([A-Z])/g, "$1_$2")->String.toLowerCase
  let fuzzy = (hay: string, needle: string): option<int> => {
    let score = ref(0)
    let from = ref(0)
    let ok = ref(true)
    needle
    ->String.split("")
    ->Array.forEach(ch =>
      if ok.contents {
        switch hay->String.indexOfFrom(ch, from.contents) {
        | -1 => ok := false
        | i => {
            score := score.contents + i
            from := i + 1
          }
        }
      }
    )
    ok.contents ? Some(score.contents) : None
  }
  let actions = {
    let q = String.toLowerCase(String.trim(text))
    chrome
    ->Array.filterMap(h => {
      if q == "" {
        Some((h, 0))
      } else {
        let name = toSnake((h.command :> string))
        switch (fuzzy(name, q), fuzzy(String.toLowerCase(h.label), q)) {
        | (Some(a), Some(b)) => Some((h, Math.Int.min(a, b)))
        | (Some(a), None) => Some((h, a))
        | (None, Some(b)) => Some((h, b))
        | (None, None) => None
        }
      }
    })
    ->Array.toSorted(((_, a), (_, b)) => Int.compare(a, b))
    ->Array.map(((h, _)) => h)
  }
  let openHit = (h: Domain.ContentHit.t) => {
    dispatch(ContentSearch({query: None, allFiles: false}))
    dispatch(
      Viewport({
        file: {repoId: h.repoId, path: h.path},
        firstRow: Int.toFloat(h.line - 30)->Math.max(0.)->Float.toInt,
        lastRow: h.line + 30,
      }),
    )
  }
  let submit = () =>
    switch mode {
    | Content =>
      // Results for this query on screen: Enter opens the highlighted
      // one; otherwise it (re)runs the search.
      switch contentSearch {
      | Some(c) if c.query == text && !c.pending =>
        switch c.hits[c.selected] {
        | Some(h) => openHit(h)
        | None => dispatch(ContentSearch({query: Some(text), allFiles}))
        }
      | _ => dispatch(ContentSearch({query: Some(text), allFiles}))
      }
    | Actions =>
      switch actions->Array.get(actionSel)->Option.orElse(actions->Array.get(0)) {
      | Some(h) => {
          dispatch(ActionPalette({open_: false}))
          dispatch(RunCommand({command: h.command}))
        }
      | None => ()
      }
    }
  let step = delta =>
    switch mode {
    | Content => dispatch(SearchStep({delta: delta}))
    | Actions => {
        let n = Array.length(actions)
        setActionSel(sel => n == 0 ? 0 : Math.Int.min(Math.Int.max(sel + delta, 0), n - 1))
      }
    }
  let onKey = k =>
    switch k {
    | "Escape" => close()
    | "Enter" => submit()
    | "Tab" => switchMode()
    | "ArrowDown" => step(1)
    | "ArrowUp" => step(-1)
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
          <UI.Kbd keys="tab" />
          {React.string(" switch · ")}
          <UI.Kbd keys="esc" />
          {React.string(" close")}
        </span>
      </div>
      <UI.TextInput
        value=text
        autoFocus=true
        placeholder={mode == Content ? "search file contents (enter)" : "run a command"}
        onChange={v => {
          setText(_ => v)
          setActionSel(_ => 0)
        }}
        onKey
        preventKeys=["Tab", "ArrowDown", "ArrowUp"]
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
                    className={"search-hit" ++ (i == c.selected ? " selected" : "")}
                    ref={ReactDOM.Ref.callbackDomRef(el => {
                      if i == c.selected {
                        switch el->Nullable.toOption {
                        | Some(el) => el->scrollNearest
                        | None => ()
                        }
                      }
                      None
                    })}
                    onClick={_ => openHit(h)}
                  >
                    <span className="hit-path">
                      {React.string(h.path ++ ":" ++ Int.toString(h.line))}
                    </span>
                    <span className="hit-text"> {React.string(h.text)} </span>
                  </li>
                )
                ->React.array}
                {c.truncated
                  ? <li className="palette-truncated">
                      {React.string("more matches not shown")}
                    </li>
                  : React.null}
              </ul>
            | None => React.null
            }}
          </div>
        }
      | Actions =>
        <ul className="palette-results" role="list">
          {actions
          ->Array.mapWithIndex((h, i) =>
            <li
              key={h.keys ++ h.label}
              className={"search-hit" ++ (i == actionSel ? " selected" : "")}
              ref={ReactDOM.Ref.callbackDomRef(el => {
                if i == actionSel {
                  switch el->Nullable.toOption {
                  | Some(el) => el->scrollNearest
                  | None => ()
                  }
                }
                None
              })}
              onClick={_ => {
                dispatch(ActionPalette({open_: false}))
                dispatch(RunCommand({command: h.command}))
              }}
            >
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
