// The hint bar (§6.4, UI-DESIGN): the bar is the mode indicator. It shows
// the focused context's primary bindings straight from `ViewModel.hints`,
// and switches to the pending group's keys while a leader is held
// (zellij-style) — never hand-written.

@react.component
let make = (
  ~hints: array<View.Hint.t>,
  ~pendingKeys: string="",
  ~connection: View.ConnectionView.t,
  ~progress: View.Progress.t,
) => {
  let conn = switch connection {
  | Disconnected(_) => "disconnected"
  | Connecting(_) => "connecting…"
  | Subscribed(_) => "connected"
  | Rejected(_) => "rejected"
  }
  let pending = pendingKeys != ""
  <footer className={"hint-bar" ++ (pending ? " hint-bar-pending" : "")} role="contentinfo">
    <span className={"conn conn-" ++ conn}> {React.string(conn)} </span>
    <span className="progress">
      {React.string(
        Int.toString(progress.viewed) ++ "/" ++ Int.toString(progress.total) ++ " viewed",
      )}
    </span>
    {pending
      ? <span className="pending-keys">
          <UI.Kbd keys=pendingKeys />
        </span>
      : React.null}
    {hints
    ->Array.map(h =>
      <span key={h.keys ++ h.label} className="hint">
        <UI.Kbd keys=h.keys /> {React.string(" " ++ h.label)}
      </span>
    )
    ->React.array}
  </footer>
}
