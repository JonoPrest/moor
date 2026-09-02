// One file of the stacked diff view (UI-DESIGN §Layout, GitHub-style):
// collapsible header with path · copy · +A −D · comment · Viewed ·
// expand, then every cached row with inline threads. Dragging across
// line numbers opens a multiline comment draft (`CommentLines`).

open View

/// The commentable side+line of a row, mirroring the core's line_anchor.
let lineOf = (row: Render.Row.t): option<(Domain.Side.t, int)> =>
  switch row {
  | Context({right}) | Modified({right}) | Added({right}) => Some((Head, right.lineNo))
  | Removed({left}) => Some((Base, left.lineNo))
  | HunkHeader(_) | Expander(_) | WhitespaceOnly(_) => None
  }

@val @scope(("navigator", "clipboard")) external writeText: string => unit = "writeText"
@send external scrollIntoView: (Dom.element, {"block": string}) => unit = "scrollIntoView"

type drag = {side: Domain.Side.t, start: int, current: int}

@react.component
let make = (
  ~diff: DiffView.t,
  ~layout: Layout.t,
  ~focus: Focus.t,
  ~threads: array<ThreadView.t>,
  ~draft: option<Draft.t>,
  ~pendingRefresh: bool,
  ~isOpen: bool,
  ~visual: option<VisualView.t>=?,
  ~dispatch: Action.t => unit,
) => {
  // Fold state is core-owned (`z a` toggles; motions skip folded files).
  let collapsed = diff.collapsed
  let (drag, setDrag) = React.useState(() => None)
  // Patches are viewport-bounded (§6.3): accumulate every row this file
  // has ever shown so scrolling/loading only ever adds rows.
  let key = DiffSeen.fileKey(diff)
  let prevKey = React.useRef("")
  let seen = React.useRef(Dict.make())
  seen.current = DiffSeen.mergeSeen(seen.current, prevKey.current, key, diff.rows)
  prevKey.current = key
  let rows =
    seen.current
    ->Dict.valuesToArray
    ->Array.toSorted((a: DiffRow.t, b: DiffRow.t) => Int.compare(a.index, b.index))
  let threadOf = (id: Ids.threadId) => threads->Array.findIndexOpt(t => t.id == id)
  let focusedThread = switch focus {
  | Thread({index}) => Some(index)
  | _ => None
  }
  let focusedRow = switch focus {
  | Diff({row}) if isOpen => Some(row)
  | _ => None
  }
  let replyTo = draft->Option.flatMap(d => d.replyTo)
  let total = switch diff.content {
  | Text({totalRows}) => totalRows
  | Binary(_) => 0
  }
  let stats = switch diff.content {
  | Text({additions, deletions}) =>
    <span className="file-stats">
      <span className="stat-add"> {React.string("+" ++ Int.toString(additions))} </span>
      <span className="stat-del"> {React.string("−" ++ Int.toString(deletions))} </span>
    </span>
  | Binary(_) => React.null
  }
  // Clicking a file in the tree opens it; scroll its section into view.
  let sectionRef = React.useRef(Nullable.null)
  let wasOpen = React.useRef(false)
  React.useEffect1(() => {
    if isOpen && !wasOpen.current {
      switch sectionRef.current->Nullable.toOption {
      | Some(el) => el->scrollIntoView({"block": "start"})
      | None => ()
      }
    }
    wasOpen.current = isOpen
    None
  }, [isOpen])
  // Visual-mode selection (core-owned): only the open file's rows.
  let inVisual = (index: int) =>
    switch visual {
    | Some({start, end_}) if isOpen => index >= start && index <= end_
    | _ => false
    }
  let inDrag = (row: Render.Row.t) =>
    switch (drag, lineOf(row)) {
    | (Some({side, start, current}), Some((s, line))) if s == side =>
      line >= Math.Int.min(start, current) && line <= Math.Int.max(start, current)
    | _ => false
    }
  <section className="file-diff" ariaLabel=diff.file.path ref={ReactDOM.Ref.domRef(sectionRef)}>
    {Attrs.focused(
      <header className="file-diff-header">
        <button
          type_="button"
          className="btn btn-ghost file-chevron"
          title={collapsed ? "expand file section" : "collapse file section"}
          onClick={_ => dispatch(ToggleFileCollapse({file: diff.file}))}
        >
          {React.string(collapsed ? "▸" : "▾")}
        </button>
        <span className="file-path mono"> {React.string(diff.file.path)} </span>
        <button
          type_="button"
          className="btn btn-ghost"
          title="copy file path"
          onClick={_ => writeText(diff.file.path)}
        >
          {React.string("⧉")}
        </button>
        stats
        {diff.fileThreads->Array.length > 0
          ? <span className="tree-threads">
              {React.string(Int.toString(Array.length(diff.fileThreads)))}
            </span>
          : React.null}
        <span className="file-diff-actions">
          <button
            type_="button"
            className="btn btn-ghost"
            title="comment on this file"
            onClick={_ => dispatch(CommentFile({file: diff.file}))}
          >
            {React.string("💬")}
          </button>
          <UI.Button
            label="expand file"
            kind=Ghost
            title="show the whole file as context"
            onClick={() => dispatch(ExpandContext({file: diff.file, full: true}))}
          />
          <label className="chip-check" title="mark viewed (v)">
            <input
              type_="checkbox"
              checked={diff.viewed == Viewed}
              onChange={_ => {
                dispatch(
                  diff.viewed == Viewed
                    ? UnmarkViewed({file: diff.file})
                    : MarkViewed({file: diff.file}),
                )
              }}
            />
            {React.string("Viewed")}
          </label>
        </span>
      </header>,
      isOpen && collapsed,
    )}
    {collapsed
      ? React.null
      : <div className="file-diff-body" onMouseLeave={_ => setDrag(_ => None)}>
          {switch diff.content {
          | Binary(_) => <div className="diff-binary"> {React.string("binary file")} </div>
          | Text(_) => React.null
          }}
          {rows
          ->Array.map(r => {
            let focused = focusedRow == Some(r.index)
            let selected = inDrag(r.row) || inVisual(r.index)
            let rowEl =
              <Row
                row=r.row
                layout
                index=r.index
                focused
                threads={Array.length(r.threads)}
                onClick={() => {
                  dispatch(Viewport({file: diff.file, firstRow: r.index, lastRow: r.index + 59}))
                  dispatch(SetFocus({focus: Focus.Diff({row: r.index})}))
                }}
                onExpand={() => dispatch(ExpandContext({file: diff.file, full: false}))}
              />
            <div
              key={Int.toString(r.index)}
              className={selected ? "row-drag-selected" : ""}
              onMouseDown={_ =>
                switch lineOf(r.row) {
                | Some((side, line)) => setDrag(_ => Some({side, start: line, current: line}))
                | None => ()
                }}
              onMouseEnter={_ =>
                switch (drag, lineOf(r.row)) {
                | (Some(d), Some((side, line))) if side == d.side =>
                  setDrag(_ => Some({...d, current: line}))
                | _ => ()
                }}
              onMouseUp={_ =>
                switch drag {
                | Some({side, start, current}) => {
                    setDrag(_ => None)
                    if start != current {
                      dispatch(
                        CommentLines({
                          file: diff.file,
                          side,
                          startLine: Math.Int.min(start, current),
                          endLine: Math.Int.max(start, current),
                        }),
                      )
                    }
                  }
                | None => ()
                }}
            >
              rowEl
              {r.threads
              ->Array.filterMap(threadOf)
              ->Array.map(ti => {
                let thread = threads->Array.getUnsafe(ti)
                let composer = switch (replyTo, draft) {
                | (Some(id), Some(d)) if id == thread.id =>
                  <Composer draft=d pendingRefresh dispatch />
                | _ => React.null
                }
                <InlineThread
                  key=thread.id
                  thread
                  focused={focusedThread == Some(ti)}
                  index=ti
                  composer
                  dispatch
                />
              })
              ->React.array}
            </div>
          })
          ->React.array}
          {switch (Array.length(rows), total) {
          // More rows exist than we hold: load onward from the last seen
          // row (this makes the file the open one, so its viewport streams).
          | (have, total) if have > 0 && have < total =>
            <button
              type_="button"
              className="btn load-more"
              onClick={_ => {
                let from = (rows->Array.getUnsafe(have - 1)).index + 1
                dispatch(Viewport({file: diff.file, firstRow: from, lastRow: from + 199}))
              }}
            >
              {React.string("Load more (" ++ Int.toString(total - have) ++ " rows below)")}
            </button>
          | _ => React.null
          }}
        </div>}
  </section>
}
