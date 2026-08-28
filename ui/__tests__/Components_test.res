// 4.4 component tests: each Row variant in both layouts, placeholder →
// chunk swap in the diff view, composer state, hint bar and tree.

open Vitest
open TestingLibrary

afterEach(cleanup)

let kebab = (name: string) =>
  name
  ->String.replaceRegExp(/([a-z0-9])([A-Z])/g, "$1-$2")
  ->String.toLowerCase

describe("Row", () => {
  Fixtures.variants("protocol", "Row")->Array.forEach(v => {
    [View.Layout.Unified, Split]->Array.forEach(layout => {
      let layoutName = layout == Unified ? "Unified" : "Split"
      test(`renders ${v} (${layoutName}) with its semantic class`, () => {
        let row = Fixtures.parse(Render.Row.schema, "protocol", "Row", v)
        let {container} = render(
          <Row row layout index=3 focused={v == "Added"} threads={v == "Modified" ? 2 : 0} />,
        )
        let el =
          Element.querySelector(container, "[role=\"row\"]")->Nullable.toOption->Option.getExn
        expect(Element.className(el))->toContain("row-" ++ kebab(v))
        expect(Element.className(el))->toContain(layout == Split ? "row-split" : "row-unified")
        expect(Element.getAttribute(el, "data-row-index"))->toEqual(Nullable.make("3"))
        expect(Element.hasAttribute(el, "data-focused"))->toBe(v == "Added")
        if v == "Modified" {
          expect(Element.querySelector(el, ".row-threads"))->not_->toBeNull
          expect(Element.querySelector(el, ".span-keyword"))->not_->toBeNull
          expect(Element.querySelector(el, ".cell-changed"))->not_->toBeNull
        }
        // Split layout always shows both sides for line rows.
        if ["Context", "Removed", "Added", "Modified"]->Array.includes(v) && layout == Split {
          expect(Element.querySelector(el, ".cell-left"))->not_->toBeNull
          expect(Element.querySelector(el, ".cell-right"))->not_->toBeNull
        }
      })
    })
  })
})

describe("DiffView", () => {
  test("shows the grid, then swaps placeholders for rows when a chunk lands", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let {container, rerender} = render(
      <DiffView diff=base layout=Unified focus={Diff({row: 121})} dispatch />,
    )
    // jsdom has no layout, so the virtualizer renders nothing until measured;
    // the grid and its scroll container must still be there.
    expect(Element.querySelector(container, "[role=\"grid\"]"))->not_->toBeNull
    expect(Element.querySelector(container, ".diff-scroll"))->not_->toBeNull
    let filled = {
      ...base,
      missing: [],
      rows: base.rows->Array.concat([{...base.rows->Array.getUnsafe(0), index: 122}]),
    }
    rerender(<DiffView diff=filled layout=Split focus={Diff({row: 122})} dispatch />)
    expect(Element.querySelector(container, "[role=\"grid\"]"))->not_->toBeNull
    expect(Screen.getByText("1 file-level thread(s)"))->toBeTruthy
  })
})

describe("Composer", () => {
  test("submits with ctrl+enter and discards with esc, never leaking keys", () => {
    let dispatch = fn()
    let draft = Fixtures.parse(View.Draft.schema, "client", "Draft", "default")
    let _ = render(<Composer draft pendingRefresh=true dispatch />)
    let box = Screen.getByPlaceholderText("Reply…")
    expect(Screen.getByText("changes pending"))->toBeTruthy
    FireEvent.change(box, {"target": {"value": "  "}})
    FireEvent.keyDown(box, {"key": "Enter", "ctrlKey": true})
    expect(dispatch)->not_->toHaveBeenCalled // blank bodies are not sent
    FireEvent.change(box, {"target": {"value": "looks good"}})
    FireEvent.keyDown(box, {"key": "j", "ctrlKey": false}) // plain text, not a command
    FireEvent.keyDown(box, {"key": "Enter", "ctrlKey": true})
    expect(dispatch)->toHaveBeenCalledWith(Action.DraftSubmitted({body: "looks good"}))
    FireEvent.keyDown(box, {"key": "Escape", "ctrlKey": false})
    expect(dispatch)->toHaveBeenLastCalledWith(Action.DraftDiscarded({}))
  })
})

