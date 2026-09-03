// The clipboard is the shell's. Two things make writing to it awkward,
// and both have to be visible to the caller rather than swallowed:
// `navigator.clipboard` is absent outside a secure context, and a write
// needs transient user activation, which is gone by the time anything has
// crossed the core and come back.

type nav
@val external navigator: nav = "navigator"
@get external clipboard: nav => Nullable.t<{"writeText": string => promise<unit>}> = "clipboard"

/// Why a copy did not happen, so the reader is told the truth.
type failure =
  | /// No clipboard API at all (an insecure context, mostly).
  Unavailable
  | /// The browser refused: no user activation, or permission denied.
  Refused

/// Write `text`, without throwing: an absent clipboard is a failure like
/// any other, not an exception in an effect.
let write = async (text: string): result<unit, failure> =>
  switch clipboard(navigator)->Nullable.toOption {
  | None => Error(Unavailable)
  | Some(api) =>
    try {
      await api["writeText"](text)
      Ok()
    } catch {
    | _ => Error(Refused)
    }
  }

/// What to tell the reader about `outcome` for `path`.
let message = (path: string, outcome: result<unit, failure>): string =>
  switch outcome {
  | Ok() => "Copied " ++ path
  | Error(Unavailable) => "Cannot copy — this browser has no clipboard here"
  | Error(Refused) => "Could not copy — the clipboard refused"
  }

/// A writer that keeps only its newest result: a slow write that settles
/// after a later one must not overwrite what the reader is now looking
/// at. `show` is called with the message and whether it is a failure.
type writer = (string, (string, bool) => unit) => unit

let latest = (): writer => {
  let seq = ref(0)
  (path, show) => {
    seq := seq.contents + 1
    let mine = seq.contents
    write(path)
    ->Promise.thenResolve(outcome =>
      if seq.contents == mine {
        show(message(path, outcome), outcome != Ok())
      }
    )
    ->Promise.done
  }
}
