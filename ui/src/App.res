// The UI is a renderer over `ViewModel` and a source of `Action`s
// (ARCHITECTURE §6.1). Keys outside text inputs go to the core as chords;
// text inputs stop propagation themselves.

@val @scope("window") external tauriInternals: Nullable.t<'a> = "__TAURI_INTERNALS__"

let chooseCore = (): Core.t =>
  switch tauriInternals->Nullable.toOption {
  | Some(_) => CoreTauri.make()
  | None => CoreWs.make(~url=CoreWs.defaultUrl())
  }

/// Keys the browser would otherwise act on when they reach the keymap.
let swallowed = ["Tab", " ", "ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"]

module KeyEvent = {
  type t
  @get external key: t => string = "key"
  @get external ctrlKey: t => bool = "ctrlKey"
  @get external altKey: t => bool = "altKey"
  @get external shiftKey: t => bool = "shiftKey"
  @get external metaKey: t => bool = "metaKey"
  @get external target: t => Nullable.t<Dom.element> = "target"
  @get external tagName: Dom.element => string = "tagName"
  @send external preventDefault: t => unit = "preventDefault"
  @val @scope("window") external listen: (string, t => unit) => unit = "addEventListener"
  @val @scope("window") external unlisten: (string, t => unit) => unit = "removeEventListener"
}

/// Keys outside text inputs become chords for the core; text inputs handle
/// their own keys and stop propagation.
let onKeyDown = (core: Core.t, ev: KeyEvent.t) => {
  let key = KeyEvent.key(ev)
  let editing = switch KeyEvent.target(ev)->Nullable.toOption {
  | Some(el) => {
      let tag = KeyEvent.tagName(el)
      tag == "INPUT" || tag == "TEXTAREA"
    }
  | None => false
  }
  if !editing {
    switch Keys.ofBrowser({
      key,
      ctrlKey: KeyEvent.ctrlKey(ev),
      altKey: KeyEvent.altKey(ev),
      shiftKey: KeyEvent.shiftKey(ev),
      metaKey: KeyEvent.metaKey(ev),
    }) {
    | Some(chord) => {
        let search = key == "p" && (KeyEvent.ctrlKey(ev) || KeyEvent.metaKey(ev))
        if swallowed->Array.includes(key) || search {
          KeyEvent.preventDefault(ev)
        }
        core.key(chord)
      }
    | None => ()
    }
  }
}

