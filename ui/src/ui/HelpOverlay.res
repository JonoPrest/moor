// `?` overlay (§6.4): every binding of the focused context plus Global,
// searchable, showing overrides and conflicts.

open View

// jsdom has no `scrollBy`; its `scrollTop` fallback also makes the behavior
// deterministic for component tests.
let scrollPanel: (Dom.element, int) => unit = %raw(`(el, top) => {
  if (el.scrollBy) el.scrollBy({top, behavior: "instant"})
  else el.scrollTop += top
}`)
let resetPanel: Dom.element => unit = %raw(`el => { el.scrollTop = 0 }`)

module Keys = {
  type ev
  @get external key: ev => string = "key"
  @send external preventDefault: ev => unit = "preventDefault"
  @val @scope("window") external listen: (string, ev => unit) => unit = "addEventListener"
  @val @scope("window") external unlisten: (string, ev => unit) => unit = "removeEventListener"
}

let compact = (text: string) => text->String.toLowerCase->String.replaceRegExp(/\s+/g, "")

// Fuzzy subsequence match. Exact and prefix matches sort before sparse
// subsequences; otherwise tighter, earlier matches receive the lower score.
let fuzzyScore = (haystack: string, needle: string): option<int> => {
  let haystack = compact(haystack)
  let needle = compact(needle)
  if needle == "" {
    Some(0)
  } else if haystack == needle {
    Some(-10000)
  } else if haystack->String.startsWith(needle) {
    Some(-5000 + String.length(haystack) - String.length(needle))
  } else {
    let from = ref(0)
    let first = ref(-1)
    let last = ref(-1)
    let found = ref(true)
    needle
    ->String.split("")
    ->Array.forEach(ch =>
      if found.contents {
        switch haystack->String.indexOfFrom(ch, from.contents) {
        | -1 => found := false
        | index => {
            if first.contents == -1 {
              first := index
            }
            last := index
            from := index + 1
          }
        }
      }
    )
    found.contents
      ? Some(
          first.contents * 100 +
          (last.contents - first.contents - String.length(needle) + 1) * 10 +
          String.length(haystack),
        )
      : None
  }
}

let entryScore = (entry: HelpEntry.t, query: string): option<int> =>
  switch (fuzzyScore(entry.keys, query), fuzzyScore(entry.label, query)) {
  | (Some(keys), Some(label)) => Some(Math.Int.min(keys, label + 1))
  | (Some(keys), None) => Some(keys)
  | (None, Some(label)) => Some(label + 1)
  | (None, None) => None
  }

let rankedEntries = (entries: array<HelpEntry.t>, query: string): array<HelpEntry.t> =>
  if compact(query) == "" {
    entries
  } else {
    entries
    ->Array.mapWithIndex((entry, index) => (entry, index))
    ->Array.filterMap(((entry, index)) =>
      entryScore(entry, query)->Option.map(score => (entry, score, index))
    )
    ->Array.toSorted(((_, a, ai), (_, b, bi)) => {
      let order = Int.compare(a, b)
      order == 0. ? Int.compare(ai, bi) : order
    })
    ->Array.map(((entry, _, _)) => entry)
  }

let filteredGroups = (groups: array<HelpGroup.t>, query: string): array<HelpGroup.t> =>
  groups->Array.filterMap(group => {
    let entries = rankedEntries(group.entries, query)
    Array.length(entries) == 0 ? None : Some({...group, entries})
  })

let conflictMatches = (conflict: Conflict.t, query: string) =>
  compact(query) == "" ||
  fuzzyScore(conflict.keys, query)->Option.isSome ||
  conflict.commands->Array.some(command => fuzzyScore((command :> string), query)->Option.isSome)

let scrollStep = key =>
  switch key {
  | "j" | "ArrowDown" => Some(48)
  | "k" | "ArrowUp" => Some(-48)
  | "PageDown" => Some(400)
  | "PageUp" => Some(-400)
  | _ => None
  }

@react.component
let make = (~help: HelpView.t, ~dispatch: Action.t => unit) => {
  let (query, setQuery) = React.useState(() => "")
  let groups = filteredGroups(help.groups, query)
  let conflicts = help.conflicts->Array.filter(conflict => conflictMatches(conflict, query))
  // j/k, arrows, and paging scroll while the query input retains focus.
  let panelRef = React.useRef(Nullable.null)
  let navigate = key =>
    switch (scrollStep(key), panelRef.current->Nullable.toOption) {
    | (Some(top), Some(element)) => element->scrollPanel(top)
    | _ => ()
    }
  React.useEffect0(() => {
    let handler = ev => {
      switch scrollStep(Keys.key(ev)) {
      | Some(_) => {
          Keys.preventDefault(ev)
          navigate(Keys.key(ev))
        }
      | None => ()
      }
    }
    Keys.listen("keydown", handler)
    Some(() => Keys.unlisten("keydown", handler))
  })
  React.useEffect1(() => {
    panelRef.current->Nullable.toOption->Option.forEach(resetPanel)
    None
  }, [query])
  <div className="help-overlay" role="dialog" ariaLabel="keyboard help">
    <div className="help-panel panel" ref={ReactDOM.Ref.domRef(panelRef)}>
      <header className="panel-header">
        {React.string("Keyboard")}
        <UI.Button label="close ⎋" kind=Ghost onClick={() => dispatch(ToggleHelp({}))} />
      </header>
      <UI.TextInput
        value=query
        autoFocus=true
        placeholder="filter…"
        onChange={q => setQuery(_ => q)}
        onKey={key =>
          switch key {
          | "Escape" => dispatch(ToggleHelp({}))
          | key => navigate(key)
          }}
        preventKeys=["j", "k", "ArrowDown", "ArrowUp", "PageDown", "PageUp"]
      />
      {Array.length(groups) == 0 && Array.length(conflicts) == 0
        ? <UI.Empty text="no shortcuts match" />
        : React.null}
      {groups
      ->Array.map(g =>
        <section key={(g.context :> string)} className="help-group">
          <h3> {React.string((g.context :> string))} </h3>
          <table>
            <tbody>
              {g.entries
              ->Array.map(e =>
                <tr key={e.keys ++ e.label} className={e.overridden ? "help-overridden" : ""}>
                  <td>
                    <UI.Kbd keys=e.keys />
                  </td>
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
      {Array.length(conflicts) > 0
        ? <section className="help-conflicts">
            <h3> {React.string("Conflicts")} </h3>
            {conflicts
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
