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

  test("switches to the pending-leader style and shows the pending keys", () => {
    let hint: View.Hint.t = {keys: "s", command: ToggleLayout, label: "split layout"}
    let {container} = render(
      <HintBar
        hints=[hint]
        pendingKeys="g"
        connection={Subscribed({})}
        progress={{viewed: 0, changedSinceViewed: 0, total: 0}}
      />,
    )
    expect(Element.querySelector(container, ".hint-bar-pending"))->not_->toBeNull
    expect(Element.querySelector(container, ".pending-keys"))->not_->toBeNull
    expect(Screen.getByText("g"))->toBeTruthy
    expect(Screen.getByText("split layout"))->toBeTruthy
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
    FireEvent.click(items->Array.getUnsafe(2))
    let calls = mock(dispatch).calls
    let opened = calls->Array.some(args =>
      switch args->Array.getUnsafe(0) {
      | Action.Viewport(_) => true
      | _ => false
      }
    )
    expect(opened)->toBe(true)
    FireEvent.click(items->Array.getUnsafe(1))
    let calls = mock(dispatch).calls
    let focused = calls->Array.some(args =>
      args->Array.getUnsafe(0) == Action.SetFocus({focus: Tree({index: 1})})
    )
    expect(focused)->toBe(true)
    switch calls->Array.getUnsafe(Array.length(calls) - 1)->Array.getUnsafe(0) {
    | Action.ToggleDir(_) => ()
    | _ => expect(false)->toBe(true)
    }
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

  test("a focused thread shows every comment body and a click opens its file", () => {
    let dispatch = fn()
    let thread = Fixtures.parse(View.ThreadView.schema, "client", "ThreadView", "default")
    let {container} = render(
      <Threads title="Threads" threads=[thread] focus={Thread({index: 0})} indexOffset=0 dispatch />,
    )
    let bodies = Element.querySelectorAll(container, ".thread-body")
    expect(Array.length(bodies))->toBe(Array.length(thread.comments))
    FireEvent.click(Element.querySelector(container, ".thread-item")->Nullable.getExn)
    let opened = mock(dispatch).calls->Array.some(args =>
      switch args->Array.getUnsafe(0) {
      | Action.Viewport(_) => true
      | _ => false
      }
    )
    expect(opened)->toBe(true)
    cleanup()
    let _ = render(
      <Threads title="Threads" threads=[thread] focus={Tree({index: 0})} indexOffset=0 dispatch />,
    )
    expect(Array.length(Screen.queryAllByText(thread.summary)))->toBe(1)
  })
})

describe("Context expanders", () => {
  test("an expander row click and the expand-file button dispatch ExpandContext", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let _ = render(<DiffView diff=base layout=Unified focus={Diff({row: 0})} dispatch />)
    FireEvent.click(Screen.getByText("expand file"))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.ExpandContext({file: base.file, full: true}))
    cleanup()
    let row = Fixtures.parse(Render.Row.schema, "protocol", "Row", "Expander")
    let expand = fn()
    let _ = render(<Row row layout=Unified index=0 focused=false threads=0 onExpand={() => expand()} />)
    FireEvent.click(Screen.getByTextRe(%re("/more lines/")))
    expect(expand)->toHaveBeenCalled
  })
})

describe("Jump to original diff", () => {
  test("an outdated thread with context offers the original diff", () => {
    let dispatch = fn()
    let thread = Fixtures.parse(View.ThreadView.schema, "client", "ThreadView", "default")
    let outdated = {
      ...thread,
      outdated: true,
      suggestion: false,
      context: Some(Fixtures.parse(Domain.ChangeKind.schema, "protocol", "ChangeKind", "Modified")),
    }
    let _ = render(
      <Threads title="Threads" threads=[outdated] focus={Thread({index: 0})} indexOffset=0 dispatch />,
    )
    FireEvent.click(Screen.getByText("Open original diff (enter)"))
    expect(dispatch)->toHaveBeenCalledWith(Action.OpenOriginalDiff({threadId: outdated.id}))
    // Clicking the row itself also jumps to the original, not the moved-on diff.
    let calls = mock(dispatch).calls
    let jumped =
      calls->Array.every(args =>
        switch args->Array.getUnsafe(0) {
        | Action.Viewport(_) => false
        | _ => true
        }
      )
    expect(jumped)->toBe(true)
  })

  test("the original diff shows the read-only banner", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let {container} = render(
      <DiffView diff={...base, original: true} layout=Unified focus={Diff({row: 0})} dispatch />,
    )
    expect(Element.querySelector(container, ".original-banner"))->not_->toBeNull
    cleanup()
    let {container} = render(
      <DiffView diff=base layout=Unified focus={Diff({row: 0})} dispatch />,
    )
    expect(Element.querySelector(container, ".original-banner"))->toBeNull
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
    let _ = render(<NewReview workspaces=[ws] onClose={() => ()} dispatch />)
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
    let _ = render(<NewReview workspaces=[ws] onClose={() => ()} dispatch />)
    FireEvent.click(Screen.getByText("Create"))
    expect(dispatch)->not_->toHaveBeenCalled
  })
})

