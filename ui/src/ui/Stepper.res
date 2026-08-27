// Commit stepper: the review's commits with a cursor.

open View

@react.component
let make = (~stepper: CommitStepper.t, ~focus: Focus.t, ~dispatch: Action.t => unit) => {
  let focusedIndex = switch focus {
  | CommitStepper({index}) => Some(index)
  | _ => None
  }
  <section className="stepper panel">
    <header className="panel-header"> {React.string("Commits")} </header>
    <ol role="list">
      {stepper.commits
      ->Array.mapWithIndex((c, i) => {
        let selected = stepper.selected == Some(i)
        Attrs.focused(
          <li
            key=c.oid
            className={"stepper-commit" ++ (selected ? " stepper-selected" : "")}
            onClick={_ => dispatch(SetFocus({focus: Focus.CommitStepper({index: i})}))}
            onDoubleClick={_ => dispatch(StepCommit({selected: Some(i)}))}>
            <span className="commit-oid"> {React.string(String.slice(c.oid, ~start=0, ~end=8))} </span>
            <span className="commit-subject"> {React.string(c.subject)} </span>
            <span className="commit-author"> {React.string(c.author)} </span>
          </li>,
          focusedIndex == Some(i),
        )
      })
      ->React.array}
    </ol>
  </section>
}
