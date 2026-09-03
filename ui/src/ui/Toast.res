// The toast region (UI-DESIGN §Chrome): one transient line, dismissed
// after a few seconds so a stale clipboard result does not sit on screen
// for the rest of the session.

@val external setTimeout: (unit => unit, int) => int = "setTimeout"
@val external clearTimeout: int => unit = "clearTimeout"

let dismissAfterMs = 3000

@react.component
let make = (~message: option<(string, bool)>) => {
  // The timer restarts for a newer message and is cancelled on unmount.
  let (hidden, setHidden) = React.useState(() => false)
  React.useEffect1(() => {
    setHidden(_ => false)
    switch message {
    | None => None
    | Some(_) =>
      let timer = setTimeout(() => setHidden(_ => true), dismissAfterMs)
      Some(() => clearTimeout(timer))
    }
  }, [message])
  switch message {
  | Some((text, failed)) if !hidden =>
    <div className={"toast" ++ (failed ? " toast-error" : "")} role="status" ariaLive=#polite>
      {React.string(text)}
    </div>
  | Some(_) | None => React.null
  }
}