describe("HintBar", () => {
  test("renders the hints the model carries, and the connection and progress", () => {
    let hint = Fixtures.parse(View.Hint.schema, "client", "Hint", "default")
    let _ = render(
      <HintBar
        hints=[hint]
        connection={Subscribed({})}
        progress={{viewed: 2, changedSinceViewed: 0, total: 5}}
      />,
    )
    expect(Screen.getByText("] f"))->toBeTruthy
    expect(Screen.getByText("next file"))->toBeTruthy
    expect(Screen.getByText("connected"))->toBeTruthy
    expect(Screen.getByText("2/5 viewed"))->toBeTruthy
  })
})

describe("Tree", () => {
  test("flattens expanded dirs in display order and marks the focused node", () => {
    let dispatch = fn()
    let tree = Fixtures.parse(View.TreeView.schema, "client", "TreeView", "default")
    let {container} = render(<Tree tree focus={Tree({index: 2})} dispatch />)
    let items = Element.querySelectorAll(container, "[role=\"treeitem\"]")
    // root, src (expanded), lib.rs, README.md
    expect(Array.length(items))->toBe(4)
    expect(Element.hasAttribute(items->Array.getUnsafe(2), "data-focused"))->toBe(true)
    FireEvent.doubleClick(items->Array.getUnsafe(2))
    let calls = mock(dispatch).calls
    let opened = calls->Array.some(args =>
      switch args->Array.getUnsafe(0) {
      | Action.Viewport(_) => true
      | _ => false
      }
    )
    expect(opened)->toBe(true)
    FireEvent.click(items->Array.getUnsafe(1))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetFocus({focus: Tree({index: 1})}))
  })
})

describe("Stepper", () => {
  test("shows the selected commit's body, committer and parents", () => {
    let dispatch = fn()
    let stepper = Fixtures.parse(View.CommitStepper.schema, "client", "CommitStepper", "default")
    let {container} = render(<Stepper stepper focus={CommitStepper({index: 0})} dispatch />)
    expect(Element.querySelector(container, ".commit-panel .commit-subject"))->not_->toBeNull
    expect(Element.querySelector(container, ".commit-body"))->not_->toBeNull
    expect(Element.querySelector(container, ".commit-panel .commit-oid"))->not_->toBeNull
    let items = Element.querySelectorAll(container, ".stepper-commit")
    expect(Element.hasAttribute(items->Array.getUnsafe(0), "data-focused"))->toBe(true)
    FireEvent.doubleClick(items->Array.getUnsafe(0))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.StepCommit({selected: Some(0)}))
  })
})

describe("Threads", () => {
  test("offers to apply a suggestion thread and dispatches ApplySuggestion", () => {
    let dispatch = fn()
    let thread = Fixtures.parse(View.ThreadView.schema, "client", "ThreadView", "default")
    let _ = render(
      <Threads title="Threads" threads=[thread] focus={Thread({index: 0})} indexOffset=0 dispatch />,
    )
    FireEvent.click(Screen.getByText("Apply suggestion (a)"))
    // The click also bubbles to the row (SetFocus), so not the last call.
    expect(dispatch)->toHaveBeenCalledWith(Action.ApplySuggestion({commentId: thread.root}))
    let plain = {...thread, suggestion: false}
    cleanup()
    let {container} = render(
      <Threads title="Threads" threads=[plain] focus={Thread({index: 0})} indexOffset=0 dispatch />,
    )
    expect(Element.querySelector(container, "button"))->toBeNull
  })
})

describe("DiffView (viewed)", () => {
  test("collapses a viewed file until the reader asks to see it", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let viewed = {...base, viewed: Viewed}
    let {container} = render(<DiffView diff=viewed layout=Unified focus={Diff({row: 121})} dispatch />)
    expect(Element.querySelector(container, ".diff-collapsed"))->not_->toBeNull
    expect(Element.querySelector(container, ".diff-scroll.hidden"))->not_->toBeNull
    FireEvent.click(Screen.getByText("show anyway"))
    expect(Element.querySelector(container, ".diff-collapsed"))->toBeNull
  })
})

