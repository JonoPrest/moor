// Commit stepper with the commit panel for the selected commit: subject,
// body, author and committer with relative and absolute times, parents.

open View

let relative = (t: Ids.timestamp): string => {
  let seconds = (Date.now() -. t) /. 1000.
  let n = seconds->Float.toInt
  if n < 60 {
    "just now"
  } else if n < 3600 {
    Int.toString(n / 60) ++ " min ago"
  } else if n < 86400 {
    Int.toString(n / 3600) ++ " h ago"
  } else {
    Int.toString(n / 86400) ++ " d ago"
  }
}

let absolute = (t: Ids.timestamp): string => Date.fromTime(t)->Date.toISOString

module CommitPanel = {
  @react.component
  let make = (~commit: StepperCommit.t) =>
    <div className="commit-panel">
      <div className="commit-subject"> {React.string(commit.subject)} </div>
      {commit.body == ""
        ? React.null
        : <pre className="commit-body"> {React.string(commit.body)} </pre>}
      <dl className="commit-meta">
        <dt> {React.string("author")} </dt>
        <dd title={absolute(commit.time)}>
          {React.string(commit.author ++ ", " ++ relative(commit.time))}
        </dd>
        <dt> {React.string("committer")} </dt>
        <dd title={absolute(commit.committerTime)}>
          {React.string(commit.committer ++ ", " ++ relative(commit.committerTime))}
        </dd>
        <dt> {React.string("parents")} </dt>
        <dd>
          {commit.parents
          ->Array.map(p =>
            <span key=p className="commit-oid">
              {React.string(String.slice(p, ~start=0, ~end=8))}
            </span>
          )
          ->React.array}
        </dd>
      </dl>
    </div>
}

@react.component
let make = (~stepper: CommitStepper.t, ~focus: Focus.t, ~dispatch: Action.t => unit) => {
  let focusedIndex = switch focus {
  | CommitStepper({index}) => Some(index)
  | _ => None
  }
  let selected = stepper.selected->Option.flatMap(i => stepper.commits[i])
  <UI.Panel title="Commits">
    <ol role="list">
      {stepper.commits
      ->Array.mapWithIndex((c, i) => {
        let isSelected = stepper.selected == Some(i)
        Attrs.focused(
          <li
            key=c.oid
            className={"stepper-commit" ++ (isSelected ? " stepper-selected" : "")}
            onClick={_ => {
              dispatch(SetFocus({focus: Focus.CommitStepper({index: i})}))
              dispatch(SetScope({scope: Commit({repoId: stepper.repoId, oid: c.oid})}))
            }}
          >
            <span className="commit-oid">
              {React.string(String.slice(c.oid, ~start=0, ~end=8))}
            </span>
            <span className="commit-subject"> {React.string(c.subject)} </span>
            <span className="commit-author"> {React.string(c.author)} </span>
          </li>,
          focusedIndex == Some(i),
        )
      })
      ->React.array}
    </ol>
    {switch selected {
    | Some(commit) => <CommitPanel commit />
    | None => React.null
    }}
  </UI.Panel>
}
