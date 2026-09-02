// One render-model row (§6.5, §6.6): the same `Row` in unified or split
// layout, styled only through the semantic classes in app.css.

open Render

let spanClassName = (c: SpanClass.t): string =>
  switch c {
  | Keyword => "span-keyword"
  | String => "span-string"
  | Number => "span-number"
  | Comment => "span-comment"
  | Type => "span-type"
  | Function => "span-function"
  | Variable => "span-variable"
  | Constant => "span-constant"
  | Operator => "span-operator"
  | Punctuation => "span-punctuation"
  | Attribute => "span-attribute"
  | Tag => "span-tag"
  | Other => "span-other"
  }

let rowClassName = (row: Row.t): string =>
  switch row {
  | HunkHeader(_) => "row-hunk-header"
  | Context(_) => "row-context"
  | Removed(_) => "row-removed"
  | Added(_) => "row-added"
  | Modified(_) => "row-modified"
  | Expander(_) => "row-expander"
  | WhitespaceOnly(_) => "row-whitespace-only"
  }

/// A cell's text split at every span and changed-range boundary, each
/// piece carrying its span class and whether it is inside a changed range.
type piece = {text: string, class: option<SpanClass.t>, changed: bool}

let pieces = (cell: Cell.t): array<piece> => {
  let len = String.length(cell.text)
  let cuts = [0, len]
  cell.spans->Array.forEach(s => {
    cuts->Array.push(s.range.start)
    cuts->Array.push(s.range.end_)
  })
  cell.changed->Array.forEach(r => {
    cuts->Array.push(r.start)
    cuts->Array.push(r.end_)
  })
  let sorted =
    cuts
    ->Array.filter(c => c >= 0 && c <= len)
    ->Array.toSorted((a, b) => Int.compare(a, b))
  let out = []
  let prev = ref(-1)
  sorted->Array.forEach(c => {
    if c != prev.contents {
      if prev.contents >= 0 && c > prev.contents {
        let start = prev.contents
        let text = String.slice(cell.text, ~start, ~end=c)
        let class =
          cell.spans
          ->Array.find(s => s.range.start <= start && c <= s.range.end_)
          ->Option.map(s => s.class)
        let changed = cell.changed->Array.some(r => r.start <= start && c <= r.end_)
        out->Array.push({text, class, changed})
      }
      prev := c
    }
  })
  out
}

/// The half of a row a cell is: base is the removed (left) column, head
/// the added (right) one. A modified row renders both, and each is its
/// own comment target.
let sideClass = (side: Domain.Side.t): string =>
  switch side {
  | Base => "left"
  | Head => "right"
  }

module CellView = {
  @react.component
  let make = (
    ~cell: Cell.t,
    ~side: Domain.Side.t,
    ~focused: bool=false,
    ~selected: bool=false,
    ~threads: int=0,
    ~onClick: unit => unit=() => (),
    ~onMouseDown: unit => unit=() => (),
    ~onMouseEnter: unit => unit=() => (),
  ) => {
    let className =
      "cell-" ++
      sideClass(side) ++
      (focused ? " cell-focused" : "") ++ (selected ? " cell-selected" : "")
    Attrs.withData(
      <div
        className
        onClick={_ => onClick()}
        onMouseDown={_ => onMouseDown()}
        onMouseEnter={_ => onMouseEnter()}
      >
        <span className="cell-line-no"> {React.string(Int.toString(cell.lineNo))} </span>
        {pieces(cell)
        ->Array.mapWithIndex((p, i) => {
          let cls = switch (p.class, p.changed) {
          | (Some(c), true) => spanClassName(c) ++ " cell-changed"
          | (Some(c), false) => spanClassName(c)
          | (None, true) => "cell-changed"
          | (None, false) => ""
          }
          <span key={Int.toString(i)} className=cls> {React.string(p.text)} </span>
        })
        ->React.array}
        {threads > 0
          ? <span className="cell-threads" title={Int.toString(threads) ++ " thread(s)"}>
              {React.string("💬")}
            </span>
          : React.null}
      </div>,
      [("data-side", Domain.Side.name(side))],
    )
  }
}

let empty = (side: Domain.Side.t) => <div className={"cell-" ++ sideClass(side) ++ " cell-empty"} />

