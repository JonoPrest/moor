// The open file (§6.5): a virtualized list over `total_rows`; rows the
// cache holds render, the rest are placeholders until their chunk lands.
// Scrolling dispatches `Viewport` so the core fetches what is visible.

open View

let rowHeight = 20

/// Rows the virtualizer shows → the viewport the core should serve.
let viewportOf = (items: array<Virtual.virtualItem>): option<(int, int)> =>
  switch (items[0], items[Array.length(items) - 1]) {
  | (Some(first), Some(last)) => Some((first.index, last.index))
  | _ => None
  }

@react.component
let make = (~diff: DiffView.t, ~layout: Layout.t, ~focus: Focus.t, ~dispatch: Action.t => unit) => {
  let scrollRef = React.useRef(Nullable.null)
  let total = switch diff.content {
  | Text({totalRows}) => totalRows
  | Binary(_) => 0
  }
  let virtualizer = Virtual.useVirtualizer({
    count: total,
    getScrollElement: () => scrollRef.current,
    estimateSize: _ => rowHeight,
    overscan: 10,
  })
  let items = virtualizer->Virtual.getVirtualItems
  let viewport = viewportOf(items)
  // Ask for the window we show whenever it moves off what the core holds.
  React.useEffect2(() => {
    switch viewport {
    | Some((first, last)) if first != diff.firstRow || last != diff.lastRow =>
      dispatch(Viewport({file: diff.file, firstRow: first, lastRow: last}))
    | _ => ()
    }
    None
  }, (viewport, diff.file))
  // Keep the focused row on screen.
  let focusedRow = switch focus {
  | Diff({row}) => Some(row)
  | _ => None
  }
  React.useEffect1(() => {
    switch focusedRow {
    | Some(row) => virtualizer->Virtual.scrollToIndex(row)
    | None => ()
    }
    None
  }, [focusedRow])
  // A viewed file collapses (§4.4); the reader can expand it for this visit.
  let (expanded, setExpanded) = React.useState(() => false)
  let collapsed = diff.viewed == Viewed && !expanded
  let cached = Dict.make()
  diff.rows->Array.forEach(r => cached->Dict.set(Int.toString(r.index), r))
  let title = diff.file.path
  let binary = switch diff.content {
  | Binary(_) => <div className="diff-binary"> {React.string("binary file")} </div>
  | Text(_) => React.null
  }
  <section className="diff-panel panel" role="grid" ariaLabel=title>
    <header className="panel-header"> {React.string(title)} </header>
    {diff.original
      ? <div className="original-banner" role="status">
          {React.string("Viewing the diff this comment was made on — read-only. ")}
          <UI.Kbd keys="esc" />
          {React.string(" back to the current diff")}
        </div>
      : React.null}
    {diff.fileThreads->Array.length > 0
      ? <div className="file-threads">
          {React.string(Int.toString(Array.length(diff.fileThreads)) ++ " file-level thread(s)")}
        </div>
      : React.null}
    binary
    {collapsed
      ? <div className="diff-collapsed">
          {React.string("Viewed — ")}
          <UI.Button label="show anyway" kind=Ghost onClick={() => setExpanded(_ => true)} />
        </div>
      : React.null}
    <div
      className={"diff-scroll" ++ (collapsed ? " hidden" : "")}
      ref={ReactDOM.Ref.domRef(scrollRef)}>
      <div
        className="diff-rows"
        style={{
          height: Int.toString(virtualizer->Virtual.getTotalSize) ++ "px",
          position: "relative",
        }}>
        {items
        ->Array.map(item => {
          let style: ReactDOM.Style.t = {
            position: "absolute",
            top: "0",
            left: "0",
            width: "100%",
            height: Int.toString(item.size) ++ "px",
            transform: "translateY(" ++ Int.toString(item.start) ++ "px)",
          }
          let focused = focusedRow == Some(item.index)
          let inner = switch cached->Dict.get(Int.toString(item.index)) {
          | Some(r) =>
            <Row
              row=r.row
              layout
              index=item.index
              focused
              threads={Array.length(r.threads)}
              onClick={() => dispatch(SetFocus({focus: Focus.Diff({row: item.index})}))}
            />
          | None =>
            Attrs.focused(
              <div className="row row-placeholder" role="row">
                {React.string("…")}
              </div>,
              focused,
            )
          }
          <div key=item.key style> inner </div>
        })
        ->React.array}
      </div>
    </div>
  </section>
}
