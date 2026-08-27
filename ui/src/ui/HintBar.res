// The hint bar (§6.4): primary bindings of the focused context, straight
// from `ViewModel.hints` — never hand-written.

@react.component
let make = (~hints: array<View.Hint.t>, ~connection: View.ConnectionView.t, ~progress: View.Progress.t) => {
  let conn = switch connection {
  | Disconnected(_) => "disconnected"
  | Connecting(_) => "connecting…"
  | Subscribed(_) => "connected"
  | Rejected(_) => "rejected"
  }
  <footer className="hint-bar" role="contentinfo">
    <span className={"conn conn-" ++ conn}> {React.string(conn)} </span>
    <span className="progress">
      {React.string(
        Int.toString(progress.viewed) ++ "/" ++ Int.toString(progress.total) ++ " viewed",
      )}
    </span>
    {hints
    ->Array.map(h =>
      <span key={h.keys ++ h.label} className="hint">
        <kbd> {React.string(h.keys)} </kbd> {React.string(" " ++ h.label)}
      </span>
    )
    ->React.array}
  </footer>
}
