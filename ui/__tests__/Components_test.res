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
    [View.Layout.Unified, Split]->Array.forEach(
      layout => {
        let layoutName = layout == Unified ? "Unified" : "Split"
        test(
          `renders ${v} (${layoutName}) with its semantic class`,
          () => {
            let row = Fixtures.parse(Render.Row.schema, "protocol", "Row", v)
            let {container} = render(
              <Row
                row
                layout
                index=3
                focused={v == "Added"}
                threads={v == "Modified" ? [{thread: "t1", side: Head}] : []}
              />,
            )
            let el =
              Element.querySelector(container, "[role=\"row\"]")->Nullable.toOption->Option.getExn
            expect(Element.className(el))->toContain("row-" ++ kebab(v))
            expect(Element.className(el))->toContain(layout == Split ? "row-split" : "row-unified")
            expect(Element.getAttribute(el, "data-row-index"))->toEqual(Nullable.make("3"))
            expect(Element.hasAttribute(el, "data-focused"))->toBe(v == "Added")
            if v == "Modified" {
              // The marker hangs on the cell the thread is anchored to,
              // so a base thread never shows against the green half.
              expect(Element.querySelector(el, ".cell-right .cell-threads"))->not_->toBeNull
              expect(Element.querySelector(el, ".cell-left .cell-threads"))->toBeNull
              expect(Element.querySelector(el, ".span-keyword"))->not_->toBeNull
              expect(Element.querySelector(el, ".cell-changed"))->not_->toBeNull
            }

            // Split layout always shows both sides for line rows.
            if ["Context", "Removed", "Added", "Modified"]->Array.includes(v) && layout == Split {
              expect(Element.querySelector(el, ".cell-left"))->not_->toBeNull
              expect(Element.querySelector(el, ".cell-right"))->not_->toBeNull
            }
          },
        )
      },
    )
  })
})

describe("DiffView", () => {
  test("shows the grid, then swaps placeholders for rows when a chunk lands", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let {container, rerender} = render(
      <DiffView diff=base layout=Unified focus={Diff({row: 121, side: Head})} dispatch />,
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
    rerender(<DiffView diff=filled layout=Split focus={Diff({row: 122, side: Head})} dispatch />)
    expect(Element.querySelector(container, "[role=\"grid\"]"))->not_->toBeNull
    expect(Screen.getByText("1 file-level thread(s)"))->toBeTruthy
  })
})

describe("DiffSeen.mergeSeen", () => {
  test("rows survive a viewport move and clear on a file change", () => {
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let row = (index: int): View.DiffRow.t => {...base.rows->Array.getUnsafe(0), index}
    let key = DiffSeen.fileKey(base)
    // First window: rows 1 and 2.
    let seen = DiffSeen.mergeSeen(Dict.make(), "", key, [row(1), row(2)])
    // The window moves on: row 3 arrives, rows 1–2 must not regress to
    // placeholders (the flicker while scrolling).
    let seen = DiffSeen.mergeSeen(seen, key, key, [row(3)])
    expect(Array.length(Dict.keysToArray(seen)))->toBe(3)
    // A different file (or a re-render with new totals) starts fresh.
    let other = DiffSeen.fileKey({...base, file: {...base.file, path: "other.rs"}})
    let seen = DiffSeen.mergeSeen(seen, key, other, [row(9)])
    expect(Dict.keysToArray(seen))->toEqual(["9"])
  })
})

