// Every `SpanClass`, `Row` kind and `Cell` side the protocol can produce
// has a semantic class in app.css (ARCHITECTURE §6.6). The sets come from
// the protocol fixtures, so a new variant on the Rust side fails here until
// the stylesheet knows it.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = join(__dirname, "..", "..");
const fixtures = join(root, "fixtures", "protocol");
const css = readFileSync(join(__dirname, "..", "src", "styles", "app.css"), "utf8");

/** `HunkHeader` → `hunk-header`. */
const kebab = (name: string): string =>
  name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();

const fixtureNames = (dir: string): string[] =>
  readdirSync(join(fixtures, dir))
    .filter((f) => f.endsWith(".json"))
    .map((f) => f.replace(/\.json$/, ""));

/** Class selectors defined anywhere in the stylesheet. */
const definedClasses = (): Set<string> => {
  const out = new Set<string>();
  for (const m of css.matchAll(/\.([a-z][a-z0-9-]*)/g)) out.add(m[1]);
  return out;
};

describe("app.css covers the render model", () => {
  const classes = definedClasses();

  it("has a class per SpanClass", () => {
    const spans = fixtureNames("SpanClass");
    expect(spans.length).toBeGreaterThan(0);
    for (const s of spans) expect(classes, `span-${kebab(s)}`).toContain(`span-${kebab(s)}`);
  });

  it("has a class per Row kind", () => {
    const rows = fixtureNames("Row");
    expect(rows.length).toBeGreaterThan(0);
    for (const r of rows) expect(classes, `row-${kebab(r)}`).toContain(`row-${kebab(r)}`);
  });

  it("has a class per Cell side", () => {
    // Sides are the `Row` fields that hold a `Cell`.
    const sides = new Set<string>();
    for (const r of fixtureNames("Row")) {
      const row = JSON.parse(readFileSync(join(fixtures, "Row", `${r}.json`), "utf8"));
      for (const k of Object.keys(row)) {
        if (row[k] && typeof row[k] === "object" && "line_no" in row[k]) sides.add(k);
      }
    }
    expect([...sides].sort()).toEqual(["left", "right"]);
    for (const s of sides) expect(classes).toContain(`cell-${s}`);
  });

  it("defines the dark theme for every colour token", () => {
    const light = new Set([...css.matchAll(/--color-([a-z0-9-]+):/g)].map((m) => m[1]));
    const dark = css.slice(css.indexOf("prefers-color-scheme: dark"));
    for (const token of light) {
      if (token === "diff-context-bg" || token === "syn-other") continue; // transparent / inherit
      expect(dark, `--color-${token} in dark`).toContain(`--color-${token}:`);
    }
  });
});