@react.component
let make = (
  ~row: Row.t,
  ~layout: View.Layout.t,
  ~index: int,
  ~focused: bool,
  ~threads: array<View.RowThread.t>,
  ~focusedSide: Domain.Side.t=Head,
  ~selectedSide: option<Domain.Side.t>=?,
  ~onClick: Domain.Side.t => unit=_ => (),
  ~onMouseDown: Domain.Side.t => unit=_ => (),
  ~onMouseEnter: Domain.Side.t => unit=_ => (),
  ~onExpand: unit => unit=() => (),
) => {
  let base = "row " ++ rowClassName(row)
  let className = switch layout {
  | Unified => base ++ " row-unified"
  | Split => base ++ " row-split"
  }
  let threadsOn = (side: Domain.Side.t) =>
    threads->Array.filter((t: View.RowThread.t) => t.side == side)->Array.length
  // One rendered cell standing for both sides: a unified context row has
  // a line on each side but shows only one. Added and removed rows are
  // NOT this — they genuinely have no cell on the other side, so they are
  // not focused or selected when the other side is the target.
  let oneCellBothSides = switch (row, layout) {
  | (Context(_), Unified) => true
  | (Context(_), Split)
  | (Modified(_), Unified | Split)
  | (Added(_), Unified | Split)
  | (Removed(_), Unified | Split)
  | (HunkHeader(_), _)
  | (Expander(_), _)
  | (WhitespaceOnly(_), _) => false
  }
  let onSide = (side: Domain.Side.t, target: Domain.Side.t) => side == target || oneCellBothSides
  let cell = (~cell: Cell.t, ~side: Domain.Side.t) =>
    <CellView
      cell
      side
      focused={focused && onSide(side, focusedSide)}
      selected={switch selectedSide {
      | Some(target) => onSide(side, target)
      | None => false
      }}
      threads={threadsOn(side)}
      onClick={() => onClick(side)}
      onMouseDown={() => onMouseDown(side)}
      onMouseEnter={() => onMouseEnter(side)}
    />
  let body = switch (row, layout) {
  | (HunkHeader({text}), _) => <div className="cell-hunk"> {React.string(text)} </div>
  | (WhitespaceOnly(_), _) =>
    <div className="cell-hunk"> {React.string("whitespace-only changes hidden")} </div>
  | (Expander({hidden, dir}), _) => {
      let arrow = switch dir {
      | Up => "↑"
      | Down => "↓"
      | Both => "↕"
      }
      <div className="cell-hunk" onClick={_ => onExpand()}>
        {React.string(arrow ++ " " ++ Int.toString(hidden) ++ " more lines — expand")}
      </div>
    }
  | (Context({right}), Unified) => cell(~cell=right, ~side=Head)
  | (Context({left, right}), Split) =>
    <>
      {cell(~cell=left, ~side=Base)}
      {cell(~cell=right, ~side=Head)}
    </>
  | (Removed({left}), Unified) => cell(~cell=left, ~side=Base)
  | (Removed({left}), Split) =>
    <>
      {cell(~cell=left, ~side=Base)}
      {empty(Head)}
    </>
  | (Added({right}), Unified) => cell(~cell=right, ~side=Head)
  | (Added({right}), Split) =>
    <>
      {empty(Base)}
      {cell(~cell=right, ~side=Head)}
    </>
  | (Modified({left, right}), Unified | Split) =>
    <>
      {cell(~cell=left, ~side=Base)}
      {cell(~cell=right, ~side=Head)}
    </>
  }
  // A row with no cells (a hunk header) still takes a click, on the side
  // the focus is already on.
  let rowClick = switch row {
  | HunkHeader(_) | Expander(_) | WhitespaceOnly(_) => _ => onClick(focusedSide)
  | Context(_) | Added(_) | Removed(_) | Modified(_) => _ => ()
  }
  Attrs.withData(
    <div className role="row" onClick=rowClick> body </div>,
    focused
      ? [
          ("data-focused", "true"),
          ("data-row-index", Int.toString(index)),
          ("data-side", Domain.Side.name(focusedSide)),
        ]
      : [("data-row-index", Int.toString(index))],
  )
}
