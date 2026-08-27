// Boundary test (ARCHITECTURE §6.3): every protocol fixture parses with the
// hand-written Sury schema and re-serialises to the same JSON, and every
// fixture directory has a schema. Drift in either direction fails here.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
// Compiled by `pnpm rescript` before the tests run (see CI).
import * as Registry from "../src/protocol/Registry.res.mjs";

const fixtures = join(__dirname, "..", "..", "fixtures", "protocol");

/** Sort object keys recursively so JSON compares byte-for-byte. */
const canonical = (v: unknown): unknown => {
  if (Array.isArray(v)) return v.map(canonical);
  if (v && typeof v === "object") {
    return Object.fromEntries(
      Object.keys(v as object)
        .sort()
        .map((k) => [k, canonical((v as Record<string, unknown>)[k])]),
    );
  }
  return v;
};

const types = readdirSync(fixtures).filter((d) => !d.startsWith("."));

describe("protocol fixtures round-trip through the Sury schemas", () => {
  it("has a schema for every fixture type", () => {
    const missing = types.filter((t) => !Registry.names.includes(t));
    expect(missing).toEqual([]);
  });

  it("has a fixture directory for every schema", () => {
    const extra = Registry.names.filter((n: string) => !types.includes(n));
    expect(extra).toEqual([]);
  });

  for (const type of types) {
    const files = readdirSync(join(fixtures, type)).filter((f) => f.endsWith(".json"));
    for (const file of files) {
      it(`${type}/${file}`, () => {
        const json = JSON.parse(readFileSync(join(fixtures, type, file), "utf8"));
        const result = Registry.roundtrip(type, json);
        // ReScript `result`: {TAG: "Ok", _0} | {TAG: "Error", _0}
        if (result.TAG !== "Ok") throw new Error(result._0);
        expect(JSON.stringify(canonical(result._0))).toBe(JSON.stringify(canonical(json)));
      });
    }
  }
});
