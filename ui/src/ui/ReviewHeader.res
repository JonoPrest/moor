// What the open review diffs, like a compare header (UI-DESIGN §Layout):
// title · scope control · `base → head` chips · layout + whitespace
// toggles · connection dot. A working-tree head shows the checked-out
// branch from the resolved targets when the daemon has reported one.
// Every control's tooltip comes from the keymap chrome, never hand-written.

module Seg = {
  /// One segment of a joined segmented control.
  @react.component
  let make = (~label: string, ~active: bool, ~title: option<string>, ~onClick: unit => unit) => {
    let el =
      <button
        type_="button"
        className="seg"
        ?title
        ariaPressed={active ? #"true" : #"false"}
        onClick={_ => onClick()}>
        {React.string(label)}
      </button>
    active ? Attrs.withData(el, [("data-active", "true")]) : el
  }
}

module Toggle = {
  @react.component
  let make = (~label: string, ~active: bool, ~title: option<string>, ~onClick: unit => unit) => {
    let el =
      <button
        type_="button"
        className="btn toggle"
        ?title
        ariaPressed={active ? #"true" : #"false"}
        onClick={_ => onClick()}>
        {React.string(label)}
      </button>
    active ? Attrs.withData(el, [("data-active", "true")]) : el
  }
}

// The scope control (UI-DESIGN §Diff scope): All changes (± working
// tree) vs By commit, with the step position while stepping.
module ScopeControl = {
  @react.component
  let make = (
    ~scope: Domain.DiffScope.t,
    ~stepper: option<View.CommitStepper.t>,
    ~chrome: array<View.Hint.t>,
    ~dispatch: Action.t => unit,
  ) => {
    let allish = switch scope {
    | All(_) | Committed(_) => true
    | Commit(_) | Worktree(_) => false
    }
    let position = switch (scope, stepper) {
    | (Worktree(_), _) => Some("worktree")
    | (Commit({oid}), Some(s)) =>
      s.commits
      ->Array.findIndexOpt(c => c.oid == oid)
      ->Option.map(i =>
        Int.toString(Array.length(s.commits) - i) ++ " of " ++ Int.toString(Array.length(s.commits))
      )
    | (Commit(_), None) | (All(_), _) | (Committed(_), _) => None
    }
    <>
      <span className="segmented" role="group" ariaLabel="diff scope">
        <Seg
          label="All changes"
          active=allish
          title={Chrome.tip(chrome, ScopeAll)}
          onClick={() => dispatch(SetScope({scope: All({})}))}
        />
        <Seg
          label="By commit"
          active={!allish}
          title={Chrome.tip(chrome, ScopeByCommit)}
          onClick={() => dispatch(SetScope({scope: ByCommit({})}))}
        />
      </span>
      {allish
        ? <Toggle
            label="+ working tree"
            active={scope == All({})}
            title={Chrome.tip(chrome, ScopeWorktree)}
            onClick={() => dispatch(SetScope({scope: scope == All({}) ? Committed({}) : All({})}))}
          />
        : React.null}
      {switch position {
      | Some(text) => <span className="scope-position"> {React.string(text)} </span>
      | None => React.null
      }}
    </>
  }
}

@react.component
let make = (
  ~reviews: array<Domain.Review.t>,
  ~workspaces: array<Domain.Workspace.t>,
  ~resolvedTargets: array<Domain.ResolvedTarget.t>,
  ~openReview: option<Ids.reviewId>,
  ~prefs: View.ViewPrefs.t,
  ~scope: Domain.DiffScope.t=Domain.DiffScope.All({}),
  ~stepper: option<View.CommitStepper.t>=?,
  ~chrome: array<View.Hint.t>=[],
  ~connection: View.ConnectionView.t=View.ConnectionView.Disconnected({}),
  ~dispatch: Action.t => unit=_ => (),
) =>
  switch openReview->Option.flatMap(id => reviews->Array.find(r => r.id == id)) {
  | None => React.null
  | Some(review) => {
      let repoName = (id: Ids.repoId) =>
        workspaces
        ->Array.findMap(ws => ws.repos->Array.find(r => r.id == id))
        ->Option.mapOr(id, r => r.displayName)
      let headText = (t: Domain.ReviewTarget.t) =>
        switch t.head {
        | WorkingTree(_) =>
          switch resolvedTargets->Array.find(r => r.repoId == t.repoId) {
          | Some({head: {source: WorkingTree({branch: Some(branch)})}}) => branch ++ " (worktree)"
          | Some(_) | None => RefSpecText.print(t.head)
          }
        | _ => RefSpecText.print(t.head)
        }
      let many = Array.length(review.targets) > 1
      let conn = switch connection {
      | Disconnected(_) => "disconnected"
      | Connecting(_) => "connecting"
      | Subscribed(_) => "connected"
      | Rejected(_) => "rejected"
      }
      <header className="review-header" ariaLabel="review targets">
        <span className="review-header-title"> {React.string(review.title)} </span>
        <ScopeControl scope stepper chrome dispatch />
        {review.targets
        ->Array.map(t =>
          <span key=t.repoId className="review-header-target">
            {many
              ? <span className="review-header-repo">
                  {React.string(repoName(t.repoId) ++ ":")}
                </span>
              : React.null}
            <span className="review-header-ref"> {React.string(RefSpecText.print(t.base))} </span>
            <span className="review-header-arrow" ariaHidden=true> {React.string("→")} </span>
            <span className="review-header-ref"> {React.string(headText(t))} </span>
          </span>
        )
        ->React.array}
        <span className="review-header-toggles">
          <span className="segmented" role="group" ariaLabel="diff layout">
            <Seg
              label="Unified"
              active={prefs.layout == Unified}
              title={Chrome.tip(chrome, ToggleLayout)}
              onClick={() => dispatch(SetLayout({layout: Unified}))}
            />
            <Seg
              label="Split"
              active={prefs.layout == Split}
              title={Chrome.tip(chrome, ToggleLayout)}
              onClick={() => dispatch(SetLayout({layout: Split}))}
            />
          </span>
          <Toggle
            label="hide whitespace"
            active=prefs.ignoreWhitespace
            title={Chrome.tip(chrome, ToggleWhitespace)}
            onClick={() =>
              dispatch(
                SetRenderOpts({
                  ignoreWhitespace: !prefs.ignoreWhitespace,
                  contextLines: prefs.contextLines,
                }),
              )}
          />
          <span className={"conn-dot conn-" ++ conn} title=conn />
        </span>
      </header>
    }
  }