describe("RefSpecText", () => {
  test("parses every ref spec form and prints it back", () => {
    let cases = [
      ("main", Some(Domain.RefSpec.Branch({name: "main"}))),
      ("branch:feature/x", Some(Branch({name: "feature/x"}))),
      ("tag:v1.0", Some(Tag({name: "v1.0"}))),
      ("commit:" ++ String.repeat("a", 40), Some(Commit({oid: String.repeat("a", 40)}))),
      ("commit:abc", None),
      ("worktree", Some(WorkingTree({}))),
      ("HEAD", Some(Head({}))),
      ("upstream", Some(Upstream({}))),
      ("", None),
    ]
    cases->Array.forEach(((text, want)) => expect(RefSpecText.parse(text))->toEqual(want))
    [
      Domain.RefSpec.Branch({name: "main"}),
      Tag({name: "v1"}),
      WorkingTree({}),
      Head({}),
      Upstream({}),
    ]->Array.forEach(spec => expect(RefSpecText.parse(RefSpecText.print(spec)))->toEqual(Some(spec)))
  })
})

describe("NewReview", () => {
  test("creates a multi-repo review with parsed targets", () => {
    let dispatch = fn()
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let _ = render(<NewReview workspaces=[ws] dispatch />)
    FireEvent.click(Screen.getByText("New review"))
    FireEvent.change(Screen.getByPlaceholderText("Title"), {"target": {"value": "  Parser  "}})
    FireEvent.click(Screen.getByText("+ target"))
    FireEvent.click(Screen.getByText("Create"))
    let calls = mock(dispatch).calls
    switch calls->Array.getUnsafe(0)->Array.getUnsafe(0) {
    | Action.CreateReview({workspaceId, title, targets}) => {
        expect(workspaceId)->toBe(ws.id)
        expect(title)->toBe("Parser")
        expect(Array.length(targets))->toBe(2)
        expect((targets->Array.getUnsafe(0)).base)->toEqual(Domain.RefSpec.Branch({name: "main"}))
        expect((targets->Array.getUnsafe(0)).head)->toEqual(Domain.RefSpec.WorkingTree({}))
      }
    | _ => expect(false)->toBe(true)
    }
  })

  test("does not submit without a title", () => {
    let dispatch = fn()
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let _ = render(<NewReview workspaces=[ws] dispatch />)
    FireEvent.click(Screen.getByText("New review"))
    FireEvent.click(Screen.getByText("Create"))
    expect(dispatch)->not_->toHaveBeenCalled
  })
})

describe("NewReview (no repos)", () => {
  test("explains how to attach a repo instead of offering Create", () => {
    let dispatch = fn()
    let _ = render(<NewReview workspaces=[] dispatch />)
    FireEvent.click(Screen.getByText("New review"))
    expect(Array.length(Screen.queryAllByText("Create")))->toBe(0)
    let _ = Screen.getByTextRe(%re("/workspace attach/"))
    expect(dispatch)->not_->toHaveBeenCalled
  })
})

describe("NewReview (late workspaces)", () => {
  test("offers Create once workspaces arrive after mount", () => {
    let dispatch = fn()
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let {rerender} = render(<NewReview workspaces=[] dispatch />)
    FireEvent.click(Screen.getByText("New review"))
    expect(Array.length(Screen.queryAllByText("Create")))->toBe(0)
    rerender(<NewReview workspaces=[ws] dispatch />)
    FireEvent.change(Screen.getByPlaceholderText("Title"), {"target": {"value": "Late"}})
    FireEvent.click(Screen.getByText("Create"))
    switch mock(dispatch).calls->Array.getUnsafe(0)->Array.getUnsafe(0) {
    | Action.CreateReview({workspaceId, targets}) => {
        expect(workspaceId)->toBe(ws.id)
        expect(Array.length(targets))->toBe(1)
      }
    | _ => expect(false)->toBe(true)
    }
  })
})

describe("Tree (viewed)", () => {
  test("the per-file checkbox marks and unmarks viewed without moving focus", () => {
    let dispatch = fn()
    let tree = Fixtures.parse(View.TreeView.schema, "client", "TreeView", "default")
    let {container} = render(<Tree tree focus={Tree({index: 0})} dispatch />)
    let boxes = Element.querySelectorAll(container, "input.tree-viewed")
    // lib.rs (changed since viewed) and README.md (unviewed)
    expect(Array.length(boxes))->toBe(2)
    FireEvent.click(boxes->Array.getUnsafe(0))
    let calls = mock(dispatch).calls
    expect(Array.length(calls))->toBe(1)
    switch calls->Array.getUnsafe(0)->Array.getUnsafe(0) {
    | Action.MarkViewed({file}) => expect(file.path)->toBe("src/lib.rs")
    | _ => expect(false)->toBe(true)
    }
  })
})
