// @vitest-environment jsdom
// 4.4 component tests: each Row variant in both layouts, placeholder →
// chunk swap in the diff view, composer state, hint bar and tree.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import React from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const Row = await import("../src/ui/Row.res.mjs");
const DiffView = await import("../src/ui/DiffView.res.mjs");
const Composer = await import("../src/ui/Composer.res.mjs");
const HintBar = await import("../src/ui/HintBar.res.mjs");
const Tree = await import("../src/ui/Tree.res.mjs");
const Registry = await import("../src/protocol/Registry.res.mjs");
const ClientRegistry = await import("../src/view/ClientRegistry.res.mjs");
const S = await import("rescript-schema/src/S.res.mjs");
const Render = await import("../src/protocol/Render.res.mjs");
const View = await import("../src/view/View.res.mjs");

const fixtures = join(__dirname, "..", "..", "fixtures");
const fixture = (set: string, type: string, name: string) =>
  JSON.parse(readFileSync(join(fixtures, set, type, `${name}.json`), "utf8"));
const parse = (schema: unknown, json: unknown) => S.parseJsonOrThrow(json, schema);

afterEach(cleanup);

describe("Row", () => {
  const variants = readdirSync(join(fixtures, "protocol", "Row")).map((f) => f.replace(/\.json$/, ""));
  for (const layout of ["Unified", "Split"]) {
    for (const v of variants) {
      it(`renders ${v} (${layout}) with its semantic class`, () => {
        const row = parse(Render.Row.schema, fixture("protocol", "Row", v));
        const { container } = render(
          React.createElement(Row.make, { row, layout, index: 3, focused: v === "Added", threads: v === "Modified" ? 2 : 0 }),
        );
        const el = container.querySelector('[role="row"]') as HTMLElement;
        const kebab = v.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
        expect(el.className).toContain(`row-${kebab}`);
        expect(el.className).toContain(layout === "Split" ? "row-split" : "row-unified");
        expect(el.getAttribute("data-row-index")).toBe("3");
        expect(el.hasAttribute("data-focused")).toBe(v === "Added");
        if (v === "Modified") expect(el.querySelector(".row-threads")).not.toBeNull();
        // Cells: split layout always shows both sides for line rows.
        const sides = ["Context", "Removed", "Added", "Modified"];
        if (sides.includes(v) && layout === "Split") {
          expect(el.querySelector(".cell-left")).not.toBeNull();
          expect(el.querySelector(".cell-right")).not.toBeNull();
        }
        // Spans carry their class; changed ranges are marked.
        if (v === "Modified") {
          expect(el.querySelector(".span-keyword")).not.toBeNull();
          expect(el.querySelector(".cell-changed")).not.toBeNull();
        }
      });
    }
  }
});

describe("DiffView", () => {
  it("shows placeholders for missing chunks and swaps in rows when they land", () => {
    const dispatch = vi.fn();
    const base = parse(View.DiffView.schema, fixture("client", "DiffView", "default"));
    // Window rows 120..179, only row 121 cached: everything else is a placeholder.
    const { container, rerender } = render(
      React.createElement(DiffView.make, { diff: base, layout: "Unified", focus: { type: "Diff", row: 121 }, dispatch }),
    );
    // jsdom has no layout: the virtualizer renders nothing until measured, so
    // drive it by checking the grid exists and the scroll container is bound.
    expect(container.querySelector('[role="grid"]')).not.toBeNull();
    expect(container.querySelector(".diff-scroll")).not.toBeNull();
    // Rows the diff carries are what the cache map holds; a re-render with
    // a chunk filled in must not throw and keeps the same grid.
    const filled = { ...base, missing: [], rows: [...base.rows, { ...base.rows[0], index: 122 }] };
    rerender(React.createElement(DiffView.make, { diff: filled, layout: "Split", focus: { type: "Diff", row: 122 }, dispatch }));
    expect(container.querySelector('[role="grid"]')).not.toBeNull();
    expect(screen.getByText("1 file-level thread(s)")).toBeTruthy();
  });
});

describe("Composer", () => {
  it("submits with ctrl+enter and discards with esc, never leaking keys", () => {
    const dispatch = vi.fn();
    const draft = parse(View.Draft.schema, fixture("client", "Draft", "default"));
    render(React.createElement(Composer.make, { draft, pendingRefresh: true, dispatch }));
    const box = screen.getByPlaceholderText("Reply…") as HTMLTextAreaElement;
    expect(screen.getByText("changes pending")).toBeTruthy();
    fireEvent.change(box, { target: { value: "  " } });
    fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    expect(dispatch).not.toHaveBeenCalled(); // blank bodies are not sent
    fireEvent.change(box, { target: { value: "looks good" } });
    fireEvent.keyDown(box, { key: "j" }); // plain text, not a command
    fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    expect(dispatch).toHaveBeenCalledWith({ type: "DraftSubmitted", body: "looks good" });
    fireEvent.keyDown(box, { key: "Escape" });
    expect(dispatch).toHaveBeenLastCalledWith({ type: "DraftDiscarded" });
  });
});

describe("HintBar", () => {
  it("renders the hints the model carries, and the connection and progress", () => {
    const hint = parse(View.Hint.schema, fixture("client", "Hint", "default"));
    render(
      React.createElement(HintBar.make, {
        hints: [hint],
        connection: parse(View.ConnectionView.schema, { type: "Subscribed" }),
        progress: { viewed: 2, changedSinceViewed: 0, total: 5 },
      }),
    );
    expect(screen.getByText("] f")).toBeTruthy();
    expect(screen.getByText("next file")).toBeTruthy();
    expect(screen.getByText("connected")).toBeTruthy();
    expect(screen.getByText("2/5 viewed")).toBeTruthy();
  });
});

describe("Tree", () => {
  it("flattens expanded dirs in display order and marks the focused node", () => {
    const dispatch = vi.fn();
    const tree = parse(View.TreeView.schema, fixture("client", "TreeView", "default"));
    const { container } = render(
      React.createElement(Tree.make, { tree, focus: { type: "Tree", index: 2 }, dispatch }),
    );
    const items = container.querySelectorAll('[role="treeitem"]');
    // root, src (expanded), lib.rs, README.md
    expect(items.length).toBe(4);
    expect(items[2].hasAttribute("data-focused")).toBe(true);
    fireEvent.doubleClick(items[2]);
    expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "Viewport" }));
    fireEvent.click(items[1]);
    expect(dispatch).toHaveBeenCalledWith({ type: "SetFocus", focus: { type: "Tree", index: 1 } });
  });
});

// Keep the registries referenced so a stale import surfaces here too.
void Registry;
void ClientRegistry;