describe("InlineThread", () => {
  test("renders the comments, dispatches reply/resolve, hosts the composer", () => {
    let dispatch = fn()
    let thread = Fixtures.parse(View.ThreadView.schema, "client", "ThreadView", "default")
    let {rerender} = render(
      <InlineThread thread focused=false index=3 composer=React.null dispatch />,
    )
    // Every comment body renders inline (design: threads under their row).
    expect(Screen.getByText("This should be a newtype."))->toBeTruthy
    FireEvent.click(Screen.getByText("Reply (r)"))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.ReplyOpened({threadId: thread.id}))
    FireEvent.click(Screen.getByText("Resolve (x)"))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.ResolveThread({threadId: thread.id}))
    // Clicking the card focuses the thread by its list index.
    FireEvent.click(Screen.getByText("This should be a newtype."))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetFocus({focus: Thread({index: 3})}))
    // While a reply is being written the composer replaces the actions.
    rerender(
      <InlineThread
        thread focused=true index=3 composer={<div> {React.string("the composer")} </div>} dispatch
      />,
    )
    expect(Screen.getByText("the composer"))->toBeTruthy
    expect(Array.length(Screen.queryAllByText("Reply (r)")))->toBe(0)
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
        progress={{viewed: 2, changedSinceViewed: 0, total: 5, additions: 0, deletions: 0}}
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
        progress={{viewed: 0, changedSinceViewed: 0, total: 0, additions: 0, deletions: 0}}
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
    // A tree click is the mouse alias of `enter` on a tree file: the same
    // deliberate open, pinned to the top of the viewport.
    let opened = calls->Array.some(
      args =>
        switch args->Array.getUnsafe(0) {
        | Action.OpenFileAt({landing: Pin}) => true
        | _ => false
        },
    )
    expect(opened)->toBe(true)
    FireEvent.click(items->Array.getUnsafe(1))
    let calls = mock(dispatch).calls
    let focused =
      calls->Array.some(
        args => args->Array.getUnsafe(0) == Action.SetFocus({focus: Tree({index: 1})}),
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
      <Threads
        title="Threads" threads=[thread] focus={Thread({index: 0})} indexOffset=0 dispatch
      />,
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
      <Threads
        title="Threads" threads=[thread] focus={Thread({index: 0})} indexOffset=0 dispatch
      />,
    )
    let bodies = Element.querySelectorAll(container, ".thread-body")
    expect(Array.length(bodies))->toBe(Array.length(thread.comments))
    FireEvent.click(Element.querySelector(container, ".thread-item")->Nullable.getExn)
    let opened = mock(dispatch).calls->Array.some(
      args =>
        switch args->Array.getUnsafe(0) {
        | Action.Viewport(_) => true
        | _ => false
        },
    )
    expect(opened)->toBe(true)
    cleanup()
    let _ = render(
      <Threads title="Threads" threads=[thread] focus={Tree({index: 0})} indexOffset=0 dispatch />,
    )
    expect(Array.length(Screen.queryAllByText(thread.summary)))->toBe(1)
  })
})

describe("Palette", () => {
  test("content hits jump to the file; actions run keymap commands", () => {
    let dispatch = fn()
    let cs = Fixtures.parse(View.ContentSearchView.schema, "client", "ContentSearchView", "default")
    let _ = render(<Palette contentSearch=Some(cs) actionPalette=false chrome=[] dispatch />)
    let hit = cs.hits->Array.getUnsafe(0)
    FireEvent.click(Screen.getByText(hit.path ++ ":" ++ Int.toString(hit.line)))
    let calls = mock(dispatch).calls
    let jumped = calls->Array.some(
      args =>
        switch args->Array.getUnsafe(0) {
        | Action.Viewport({file}) => file.path == hit.path
        | _ => false
        },
    )
    expect(jumped)->toBe(true)
    cleanup()
    let chrome: array<View.Hint.t> = [
      {keys: "s", command: ToggleLayout, label: "split/unified"},
      {keys: "w", command: ToggleWhitespace, label: "whitespace"},
    ]
    let _ = render(<Palette contentSearch=None actionPalette=true chrome dispatch />)
    FireEvent.click(Screen.getByText("split/unified"))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.RunCommand({command: ToggleLayout}))
  })
})

describe("Context expanders", () => {
  test("an expander row click and the expand-file button dispatch ExpandContext", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let _ = render(
      <DiffView diff=base layout=Unified focus={Diff({row: 0, side: Head})} dispatch />,
    )
    FireEvent.click(Screen.getByText("expand file"))
    expect(dispatch)->toHaveBeenLastCalledWith(Action.ExpandContext({file: base.file, full: true}))
    cleanup()
    let row = Fixtures.parse(Render.Row.schema, "protocol", "Row", "Expander")
    let expand = fn()
    let _ = render(
      <Row row layout=Unified index=0 focused=false threads=[] onExpand={() => expand()} />,
    )
    FireEvent.click(Screen.getByTextRe(/more lines/))
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
      <Threads
        title="Threads" threads=[outdated] focus={Thread({index: 0})} indexOffset=0 dispatch
      />,
    )
    FireEvent.click(Screen.getByText("Open original diff (enter)"))
    expect(dispatch)->toHaveBeenCalledWith(Action.OpenOriginalDiff({threadId: outdated.id}))
    // Clicking the row itself also jumps to the original, not the moved-on diff.
    let calls = mock(dispatch).calls
    let jumped = calls->Array.every(
      args =>
        switch args->Array.getUnsafe(0) {
        | Action.Viewport(_) => false
        | _ => true
        },
    )
    expect(jumped)->toBe(true)
  })

  test("the original diff shows the read-only banner", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let {container} = render(
      <DiffView
        diff={...base, original: true} layout=Unified focus={Diff({row: 0, side: Head})} dispatch
      />,
    )
    expect(Element.querySelector(container, ".original-banner"))->not_->toBeNull
    cleanup()
    let {container} = render(
      <DiffView diff=base layout=Unified focus={Diff({row: 0, side: Head})} dispatch />,
    )
    expect(Element.querySelector(container, ".original-banner"))->toBeNull
  })
})

