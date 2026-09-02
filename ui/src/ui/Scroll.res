// Scrolling follows the focus (§6.4): the core says which row is focused
// and, for `z z`/`z t`/`z b`, where the view should sit around it. This
// module is only the arithmetic and the DOM call that carry that out —
// no decisions of its own.

/// A vertical extent in viewport coordinates.
type box = {top: float, bottom: float, height: float}

type mode =
  /// Keep the row on screen, moving as little as possible and leaving a
  /// margin of context past it (vim's `scrolloff`): what a motion does.
  | Nearest
  /// Put the row where the reader asked for it.
  | Align(View.ScrollAlign.t)

/// How far `container` must scroll for `row` to sit as `mode` says;
/// positive scrolls down. `headroom` is the height of anything painted
/// over the top of the container (the sticky file header), and `margin`
/// the context to keep past the row.
let delta = (~container: box, ~row: box, ~headroom: float, ~margin: float, ~mode: mode): float => {
  let top = container.top +. headroom
  let bottom = container.bottom
  switch mode {
  | Align(Top) => row.top -. top
  | Align(Bottom) => row.bottom -. bottom
  | Align(Center) => row.top -. (top +. (bottom -. top -. row.height) /. 2.)
  | Nearest => {
      // A viewport barely taller than the row has no room for a margin;
      // showing the row at all beats showing context around it.
      let m = Math.min(margin, Math.max(0., (bottom -. top -. row.height) /. 2.))
      if row.top < top +. m {
        row.top -. (top +. m)
      } else if row.bottom > bottom -. m {
        row.bottom -. (bottom -. m)
      } else {
        0.
      }
    }
  }
}

/// Rows of context kept past the cursor (vim's default is 0; a few rows
/// is the setting people actually use).
let scrolloff = 3.

type element = Dom.element

@get external scrollTop: element => float = "scrollTop"
@set external setScrollTop: (element, float) => unit = "scrollTop"
@get external clientHeight: element => float = "clientHeight"
@get external scrollHeight: element => float = "scrollHeight"
@get external parentElement: element => Nullable.t<element> = "parentElement"
@send
external getBoundingClientRect: element => {"top": float, "bottom": float, "height": float} =
  "getBoundingClientRect"
@val @scope("document") external querySelector: string => Nullable.t<element> = "querySelector"
@send external closest: (element, string) => Nullable.t<element> = "closest"
@send external querySelectorIn: (element, string) => Nullable.t<element> = "querySelector"

let boxOf = (el: element): box => {
  let r = getBoundingClientRect(el)
  {top: r["top"], bottom: r["bottom"], height: r["height"]}
}

/// The nearest ancestor that actually scrolls.
let rec scroller = (el: element): option<element> =>
  switch parentElement(el)->Nullable.toOption {
  | None => None
  | Some(p) =>
    if scrollHeight(p) > clientHeight(p) +. 1. {
      Some(p)
    } else {
      scroller(p)
    }
  }

/// A file section's header is sticky, so it covers the top of the
/// scroller: a row put flush with the edge would hide behind it.
let headroomOf = (el: element): float =>
  switch closest(el, ".file-diff")->Nullable.toOption {
  | None => 0.
  | Some(section) =>
    switch querySelectorIn(section, ".file-diff-header")->Nullable.toOption {
    | Some(header) => boxOf(header).height
    | None => 0.
    }
  }

/// Scroll the focused element as `mode` says. A no-op when nothing is
/// focused, when it is not inside a scroller, or when it is already where
/// it belongs.
let apply = (mode: mode) =>
  switch querySelector("[data-focused]")->Nullable.toOption {
  | None => ()
  | Some(el) =>
    switch scroller(el) {
    | None => ()
    | Some(container) => {
        let row = boxOf(el)
        let d = delta(
          ~container=boxOf(container),
          ~row,
          ~headroom=headroomOf(el),
          ~margin=scrolloff *. row.height,
          ~mode,
        )
        if d != 0. {
          setScrollTop(container, scrollTop(container) +. d)
        }
      }
    }
  }
