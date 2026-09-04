// Button tooltips come from the keymap's chrome (UI-DESIGN: derived,
// never hand-written). A control whose command has no entry has no
// binding, which is a bug.

let tip = (chrome: array<View.Hint.t>, command: View.Command.t): option<string> =>
  chrome
  ->Array.find(h => h.command == command)
  ->Option.map(h => h.label ++ " (" ++ h.keys ++ ")")

/// The configured chord by itself, for compact menu hints. Like `tip`,
/// this follows the first binding in the core-derived chrome.
let keys = (chrome: array<View.Hint.t>, command: View.Command.t): option<string> =>
  chrome->Array.find(h => h.command == command)->Option.map(h => h.keys)
