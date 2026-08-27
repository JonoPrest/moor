// Reviews of the workspace; Enter/double-click opens one.

open View

@react.component
let make = (~reviews: array<Domain.Review.t>, ~focus: Focus.t, ~dispatch: Action.t => unit) => {
  let focusedIndex = switch focus {
  | ReviewList({index}) => Some(index)
  | _ => None
  }
  <UI.Panel title="Reviews">
    {Array.length(reviews) == 0
      ? <UI.Empty text="No reviews yet." />
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
                <UI.Badge text=status tone={r.status == Open ? Accent : Neutral} />
              </li>,
              focusedIndex == Some(i),
            )
          })
          ->React.array}
        </ul>}
  </UI.Panel>
}