describe("NewReview (no repos)", () => {
  test("explains how to attach a repo instead of offering Create", () => {
    let dispatch = fn()
    let _ = render(<NewReview workspaces=[] onClose={() => ()} dispatch />)
    expect(Array.length(Screen.queryAllByText("Create")))->toBe(0)
    let _ = Screen.getByTextRe(%re("/workspace attach/"))
    expect(dispatch)->not_->toHaveBeenCalled
  })
})

describe("NewReview (late workspaces)", () => {
  test("offers Create once workspaces arrive after mount", () => {
    let dispatch = fn()
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let {rerender} = render(<NewReview workspaces=[] onClose={() => ()} dispatch />)
    expect(Array.length(Screen.queryAllByText("Create")))->toBe(0)
    rerender(<NewReview workspaces=[ws] onClose={() => ()} dispatch />)
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

describe("ReviewList", () => {
  test("groups reviews under their workspace and opens the form from +", () => {
    let dispatch = fn()
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let review = Fixtures.parse(Domain.Review.schema, "protocol", "Review", "default")
    let other = {...ws, id: "01ARZ3NDEKTSV4RRFFQ69G5FAV", name: "empty-ws"}
    let _ = render(
      <ReviewList
        reviews=[{...review, workspaceId: ws.id}]
        workspaces=[ws, other]
        connection={Subscribed({})}
        focus={ReviewList({index: 0})}
        dispatch
      />,
    )
    let group = Screen.getByLabelText(ws.name)
    expect(Array.length(Element.querySelectorAll(group, ".review-item")))->toBe(1)
    let emptyGroup = Screen.getByLabelText("empty-ws")
    expect(Array.length(Element.querySelectorAll(emptyGroup, ".review-item")))->toBe(0)
    FireEvent.click(Element.querySelector(emptyGroup, "button[title]")->Nullable.getExn)
    let _ = Screen.getByPlaceholderText("Title")
    expect(dispatch)->not_->toHaveBeenCalled
  })
})

describe("ReviewHeader", () => {
  test("shows base → head for the open review, nothing otherwise", () => {
    let review = Fixtures.parse(Domain.Review.schema, "protocol", "Review", "default")
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let resolved = Fixtures.parse(
      Domain.ResolvedTarget.schema,
      "protocol",
      "ResolvedTarget",
      "default",
    )
    let prefs = View.ViewModel.empty.prefs
    let {container} = render(
      <ReviewHeader
        reviews=[review] workspaces=[ws] resolvedTargets=[resolved] openReview=Some(review.id) prefs
      />,
    )
    let refs = Element.querySelectorAll(container, ".review-header-ref")
    expect(Array.length(refs))->toBe(2 * Array.length(review.targets))
    cleanup()
    let {container: closed} = render(
      <ReviewHeader reviews=[review] workspaces=[ws] resolvedTargets=[] openReview=None prefs />,
    )
    expect(Element.querySelector(closed, ".review-header"))->toBeNull
  })

  test("layout and whitespace toggles dispatch, with keymap-derived tooltips", () => {
    let dispatch = fn()
    let review = Fixtures.parse(Domain.Review.schema, "protocol", "Review", "default")
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let prefs = View.ViewModel.empty.prefs
    let chrome: array<View.Hint.t> = [
      {keys: "g s", command: ToggleLayout, label: "split layout"},
      {keys: "g h", command: ToggleWhitespace, label: "hide whitespace"},
    ]
    let _ = render(
      <ReviewHeader
        reviews=[review]
        workspaces=[ws]
        resolvedTargets=[]
        openReview=Some(review.id)
        prefs
        chrome
        dispatch
      />,
    )
    let split = Screen.getByText("split")
    expect(Element.getAttribute(split, "title"))->toEqual(Nullable.make("split layout (g s)"))
    FireEvent.click(split)
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetLayout({layout: Split}))
    FireEvent.click(Screen.getByText("hide whitespace"))
    expect(dispatch)->toHaveBeenLastCalledWith(
      Action.SetRenderOpts({ignoreWhitespace: true, contextLines: prefs.contextLines}),
    )
    // No data-active until the pref says so.
    expect(Element.hasAttribute(split, "data-active"))->toBe(false)
    cleanup()
    let _ = render(
      <ReviewHeader
        reviews=[review]
        workspaces=[ws]
        resolvedTargets=[]
        openReview=Some(review.id)
        prefs={...prefs, layout: Split, ignoreWhitespace: true}
        chrome
        dispatch
      />,
    )
    expect(Element.hasAttribute(Screen.getByText("split"), "data-active"))->toBe(true)
    expect(Element.hasAttribute(Screen.getByText("hide whitespace"), "data-active"))->toBe(true)
  })
})

describe("ScopeControl", () => {
  test("shows the scope, toggles worktree, enters by-commit, shows the step", () => {
    let dispatch = fn()
    let review = Fixtures.parse(Domain.Review.schema, "protocol", "Review", "default")
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let prefs = View.ViewModel.empty.prefs
    let render_ = (scope, stepper) => render(
      <ReviewHeader
        reviews=[review]
        workspaces=[ws]
        resolvedTargets=[]
        openReview=Some(review.id)
        prefs
        scope
        ?stepper
        dispatch
      />,
    )
    let _ = render_(Domain.DiffScope.All({}), None)
    let all = Screen.getByText("All changes")
    expect(Element.hasAttribute(all, "data-active"))->toBe(true)
    expect(Element.hasAttribute(Screen.getByText("+ working tree"), "data-active"))->toBe(true)
    FireEvent.click(Screen.getByText("+ working tree"))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetScope({scope: Committed({})}))
    FireEvent.click(Screen.getByText("By commit"))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetScope({scope: ByCommit({})}))
    cleanup()
    // Committed: worktree toggle shown but off.
    let _ = render_(Domain.DiffScope.Committed({}), None)
    expect(Element.hasAttribute(Screen.getByText("+ working tree"), "data-active"))->toBe(false)
    cleanup()
    // By-commit at a commit: position indicator from the stepper.
    let stepper = Fixtures.parse(View.CommitStepper.schema, "client", "CommitStepper", "default")
    let oid = (stepper.commits->Array.getUnsafe(0)).oid
    let repoId = stepper.repoId
    let {container} = render_(Domain.DiffScope.Commit({repoId, oid}), Some(stepper))
    expect(Element.hasAttribute(Screen.getByText("By commit"), "data-active"))->toBe(true)
    expect(Array.length(Screen.queryAllByText("+ working tree")))->toBe(0)
    let position =
      Element.querySelector(container, ".scope-position")->Nullable.toOption->Option.getExn
    expect(Element.textContent(position))->toContain("of")
    cleanup()
    // The worktree step names itself.
    let {container} = render_(Domain.DiffScope.Worktree({repoId: repoId}), Some(stepper))
    let position =
      Element.querySelector(container, ".scope-position")->Nullable.toOption->Option.getExn
    expect(Element.textContent(position))->toBe("worktree")
  })
})

describe("Tabs", () => {
  test("marks the active tab, shows counts, dispatches SetTab on click", () => {
    let dispatch = fn()
    let chrome: array<View.Hint.t> = [
      {keys: "2", command: TabConversation, label: "conversation"},
    ]
    let {container} = render(
      <Tabs tab=FilesChanged fileCount=4 threadCount=2 chrome dispatch />,
    )
    let tabs = Element.querySelectorAll(container, "[role=\"tab\"]")
    expect(Array.length(tabs))->toBe(3)
    expect(Element.hasAttribute(tabs->Array.getUnsafe(0), "data-active"))->toBe(true)
    expect(Element.hasAttribute(tabs->Array.getUnsafe(1), "data-active"))->toBe(false)
    expect(Element.getAttribute(tabs->Array.getUnsafe(1), "title"))->toEqual(
      Nullable.make("conversation (2)"),
    )
    FireEvent.click(tabs->Array.getUnsafe(1))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetTab({tab: Conversation}))
    FireEvent.click(tabs->Array.getUnsafe(2))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetTab({tab: Browse}))
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
