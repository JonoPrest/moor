// Key chords (client-core `keymap.rs`) and the browser → chord normaliser
// (plan 4.5). The core resolves chords; the UI only captures them.

module NamedKey = {
  type t =
    | Enter
    | Esc
    | Tab
    | Backspace
    | Space
    | Up
    | Down
    | Left
    | Right
    | PageUp
    | PageDown
    | Home
    | End
  let all = [Enter, Esc, Tab, Backspace, Space, Up, Down, Left, Right, PageUp, PageDown, Home, End]
  let schema: S.t<t> = S.enum(all)
}

module KeyCode = {
  type t = Char({c: string}) | Named({key: NamedKey.t})
  let schema: S.t<t> = S.union([
    S.object(s => {
      s.tag("type", "Char")
      Char({c: s.field("c", S.string)})
    }),
    S.object(s => {
      s.tag("type", "Named")
      Named({key: s.field("key", NamedKey.schema)})
    }),
  ])
}

type modifiers = {ctrl: bool, alt: bool, shift: bool, meta: bool}
let modifiers: S.t<modifiers> = S.object(s => {
  ctrl: s.field("ctrl", S.bool),
  alt: s.field("alt", S.bool),
  shift: s.field("shift", S.bool),
  meta: s.field("meta", S.bool),
})

type keyChord = {key: KeyCode.t, mods: modifiers}
let keyChord: S.t<keyChord> = S.object(s => {
  key: s.field("key", KeyCode.schema),
  mods: s.field("mods", modifiers),
})

/// What a browser `KeyboardEvent` gives us.
type browserKey = {
  key: string,
  ctrlKey: bool,
  altKey: bool,
  shiftKey: bool,
  metaKey: bool,
}

/// `KeyboardEvent.key` → chord. Printable keys become `Char` (shift is
/// implied by the character, so it is dropped); named keys carry their
/// modifiers. Modifier-only presses and unknown keys are `None`.
let ofBrowser = (ev: browserKey): option<keyChord> => {
  let named = switch ev.key {
  | "Enter" => Some(NamedKey.Enter)
  | "Escape" => Some(Esc)
  | "Tab" => Some(Tab)
  | "Backspace" => Some(Backspace)
  | " " => Some(Space)
  | "ArrowUp" => Some(Up)
  | "ArrowDown" => Some(Down)
  | "ArrowLeft" => Some(Left)
  | "ArrowRight" => Some(Right)
  | "PageUp" => Some(PageUp)
  | "PageDown" => Some(PageDown)
  | "Home" => Some(Home)
  | "End" => Some(End)
  | _ => None
  }
  switch named {
  | Some(key) =>
    Some({
      key: KeyCode.Named({key: key}),
      mods: {ctrl: ev.ctrlKey, alt: ev.altKey, shift: ev.shiftKey, meta: ev.metaKey},
    })
  | None =>
    if String.length(ev.key) == 1 {
      Some({
        key: KeyCode.Char({c: ev.key}),
        mods: {ctrl: ev.ctrlKey, alt: ev.altKey, shift: false, meta: ev.metaKey},
      })
    } else {
      None
    }
  }
}

let toJson = (chord: keyChord): JSON.t => S.reverseConvertToJsonOrThrow(chord, keyChord)