describe("DiffView (viewed)", () => {
  test("collapses a viewed file until the reader asks to see it", () => {
    let dispatch = fn()
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let viewed = {...base, viewed: Viewed}
    let {container} = render(
      <DiffView diff=viewed layout=Unified focus={Diff({row: 121, side: Head})} dispatch />,
    )
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
    ]->Array.forEach(
      spec => expect(RefSpecText.parse(RefSpecText.print(spec)))->toEqual(Some(spec)),
    )
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
    let _ = Screen.getByTextRe(/workspace attach/)
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
    let split = Screen.getByText("Split")
    expect(Element.getAttribute(split, "title"))->toEqual(Nullable.make("split layout (g s)"))
    FireEvent.click(split)
    expect(dispatch)->toHaveBeenLastCalledWith(Action.SetLayout({layout: Split}))
    FireEvent.click(Screen.getByText("hide whitespace"))
    expect(dispatch)->toHaveBeenLastCalledWith(
      Action.SetRenderOpts({ignoreWhitespace: true, contextLines: prefs.contextLines}),
    )
    // The inactive segment carries no data-active (Unified is default).
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
    expect(Element.hasAttribute(Screen.getByText("Split"), "data-active"))->toBe(true)
    expect(Element.hasAttribute(Screen.getByText("hide whitespace"), "data-active"))->toBe(true)
  })
})