module Shell = {
  @react.component
  let make = (~core: Core.t) => {
    let (model, setModel) = React.useState(() => View.ViewModel.empty)
    React.useEffect0(() => {
      let unsubscribe = core.subscribe(m => setModel(_ => m))
      core.attach()
      Some(unsubscribe)
    })
    // Deep link: open `?review=<id>` once the daemon is subscribed.
    let deepLinked = React.useRef(false)
    React.useEffect1(() => {
      switch (model.connection, CoreWs.reviewParam()) {
      | (Subscribed(_), Some(reviewId)) if !deepLinked.current => {
          deepLinked.current = true
          core.dispatch(OpenReview({reviewId: reviewId}))
        }
      | _ => ()
      }
      None
    }, [model.connection])
    React.useEffect0(() => {
      let handler = ev => onKeyDown(core, ev)
      KeyEvent.listen("keydown", handler)
      Some(() => KeyEvent.unlisten("keydown", handler))
    })
    // Keep the focused list item on screen (the diff scrolls itself).
    React.useEffect1(() => {
      switch model.focus {
      | Diff(_) | Composer(_) | Help(_) => ()
      | ReviewList(_) | Tree(_) | Thread(_) | CommitStepper(_) => Focused.scrollIntoView()
      }
      None
    }, [model.focus])
    // `y`: the clipboard is the shell's; copy here, the core no-ops.
    let dispatch = (action: Action.t) => {
      switch action {
      | CopyPath({path}) => FileDiff.writeText(path)
      | _ => ()
      }
      core.dispatch(action)
    }
    let left = if Array.length(model.tree.roots) > 0 {
      let home =
        model.openReview
        ->Option.flatMap(id => model.reviews->Array.find(r => r.id == id))
        ->Option.flatMap(r => model.workspaces->Array.find(w => w.id == r.workspaceId))
        ->Option.map(w => w.name)
      <Tree tree=model.tree focus=model.focus ?home dispatch />
    } else {
      <ReviewList
        reviews=model.reviews workspaces=model.workspaces connection=model.connection focus=model.focus dispatch
      />
    }
    // The sidebar auto-expands to fit full file names while the tree is
    // focused; otherwise names truncate at the resting width.
    let treeFocused = switch model.focus {
    | Tree(_) => true
    | _ => false
    }
    <main className="app-shell">
      <div className="app-body">
        {model.prefs.sidebarHidden
          ? <button
              type_="button"
              className="sidebar-rail"
              title=?{Chrome.tip(model.chrome, ToggleSidebar)}
              onClick={_ => dispatch(ToggleSidebar({}))}>
              {React.string("⟩")}
            </button>
          : <aside className={"app-left" ++ (treeFocused ? " app-left-expanded" : "")}>
              <div className="app-left-tree"> left </div>
              {switch model.stepper {
              | Some(stepper) => <Stepper stepper focus=model.focus dispatch />
              | None => React.null
              }}
              <button
                type_="button"
                className="sidebar-collapse"
                title=?{Chrome.tip(model.chrome, ToggleSidebar)}
                onClick={_ => dispatch(ToggleSidebar({}))}>
                {React.string("⟨ hide")}
              </button>
            </aside>}
        <div className="app-center">
          <ReviewHeader
            reviews=model.reviews
            workspaces=model.workspaces
            resolvedTargets=model.resolvedTargets
            openReview=model.openReview
            prefs=model.prefs
            scope=model.scope
            stepper=?model.stepper
            chrome=model.chrome
            connection=model.connection
            progress=model.progress
            dispatch
          />
          <Tabs
            tab=model.tab
            fileCount=model.progress.total
            threadCount={Array.length(model.threads)}
            chrome=model.chrome
            dispatch
          />
          {switch model.tab {
          | FilesChanged =>
            <>
              {switch model.tree.search {
              | Some(search) => <SearchBox search dispatch />
              | None => React.null
              }}
              {Array.length(model.diffs) > 0
                ? <div className="diff-stack">
                    {model.diffs
                    ->Array.map(diff =>
                      <FileDiff
                        key={diff.file.repoId ++ diff.file.path}
                        diff
                        layout=model.prefs.layout
                        focus=model.focus
                        threads=model.threads
                        draft=model.draft
                        pendingRefresh=model.pendingRefresh
                        isOpen={switch model.diff {
                        | Some(open_) => open_.file == diff.file
                        | None => false
                        }}
                        dispatch
                      />
                    )
                    ->React.array}
                  </div>
                : <div className="diff-empty"> {React.string("No changed files")} </div>}
              {switch model.draft {
              // A reply renders inline in its thread's card; only a fresh
              // comment uses the bottom composer.
              | Some({replyTo: None} as draft) =>
                <Composer draft pendingRefresh=model.pendingRefresh dispatch />
              | Some({replyTo: Some(_)}) | None => React.null
              }}
            </>
          | Conversation =>
            // Every thread of the review, chronologically (GitHub-style):
            // file/line threads and review-level ones together.
            <Threads
              title="Conversation" threads=model.threads focus=model.focus indexOffset=0 dispatch
            />
          | Browse =>
            <>
              <BrowseBar
                browseRef=model.browseRef
                repoId={model.resolvedTargets->Array.get(0)->Option.map(t => t.repoId)}
                dispatch
              />
              {switch model.tree.search {
              | Some(search) => <SearchBox search dispatch />
              | None => React.null
              }}
              {switch model.diff {
              | Some(diff) => <DiffView diff layout=model.prefs.layout focus=model.focus dispatch />
              | None => <div className="diff-empty"> {React.string("Open a file")} </div>
              }}
              {switch model.draft {
              | Some(draft) => <Composer draft pendingRefresh=model.pendingRefresh dispatch />
              | None => React.null
              }}
            </>
          }}
        </div>
      </div>
      <WhichKey
        pendingKeys=model.pendingKeys pendingLabel=model.pendingLabel hints=model.hints
      />
      <HintBar
        hints=model.hints
        pendingKeys=model.pendingKeys
        pendingLabel=?model.pendingLabel
        mode=model.mode
        leader=model.leader
        focusName={switch model.focus {
        | Tree(_) => "TREE"
        | Diff(_) => "DIFF"
        | Thread(_) => "THREAD"
        | ReviewList(_) => "REVIEWS"
        | CommitStepper(_) => "COMMITS"
        | Composer(_) | Help(_) => ""
        }}
        connection=model.connection
        progress=model.progress
      />
      {switch model.help {
      | Some(help) => <HelpOverlay help dispatch />
      | None => React.null
      }}
      {model.contentSearch != None || model.actionPalette
        ? <Palette
            contentSearch=model.contentSearch
            actionPalette=model.actionPalette
            chrome=model.chrome
            dispatch
          />
        : React.null}
    </main>
  }
}

@react.component
let make = () => {
  let core = React.useMemo0(chooseCore)
  <Shell core />
}
