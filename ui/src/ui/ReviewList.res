// Workspaces as groups (like a projects sidebar), each with its reviews
// beneath and a `+` that opens the create form for that workspace.
// Enter/double-click opens a review; the focus index is the position in
// the flat `reviews` array, which is what the keymap uses.

open View

@react.component
let make = (
  ~reviews: array<Domain.Review.t>,
  ~workspaces: array<Domain.Workspace.t>,
  ~connection: ConnectionView.t,
  ~focus: Focus.t,
  ~dispatch: Action.t => unit,
) => {
  let focusedIndex = switch focus {
  | ReviewList({index}) => Some(index)
  | _ => None
  }
  let (creating, setCreating) = React.useState(() => None)
  let conn = switch connection {
  | Disconnected(_) => "disconnected"
  | Connecting(_) => "connecting"
  | Subscribed(_) => "connected"
  | Rejected(_) => "rejected"
  }
  let item = (r: Domain.Review.t, i) => {
    let status = switch r.status {
    | Open => "open"
    | Archived => "archived"
    }
    Attrs.focused(
      <li
        key=r.id
        className="review-item"
        onClick={_ => {
          dispatch(SetFocus({focus: Focus.ReviewList({index: i})}))
          dispatch(OpenReview({reviewId: r.id}))
        }}>
        <span className="review-title"> {React.string(r.title)} </span>
        {r.status == Open ? React.null : <UI.Badge text=status />}
      </li>,
      focusedIndex == Some(i),
    )
  }
  <UI.Panel
    title="Workspaces"
    actions={<UI.Button label="refresh (R)" kind=Ghost onClick={() => dispatch(ListWorkspaces({}))} />}>
    {Array.length(workspaces) == 0
      ? <UI.Empty
          text="No workspaces. Run `nits workspace add <name>` then `nits workspace attach <id> [path]`, and refresh (R)."
        />
      : workspaces
        ->Array.map(ws => {
          let mine = reviews->Array.mapWithIndex((r, i) => (r, i))->Array.filter(((r, _)) => r.workspaceId == ws.id)
          <section key=ws.id className="workspace-group" ariaLabel=ws.name>
            <header className="workspace-header">
              <span className="workspace-glyph" ariaHidden=true> {React.string("▸")} </span>
              <span className="workspace-name"> {React.string(ws.name)} </span>
              <span className={"conn-dot conn-" ++ conn} title=conn />
              <UI.Button
                label="+"
                kind=Ghost
                title={"New review in " ++ ws.name}
                onClick={() => setCreating(_ => Some(ws.id))}
              />
            </header>
            {creating == Some(ws.id)
              ? <NewReview workspaces=[ws] onClose={() => setCreating(_ => None)} dispatch />
              : React.null}
            {Array.length(mine) == 0
              ? <UI.Empty text="No reviews." />
              : <ul role="list"> {mine->Array.map(((r, i)) => item(r, i))->React.array} </ul>}
          </section>
        })
        ->React.array}
  </UI.Panel>
}
