// Merged file tree (§5.5, §5.6) from `View.treeView`; every node in
// display order gets its visible index so focus can name it.

open View

let viewedGlyph = (v: ViewedState.t) =>
  switch v {
  | Viewed => "✓"
  | ChangedSinceViewed => "↻"
  | Unviewed => " "
  }

let changeGlyph = (c: option<ChangeKindKind.t>) =>
  switch c {
  | Some(Added) => "A"
  | Some(Deleted) => "D"
  | Some(Modified) => "M"
  | Some(Renamed) => "R"
  | None => ""
  }

/// Flatten visible nodes in display order, with depth.
let rec flatten = (nodes: array<TreeNode.t>, depth: int, out: array<(TreeNode.t, int)>) =>
  nodes->Array.forEach(n => {
    out->Array.push((n, depth))
    switch n {
    | Dir({expanded: true, children}) => flatten(children, depth + 1, out)
    | Dir(_) | File(_) => ()
    }
  })

@react.component
let make = (~tree: TreeView.t, ~focus: Focus.t, ~dispatch: Action.t => unit) => {
  let rows = []
  flatten(tree.roots, 0, rows)
  let focusedIndex = switch focus {
  | Tree({index}) => Some(index)
  | _ => None
  }
  <nav className="tree panel" ariaLabel="files">
    <header className="panel-header">
      {React.string(tree.breadcrumbs->Array.join(" / "))}
    </header>
    <ul className="tree-list" role="tree">
      {rows
      ->Array.mapWithIndex(((node, depth), i) => {
        let focused = focusedIndex == Some(i)
        let style: ReactDOM.Style.t = {paddingLeft: Int.toString(depth * 12 + 4) ++ "px"}
        let onSelect = _ => dispatch(SetFocus({focus: Focus.Tree({index: i})}))
        let item = switch node {
        | Dir({name, repoId, path, expanded, changedBelow}) =>
          <li
            key={Int.toString(i)}
            className="tree-dir"
            role="treeitem"
            style
            onClick=onSelect
            onDoubleClick={_ => dispatch(ToggleDir({repoId, path}))}>
            <span className="tree-glyph"> {React.string(expanded ? "▾" : "▸")} </span>
            <span className="tree-name"> {React.string(name)} </span>
            {changedBelow > 0
              ? <span className="tree-badge"> {React.string(Int.toString(changedBelow))} </span>
              : React.null}
          </li>
        | File({name, repoId, path, change, viewed, open_}) =>
          <li
            key={Int.toString(i)}
            className={"tree-file" ++ (open_ ? " tree-open" : "")}
            role="treeitem"
            style
            onClick=onSelect
            onDoubleClick={_ =>
              dispatch(Viewport({file: {repoId, path}, firstRow: 0, lastRow: 59}))}>
            <span className="tree-glyph tree-viewed"> {React.string(viewedGlyph(viewed))} </span>
            <span className="tree-name"> {React.string(name)} </span>
            <span className={"tree-change tree-change-" ++ String.toLowerCase(changeGlyph(change))}>
              {React.string(changeGlyph(change))}
            </span>
          </li>
        }
        Attrs.focused(item, focused)
      })
      ->React.array}
    </ul>
  </nav>
}