describe("ScopeControl", () => {
  test("shows the scope, toggles worktree, enters by-commit, shows the step", () => {
    let dispatch = fn()
    let review = Fixtures.parse(Domain.Review.schema, "protocol", "Review", "default")
    let ws = Fixtures.parse(Domain.Workspace.schema, "protocol", "Workspace", "default")
    let prefs = View.ViewModel.empty.prefs
    let render_ = (scope, stepper) =>
      render(
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
    let chrome: array<View.Hint.t> = [{keys: "2", command: TabConversation, label: "conversation"}]
    let {container} = render(<Tabs tab=FilesChanged fileCount=4 threadCount=2 chrome dispatch />)
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

describe("Tree (rows)", () => {
  test("rows show line stats and thread badges, never a checkbox", () => {
    let dispatch = fn()
    let tree = Fixtures.parse(View.TreeView.schema, "client", "TreeView", "default")
    let {container} = render(<Tree tree focus={Tree({index: 0})} dispatch />)
    // The design has no checkbox in the tree (`v` toggles viewed).
    expect(Element.querySelectorAll(container, "input")->Array.length)->toBe(0)
    // lib.rs carries +9 −1 and a 2-thread badge (from the fixture).
    expect(Screen.getByText("+9"))->toBeTruthy
    expect(Screen.getByText("−1"))->toBeTruthy
    let badge = Element.querySelector(container, ".tree-threads")->Nullable.toOption->Option.getExn
    expect(Element.textContent(badge))->toBe("2")
    // Clicking a file opens it.
    FireEvent.click(Screen.getByText("lib.rs"))
    let calls = mock(dispatch).calls
    switch calls->Array.getUnsafe(Array.length(calls) - 1)->Array.getUnsafe(0) {
    | Action.OpenFileAt({file, landing}) => {
        expect(file.path)->toBe("src/lib.rs")
        expect(landing)->toBe(Pin)
      }
    | _ => expect(false)->toBe(true)
    }
  })
})

// jsdom has no layout, so the stacked view's scroll-into-view is a no-op
// here; without the stub mounting an open file throws.
%%raw(`
if (!globalThis.Element.prototype.scrollIntoView) {
  globalThis.Element.prototype.scrollIntoView = function () {}
}
`)

describe("Row sides", () => {
  // The fixture row is modified (two cells) and carries a base-anchored
  // thread; a removed row below it gives the drag a second base line.
  let atLine = (row: Render.Row.t, n: int): Render.Row.t =>
    switch row {
    | Modified({left, right}) =>
      Modified({left: {...left, lineNo: n}, right: {...right, lineNo: n}})
    | Context({left, right}) => Context({left: {...left, lineNo: n}, right: {...right, lineNo: n}})
    | Removed({left}) => Removed({left: {...left, lineNo: n}})
    | Added({right}) => Added({right: {...right, lineNo: n}})
    | HunkHeader(_) | Expander(_) | WhitespaceOnly(_) => row
    }

  let diff = (): View.DiffView.t => {
    let base = Fixtures.parse(View.DiffView.schema, "client", "DiffView", "default")
    let modified = Fixtures.parse(View.DiffRow.schema, "client", "DiffRow", "default")
    let removed = Fixtures.parse(Render.Row.schema, "protocol", "Row", "Removed")
    {
      ...base,
      firstRow: 121,
      lastRow: 122,
      missing: [],
      collapsed: false,
      rows: [
        {...modified, index: 121, row: atLine(modified.row, 9)},
        {index: 122, row: atLine(removed, 10), threads: []},
      ],
    }
  }

  let mount = (~diff as d, ~dispatch) =>
    render(
      <FileDiff
        diff=d
        layout=Split
        focus={Diff({row: 121, side: Head})}
        threads=[]
        draft=None
        pendingRefresh=false
        isOpen=true
        dispatch
      />,
    )

  test("a base-anchored thread hangs on the removed cell, not the added one", () => {
    let {container} = mount(~diff=diff(), ~dispatch=fn())
    let row =
      Element.querySelector(container, "[data-row-index=\"121\"]")
      ->Nullable.toOption
      ->Option.getExn
    expect(Element.querySelector(row, ".cell-left .cell-threads"))->not_->toBeNull
    expect(Element.querySelector(row, ".cell-right .cell-threads"))->toBeNull
  })

  test("clicking a cell focuses that half of the row", () => {
    let dispatch = fn()
    let {container} = mount(~diff=diff(), ~dispatch)
    let row =
      Element.querySelector(container, "[data-row-index=\"121\"]")
      ->Nullable.toOption
      ->Option.getExn
    let left = Element.querySelector(row, ".cell-left")->Nullable.toOption->Option.getExn
    FireEvent.click(left)
    expect(dispatch)->toHaveBeenLastCalledWith(
      Action.SetFocus({focus: Diff({row: 121, side: Base})}),
    )
    let right = Element.querySelector(row, ".cell-right")->Nullable.toOption->Option.getExn
    FireEvent.click(right)
    expect(dispatch)->toHaveBeenLastCalledWith(
      Action.SetFocus({focus: Diff({row: 121, side: Head})}),
    )
  })

  test("dragging down the removed side comments on base lines", () => {
    let dispatch = fn()
    let {container} = mount(~diff=diff(), ~dispatch)
    let leftOf = (index: int) =>
      Element.querySelector(container, "[data-row-index=\"" ++ Int.toString(index) ++ "\"]")
      ->Nullable.toOption
      ->Option.getExn
      ->Element.querySelector(".cell-left")
      ->Nullable.toOption
      ->Option.getExn
    // Starting on the red half of a modified row and crossing into a
    // removed row: the drag keeps growing (it used to stop dead).
    FireEvent.mouseDown(leftOf(121))
    FireEvent.mouseEnter(leftOf(122))
    FireEvent.mouseUp(leftOf(122))
    let calls = mock(dispatch).calls
    let commented = calls->Array.some(
      args =>
        switch args->Array.getUnsafe(0) {
        | Action.CommentLines({side, startLine, endLine}) =>
          side == Base && startLine == 9 && endLine == 10
        | _ => false
        },
    )
    expect(commented)->toBe(true)
  })
})

describe("Scroll.delta", () => {
  // A 400px viewport of 20px rows, nothing painted over the top.
  let container: Scroll.box = {top: 0., bottom: 400., height: 400.}
  let row = (top: float): Scroll.box => {top, bottom: top +. 20., height: 20.}
  let d = (~row as r, ~mode, ~headroom=0.) =>
    Scroll.delta(~container, ~row=r, ~headroom, ~margin=Scroll.scrolloff *. 20., ~mode)

  test("a row well inside the viewport does not scroll", () => {
    expect(d(~row=row(200.), ~mode=Nearest))->toBe(0.)
  })

  test("a row inside the scrolloff margin pushes the view by the shortfall", () => {
    // 3 rows of margin = 60px: a row at 40 is 20px too high, one whose
    // bottom is at 360 is exactly at the limit.
    expect(d(~row=row(40.), ~mode=Nearest))->toBe(-20.)
    expect(d(~row=row(320.), ~mode=Nearest))->toBe(0.)
    expect(d(~row=row(340.), ~mode=Nearest))->toBe(20.)
  })

  test("a row below the fold scrolls just far enough to clear the margin", () => {
    expect(d(~row=row(1000.), ~mode=Nearest))->toBe(1020. -. 340.)
  })

  test("the sticky header is headroom the row must clear", () => {
    // With a 24px header the top limit is 24 + 60 = 84.
    expect(d(~row=row(80.), ~mode=Nearest, ~headroom=24.))->toBe(-4.)
  })

  test("zt/zz/zb put the row at the top, middle and bottom", () => {
    expect(d(~row=row(200.), ~mode=Align(Top)))->toBe(200.)
    expect(d(~row=row(200.), ~mode=Align(Center)))->toBe(200. -. 190.)
    expect(d(~row=row(200.), ~mode=Align(Bottom)))->toBe(220. -. 400.)
    // `z t` on a row inside a file section leaves its sticky header room.
    expect(d(~row=row(200.), ~mode=Align(Top), ~headroom=24.))->toBe(176.)
  })

  test("a viewport too short for the margin still shows the row", () => {
    let tiny: Scroll.box = {top: 0., bottom: 30., height: 30.}
    let delta = Scroll.delta(
      ~container=tiny,
      ~row=row(100.),
      ~headroom=0.,
      ~margin=60.,
      ~mode=Nearest,
    )
    expect(delta)->toBe(120. -. 25.)
  })
})

describe("Scroll.plan", () => {
  let intent = (seq: int): View.ScrollIntent.t => {row: 120, align: Center, seq}

  test("a focused row that has not arrived yet keeps the intent for later", () => {
    // `G` focuses a row whose chunk is still in flight: there is nothing
    // in the DOM to scroll to, and consuming the intent here would lose
    // the reposition for good.
    let (step, seen) = Scroll.plan(
      ~focus=Diff({row: 120, side: Head}),
      ~scroll=Some(intent(4)),
      ~present=false,
      ~seen=None,
    )
    expect(step)->toEqual(Scroll.Skip)
    expect(seen)->toEqual(None)
    // The chunk lands: same intent, now performable.
    let (step, seen) = Scroll.plan(
      ~focus=Diff({row: 120, side: Head}),
      ~scroll=Some(intent(4)),
      ~present=true,
      ~seen,
    )
    expect(step)->toEqual(Scroll.Reposition(Center))
    expect(seen)->toEqual(Some(4))
  })

  test("an intent is performed once; motions after it only follow", () => {
    let (step, seen) = Scroll.plan(
      ~focus=Diff({row: 121, side: Head}),
      ~scroll=Some(intent(4)),
      ~present=true,
      ~seen=Some(4),
    )
    expect(step)->toEqual(Scroll.Follow)
    expect(seen)->toEqual(Some(4))
    // Pressing the chord again is a new instruction.
    let (step, _) = Scroll.plan(
      ~focus=Diff({row: 121, side: Head}),
      ~scroll=Some(intent(5)),
      ~present=true,
      ~seen,
    )
    expect(step)->toEqual(Scroll.Reposition(Center))
  })

  test("a view mounting at an existing intent does not replay it", () => {
    // Leaving a tab and coming back remounts the consumer; starting its
    // watermark at the current intent is what keeps the old `z z` from
    // yanking the view back to where the cursor used to be.
    let (step, _) = Scroll.plan(
      ~focus=Diff({row: 121, side: Head}),
      ~scroll=Some(intent(4)),
      ~present=true,
      ~seen=Some(4),
    )
    expect(step)->toEqual(Scroll.Follow)
  })

  test("list focus scrolls itself, and modal focus not at all", () => {
    let (step, _) = Scroll.plan(~focus=Tree({index: 2}), ~scroll=None, ~present=false, ~seen=None)
    expect(step)->toEqual(Scroll.List)
    let (step, _) = Scroll.plan(~focus=Composer({}), ~scroll=None, ~present=false, ~seen=None)
    expect(step)->toEqual(Scroll.Skip)
  })
})
