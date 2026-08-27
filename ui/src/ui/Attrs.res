// `data-*` attributes are not in ReScript's DOM props; add them by cloning
// (§6.6: focus is styled through `data-focused`, never `:focus`).

let withData = (el: React.element, attrs: array<(string, string)>): React.element =>
  React.cloneElement(el, Dict.fromArray(attrs))

let focused = (el: React.element, is: bool): React.element =>
  is ? withData(el, [("data-focused", "true")]) : el
