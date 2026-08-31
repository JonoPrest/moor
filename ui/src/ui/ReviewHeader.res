// What the open review diffs, like a compare header: `base → head` per
// target (prefixed by the repo name when the review spans several). A
// working-tree head shows the checked-out branch from the resolved
// targets when the daemon has reported one.

@react.component
let make = (
  ~reviews: array<Domain.Review.t>,
  ~workspaces: array<Domain.Workspace.t>,
  ~resolvedTargets: array<Domain.ResolvedTarget.t>,
  ~openReview: option<Ids.reviewId>,
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
      <header className="review-header" ariaLabel="review targets">
        <span className="review-header-title"> {React.string(review.title)} </span>
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
      </header>
    }
  }
