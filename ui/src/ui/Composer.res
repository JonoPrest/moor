// The comment editor (§5.3): its text never enters the core; only
// `DraftSubmitted { body }` / `DraftDiscarded` cross. Ctrl/Cmd+Enter
// submits, Esc discards.

@react.component
let make = (~draft: View.Draft.t, ~pendingRefresh: bool, ~dispatch: Action.t => unit) => {
  let (body, setBody) = React.useState(() => "")
  let submit = () =>
    if String.trim(body) != "" {
      dispatch(DraftSubmitted({body: body}))
    }
  let onKeyDown = (ev: ReactEvent.Keyboard.t) => {
    let key = ReactEvent.Keyboard.key(ev)
    let submitChord =
      key == "Enter" && (ReactEvent.Keyboard.ctrlKey(ev) || ReactEvent.Keyboard.metaKey(ev))
    if submitChord {
      ReactEvent.Keyboard.preventDefault(ev)
      submit()
    } else if key == "Escape" {
      ReactEvent.Keyboard.preventDefault(ev)
      dispatch(DraftDiscarded({}))
    }
    // Everything else is text; never forwarded to the keymap.
    ReactEvent.Keyboard.stopPropagation(ev)
  }
  let placeholder = switch draft.replyTo {
  | Some(_) => "Reply…"
  | None => "Comment…"
  }
  <div className="composer panel">
    {pendingRefresh
      ? <div className="composer-pending"> {React.string("changes pending")} </div>
      : React.null}
    <textarea
      className="composer-input"
      autoFocus=true
      placeholder
      value=body
      onChange={ev => setBody(_ => ReactEvent.Form.target(ev)["value"])}
      onKeyDown
    />
    <div className="composer-actions">
      <button className="btn" onClick={_ => submit()}> {React.string("Submit ⌘⏎")} </button>
      <button className="btn" onClick={_ => dispatch(DraftDiscarded({}))}>
        {React.string("Discard ⎋")}
      </button>
    </div>
  </div>
}
