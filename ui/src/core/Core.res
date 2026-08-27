// The UI's only door to the core (ARCHITECTURE §6.2): dispatch actions,
// receive the view. Adapters implement `t`; components never see IPC.

type unsubscribe = unit => unit

type t = {
  /// Send a user intent to the core.
  dispatch: Action.t => unit,
  /// Send a key chord (outside text inputs) for the core's keymap.
  key: Keys.keyChord => unit,
  /// Called with the current model now and after every patch.
  subscribe: (View.viewModel => unit) => unsubscribe,
  /// Ask the host for every section (a UI that just attached).
  attach: unit => unit,
}

/// The UI-side copy of the model: applies patches, fans out changes.
module Store = {
  type t = {
    mutable model: View.viewModel,
    mutable listeners: array<View.viewModel => unit>,
  }

  let make = (): t => {model: View.empty, listeners: []}

  let apply = (store: t, patches: array<View.ViewPatch.t>) => {
    store.model = patches->Array.reduce(store.model, View.ViewPatch.apply)
    store.listeners->Array.forEach(l => l(store.model))
  }

  let subscribe = (store: t, listener: View.viewModel => unit): unsubscribe => {
    store.listeners = store.listeners->Array.concat([listener])
    listener(store.model)
    () => {
      store.listeners = store.listeners->Array.filter(l => l !== listener)
    }
  }
}

// JSON crossings, so adapters and tests never touch Sury directly.
let actionToJson = (action: Action.t): JSON.t =>
  S.reverseConvertToJsonOrThrow(action, Action.schema)

let actionOfJson = (json: JSON.t): result<Action.t, string> =>
  try {
    Ok(S.parseJsonOrThrow(json, Action.schema))
  } catch {
  | Exn.Error(e) => Error(Exn.message(e)->Option.getOr("invalid action"))
  | _ => Error("invalid action")
  }

let patchesSchema = S.array(View.ViewPatch.schema)

let patchesOfJson = (json: JSON.t): result<array<View.ViewPatch.t>, string> =>
  try {
    Ok(S.parseJsonOrThrow(json, patchesSchema))
  } catch {
  | Exn.Error(e) => Error(Exn.message(e)->Option.getOr("invalid patches"))
  | _ => Error("invalid patches")
  }

let modelToJson = (model: View.viewModel): JSON.t =>
  S.reverseConvertToJsonOrThrow(model, View.viewModel)
