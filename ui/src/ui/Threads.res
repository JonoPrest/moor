// Thread list and the review conversation (§6.5).

open View

let authorName = (a: Domain.Author.t) =>
  switch a {
  | Human({name}) => name
  | Agent({name}) => name ++ " (agent)"
  | Daemon(_) => "moord"
  }

let placeText = (p: ThreadPlace.t) =>
  switch p {
  | Review(_) => "review"
  | File({file}) => file.path
  | Lines({file, start, end_}) =>
    file.path ++ ":" ++ Int.toString(start) ++ (end_ > start ? "-" ++ Int.toString(end_) : "")
  }

module Item = {
  @react.component
  let make = (
    ~thread: ThreadView.t,
    ~focused: bool,
    ~onSelect: unit => unit,
    ~onApply: unit => unit,
  ) => {
    let flags = [
      thread.resolved ? "resolved" : "",
      thread.outdated ? "outdated" : "",
      thread.pending ? "pending" : "",
    ]->Array.filter(s => s != "")
    Attrs.focused(
      <li className={"thread-item " ++ flags->Array.join(" ")} onClick={_ => onSelect()}>
        <div className="thread-meta">
          <span className="thread-author"> {React.string(authorName(thread.author))} </span>
          <span className="thread-place"> {React.string(placeText(thread.place))} </span>
          {thread.replies > 0
            ? <UI.Badge text={Int.toString(thread.replies) ++ " replies"} />
            : React.null}
          {thread.pending ? <span className="thread-pending"> {React.string("…")} </span> : React.null}
        </div>
        <div className="thread-summary"> {React.string(thread.summary)} </div>
        {thread.suggestion
          ? <UI.Button label="Apply suggestion (a)" kind=Primary onClick=onApply />
          : React.null}
      </li>,
      focused,
    )
  }
}

@react.component
let make = (
  ~title: string,
  ~threads: array<ThreadView.t>,
  ~focus: Focus.t,
  ~indexOffset: int,
  ~dispatch: Action.t => unit,
) => {
  let focusedIndex = switch focus {
  | Thread({index}) => Some(index)
  | _ => None
  }
  <UI.Panel title>
    {Array.length(threads) == 0
      ? <UI.Empty text="No threads." />
      : <ul role="list">
          {threads
          ->Array.mapWithIndex((t, i) =>
            <Item
              key=t.id
              thread=t
              focused={focusedIndex == Some(indexOffset + i)}
              onSelect={() => dispatch(SetFocus({focus: Focus.Thread({index: indexOffset + i})}))}
              onApply={() => dispatch(ApplySuggestion({commentId: t.root}))}
            />
          )
          ->React.array}
        </ul>}
  </UI.Panel>
}
