// The text form of a `RefSpec` for the review-create form: `main` or
// `branch:main`, `tag:v1`, `commit:<oid>`, `worktree`, `head`, `upstream`.

open Domain

let parse = (text: string): option<RefSpec.t> => {
  let t = String.trim(text)
  switch String.toLowerCase(t) {
  | "" => None
  | "worktree" | "working-tree" => Some(WorkingTree({}))
  | "head" => Some(Head({}))
  | "upstream" | "@{upstream}" => Some(Upstream({}))
  | _ =>
    switch String.split(t, ":") {
    | ["branch", name] => Some(Branch({name: name}))
    | ["tag", name] => Some(Tag({name: name}))
    | ["commit", oid] if String.length(oid) == 40 => Some(Commit({oid: oid}))
    | [name] => Some(Branch({name: name}))
    | _ => None
    }
  }
}

let print = (spec: RefSpec.t): string =>
  switch spec {
  | Branch({name}) => name
  | Tag({name}) => "tag:" ++ name
  | Commit({oid}) => "commit:" ++ oid
  | WorkingTree(_) => "worktree"
  | Head(_) => "head"
  | Upstream(_) => "upstream"
  }
