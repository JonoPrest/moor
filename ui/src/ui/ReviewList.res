// Reviews of the workspace; Enter/double-click opens one.

open View

@react.component
let make = (~reviews: array<Domain.Review.t>, ~focus: Focus.t, ~dispatch: Action.t => unit) => {
  let focusedIndex = switch focus {
  | ReviewList({index}) => Some(index)
  | _ => None
  }
  <section className="review-list panel">
    <header className="panel-header"> {React.string("Reviews")} </header>
    {Array.length(reviews) == 0
      ? <p className="empty"> {React.string("No reviews yet.")} </p>
      : <ul role="list">
          {reviews
          ->Array.mapWithIndex((r, i) => {
            let status = switch r.status {
            | Open => "open"
            | Archived => "archived"
            }
            Attrs.focused(
              <li
                key=r.id
                className="review-item"
                onClick={_ => dispatch(SetFocus({focus: Focus.ReviewList({index: i})}))}
                onDoubleClick={_ => dispatch(OpenReview({reviewId: r.id}))}>
                <span className="review-title"> {React.string(r.title)} </span>
                <span className={"review-status review-" ++ status}> {React.string(status)} </span>
              </li>,
              focusedIndex == Some(i),
            )
          })
          ->React.array}
        </ul>}
  </section>
}
