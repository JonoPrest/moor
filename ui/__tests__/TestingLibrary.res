// @testing-library/react bindings, just what the component tests use.

type element = Dom.element
type renderResult = {container: element, rerender: React.element => unit}

@module("@testing-library/react") external render: React.element => renderResult = "render"
@module("@testing-library/react") external cleanup: unit => unit = "cleanup"

module Screen = {
  @module("@testing-library/react") @scope("screen")
  external getByText: string => element = "getByText"
  @module("@testing-library/react") @scope("screen")
  external getByPlaceholderText: string => element = "getByPlaceholderText"
  @module("@testing-library/react") @scope("screen")
  external queryAllByText: string => array<element> = "queryAllByText"
  @module("@testing-library/react") @scope("screen")
  external getByLabelText: string => element = "getByLabelText"
  @module("@testing-library/react") @scope("screen")
  external getByTextRe: RegExp.t => element = "getByText"
}

module Document = {
  @val @scope("document") external activeElement: Nullable.t<element> = "activeElement"
  @val @scope("document") external body: element = "body"
}

module FireEvent = {
  @module("@testing-library/react") @scope("fireEvent")
  external click: element => unit = "click"
  @module("@testing-library/react") @scope("fireEvent")
  external keyDown: (element, {"key": string, "ctrlKey": bool}) => unit = "keyDown"
  @module("@testing-library/react") @scope("fireEvent")
  external keyDownWithShift: (element, {"key": string, "ctrlKey": bool, "shiftKey": bool}) => unit =
    "keyDown"
  @module("@testing-library/react") @scope("fireEvent")
  external change: (element, {"target": {"value": string}}) => unit = "change"
  @module("@testing-library/react") @scope("fireEvent")
  external mouseDown: element => unit = "mouseDown"
  @module("@testing-library/react") @scope("fireEvent")
  external mouseEnter: element => unit = "mouseEnter"
  @module("@testing-library/react") @scope("fireEvent")
  external mouseUp: element => unit = "mouseUp"
}

module Element = {
  @send external querySelector: (element, string) => Nullable.t<element> = "querySelector"
  @send external querySelectorAll: (element, string) => array<element> = "querySelectorAll"
  @get external className: element => string = "className"
  @get external textContent: element => string = "textContent"
  @send external getAttribute: (element, string) => Nullable.t<string> = "getAttribute"
  @send external hasAttribute: (element, string) => bool = "hasAttribute"
  @get external parentElement: element => element = "parentElement"
  @get external scrollTop: element => int = "scrollTop"
  @set external setScrollTop: (element, int) => unit = "scrollTop"
  @get external value: element => string = "value"
  @get external innerHTML: element => string = "innerHTML"
  @send external focus: element => unit = "focus"
}
