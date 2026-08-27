// Browser adapter stub: the wasm build of `moor-client-core` (plan "Later").
// Until it exists the UI shows the empty model and refuses actions loudly,
// so nothing silently pretends to work.

let make = (~onError: string => unit=e => Console.error(e)): Core.t => {
  let store = Core.Store.make()
  {
    dispatch: _ => onError("CoreWasm: not implemented (see PLAN 'Later')"),
    subscribe: listener => Core.Store.subscribe(store, listener),
    attach: () => (),
  }
}
