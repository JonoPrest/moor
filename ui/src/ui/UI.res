// The design system (after the Envio UI's `UI.res`): every shared primitive
// lives here and takes configuration props (`~kind`, `~tone`, `~gap`), never
// a `className` escape hatch. Tailwind's scanner needs class literals, so
// variants are spelled out rather than interpolated.

module Box = {
  type direction = Row | Column
  type gap = NoGap | Xs | Sm | Md
  @react.component
  let make = (~children, ~direction=Column, ~gap=NoGap, ~grow=false) => {
    let dir = switch direction {
    | Row => "flex flex-row items-center"
    | Column => "flex flex-col"
    }
    let gapClass = switch gap {
    | NoGap => ""
    | Xs => " gap-1"
    | Sm => " gap-2"
    | Md => " gap-4"
    }
    let growClass = grow ? " min-h-0 flex-1" : ""
    <div className={dir ++ gapClass ++ growClass}> children </div>
  }
}

module Panel = {
  /// A bordered region with a header; `grow` fills the remaining height.
  @react.component
  let make = (
    ~title: string,
    ~children,
    ~grow=false,
    ~actions=React.null,
    ~role=?,
    ~ariaLabel=?,
  ) => {
    let className = "panel flex flex-col" ++ (grow ? " min-h-0 flex-1" : "")
    <section className ?role ?ariaLabel>
      <header className="panel-header">
        {React.string(title)}
        actions
      </header>
      children
    </section>
  }
}

module Button = {
  type kind = Primary | Secondary | Ghost
  @react.component
  let make = (~label: string, ~onClick: unit => unit, ~kind=Secondary, ~title=?) => {
    let className = switch kind {
    | Primary => "btn btn-primary"
    | Secondary => "btn"
    | Ghost => "btn btn-ghost"
    }
    // Explicit type: inside a <form> a bare <button> would submit it.
    <button type_="button" className ?title onClick={_ => onClick()}>
      {React.string(label)}
    </button>
  }
}

module Kbd = {
  /// `space` renders as the ␣ glyph everywhere a key is shown.
  @react.component
  let make = (~keys: string) => {
    let text =
      keys
      ->String.split(" ")
      ->Array.map(tok => tok == "space" ? "␣" : tok)
      ->Array.join(" ")
    <kbd> {React.string(text)} </kbd>
  }
}

module Empty = {
  @react.component
  let make = (~text: string) => <p className="empty"> {React.string(text)} </p>
}

module Badge = {
  type tone = Neutral | Add | Remove | Accent
  @react.component
  let make = (~text: string, ~tone=Neutral) => {
    let className = switch tone {
    | Neutral => "badge"
    | Add => "badge badge-add"
    | Remove => "badge badge-remove"
    | Accent => "badge badge-accent"
    }
    <span className> {React.string(text)} </span>
  }
}

module Select = {
  /// A native select over `(value, label)` options.
  @react.component
  let make = (
    ~value: string,
    ~options: array<(string, string)>,
    ~onChange: string => unit,
    ~ariaLabel=?,
  ) =>
    <select
      className="text-input"
      value
      ?ariaLabel
      onChange={ev => onChange(ReactEvent.Form.target(ev)["value"])}
    >
      {options
      ->Array.map(((v, label)) => <option key=v value=v> {React.string(label)} </option>)
      ->React.array}
    </select>
}

module TextInput = {
  /// A text field whose keys never reach the keymap; `onKey` sees named
  /// keys (Enter, Escape) first.
  @react.component
  let make = (
    ~value: string,
    ~onChange: string => unit,
    ~placeholder: string,
    ~autoFocus=false,
    ~onKey: string => unit=_ => (),
    ~preventKeys: array<string>=[],
  ) =>
    <input
      className="text-input"
      autoFocus
      placeholder
      value
      onChange={ev => onChange(ReactEvent.Form.target(ev)["value"])}
      onKeyDown={ev => {
        let key = ReactEvent.Keyboard.key(ev)
        if preventKeys->Array.includes(key) {
          ReactEvent.Keyboard.preventDefault(ev)
        }
        onKey(key)
        ReactEvent.Keyboard.stopPropagation(ev)
      }}
    />
}
