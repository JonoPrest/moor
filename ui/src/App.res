// The UI is a renderer over `ViewModel` and a source of `Action`s
// (ARCHITECTURE §6.1). 4.0 is the scaffold: the adapters (4.2) and the
// Sury schemas (4.1) land next; until then this only proves the toolchain.

@react.component
let make = () => {
  <main className="app-shell">
    <header className="panel-header"> {React.string("moor")} </header>
    <section className="hint-bar">
      {React.string("scaffold — the view model arrives in 4.1")}
    </section>
  </main>
}
