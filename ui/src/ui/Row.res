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

module CellView = {
  @react.component
  let make = (~cell: Cell.t, ~side: string) => {
    <div className={"cell-" ++ side}>
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
    </div>
  }
}

let empty = (side: string) => <div className={"cell-" ++ side ++ " cell-empty"} />

@react.component
let make = (
  ~row: Row.t,
  ~layout: View.Layout.t,
  ~index: int,
  ~focused: bool,
  ~threads: int,
  ~onClick: unit => unit=() => (),
  ~onExpand: unit => unit=() => (),
) => {
  let base = "row " ++ rowClassName(row)
  let className = switch layout {
  | Unified => base ++ " row-unified"
  | Split => base ++ " row-split"
  }
  let marker = if threads > 0 {
    <span className="row-threads" title={Int.toString(threads) ++ " thread(s)"}>
      {React.string("💬")}
    </span>
  } else {
    React.null
  }
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
  | (Context({right}), Unified) => <CellView cell=right side="right" />
  | (Context({left, right}), Split) =>
    <> <CellView cell=left side="left" /> <CellView cell=right side="right" /> </>
  | (Removed({left}), Unified) => <CellView cell=left side="left" />
  | (Removed({left}), Split) => <> <CellView cell=left side="left" /> {empty("right")} </>
  | (Added({right}), Unified) => <CellView cell=right side="right" />
  | (Added({right}), Split) => <> {empty("left")} <CellView cell=right side="right" /> </>
  | (Modified({left, right}), Unified | Split) =>
    <> <CellView cell=left side="left" /> <CellView cell=right side="right" /> </>
  }
  Attrs.withData(
    <div className role="row" onClick={_ => onClick()}> body marker </div>,
    focused
      ? [("data-focused", "true"), ("data-row-index", Int.toString(index))]
      : [("data-row-index", Int.toString(index))],
  )
}
