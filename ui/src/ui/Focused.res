// Scroll the element carrying `data-focused` into view (§6.4: focus is
// core state, the DOM follows it).

type element
@val @scope("document") external querySelector: string => Nullable.t<element> = "querySelector"
@send external scrollIntoView: (element, {"block": string}) => unit = "scrollIntoView"

let scrollIntoView = () =>
  switch querySelector("[data-focused]")->Nullable.toOption {
  | Some(el) => el->scrollIntoView({"block": "nearest"})
  | None => ()
  }
