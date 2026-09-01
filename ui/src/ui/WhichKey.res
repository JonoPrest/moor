// Which-key popup (UI-DESIGN: modal keys): while a prefix is pending,
// the group's label and every continuation — derived from the keymap,
// never hand-written.

@react.component
let make = (~pendingKeys: string, ~pendingLabel: option<string>, ~hints: array<View.Hint.t>) =>
  pendingKeys == ""
    ? React.null
    : <div className="whichkey" role="menu" ariaLabel="pending keys">
        <div className="whichkey-title">
          <UI.Kbd keys=pendingKeys />
          {React.string(" " ++ pendingLabel->Option.getOr(""))}
        </div>
        <div className="whichkey-grid">
          {hints
          ->Array.map(h =>
            <div key={h.keys ++ h.label} className="whichkey-row">
              <UI.Kbd keys=h.keys />
              <span className="whichkey-label"> {React.string(h.label)} </span>
            </div>
          )
          ->React.array}
        </div>
      </div>
