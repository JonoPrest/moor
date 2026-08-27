// 4.2 adapter tests with a mocked Tauri API: actions go out as
// `invoke("dispatch", {action})`, `view` events apply patches to the store,
// subscribers see every change, and no IPC message exceeds 64 KB across a
// scripted session (every fixture patch, a long comment).

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn(async () => null);
type Handler = (ev: { payload: unknown }) => void;
const handlers: Record<string, Handler[]> = {};
const listen = vi.fn(async (name: string, handler: Handler) => {
  (handlers[name] ??= []).push(handler);
  return () => {
    handlers[name] = handlers[name].filter((h) => h !== handler);
  };
});
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

// Compiled by `pnpm rescript` before the tests run.
const CoreTauri = await import("../src/core/CoreTauri.res.mjs");
const Core = await import("../src/core/Core.res.mjs");
const CoreWasm = await import("../src/core/CoreWasm.res.mjs");

const fixtures = join(__dirname, "..", "..", "fixtures", "client");
const patchFixtures = () =>
  readdirSync(join(fixtures, "ViewPatch"))
    .filter((f) => f.endsWith(".json"))
    .map((f) => JSON.parse(readFileSync(join(fixtures, "ViewPatch", f), "utf8")));
const actionFixture = (name: string) =>
  JSON.parse(readFileSync(join(fixtures, "Action", `${name}.json`), "utf8"));

const IPC_LIMIT = 64 * 1024;
const bytes = (v: unknown) => new TextEncoder().encode(JSON.stringify(v)).length;

const emit = (payload: unknown) => {
  for (const h of handlers["view"] ?? []) h({ payload });
};

const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  invoke.mockClear();
  listen.mockClear();
  for (const k of Object.keys(handlers)) delete handlers[k];
});

describe("CoreTauri", () => {
  it("listens for view events and applies patches to subscribers", async () => {
    const errors: string[] = [];
    const core = CoreTauri.make((e: string) => errors.push(e));
    await tick();
    expect(listen).toHaveBeenCalledWith("view", expect.any(Function));
    const seen: unknown[] = [];
    const unsubscribe = core.subscribe((m: unknown) => seen.push(m));
    // Subscribing delivers the empty model at once.
    expect(seen.length).toBe(1);
    emit(patchFixtures());
    expect(seen.length).toBe(2);
    const model = seen[1] as { progress: { total: number }; focus: { TAG: string } };
    expect(model.progress.total).toBe(12);
    expect(model.focus.TAG).toBe("Diff");
    expect(errors).toEqual([]);
    unsubscribe();
    emit(patchFixtures());
    expect(seen.length).toBe(2);
    // A malformed event is reported, not thrown.
    emit([{ type: "Nope" }]);
    expect(errors.length).toBe(1);
  });

  it("dispatches actions as invoke(dispatch, {action}) and attaches", async () => {
    const core = CoreTauri.make();
    await tick();
    const action = Core.actionOfJson(actionFixture("Viewport"));
    expect(action.TAG).toBe("Ok");
    core.dispatch(action._0);
    await tick();
    expect(invoke).toHaveBeenCalledWith("dispatch", { action: actionFixture("Viewport") });
    core.attach();
    await tick();
    expect(invoke).toHaveBeenCalledWith("attach", {});
  });

  it("keeps every IPC message under 64 KB in a scripted session", async () => {
    const core = CoreTauri.make();
    await tick();
    // Every patch the host can send, one message each.
    for (const p of patchFixtures()) {
      expect(bytes([p])).toBeLessThan(IPC_LIMIT);
    }
    // Typing a long comment: the body travels once, in the submit.
    const long = "x".repeat(10_000);
    const submit = Core.actionOfJson({ type: "DraftSubmitted", body: long });
    core.dispatch(submit._0);
    await tick();
    const [, args] = invoke.mock.calls.at(-1) as unknown as [string, unknown];
    expect(bytes(args)).toBeLessThan(IPC_LIMIT);
    // Scrolling: a viewport action is tiny regardless of file size.
    const scroll = Core.actionOfJson({
      type: "Viewport",
      file: actionFixture("Viewport").file,
      first_row: 99_940,
      last_row: 99_999,
    });
    core.dispatch(scroll._0);
    await tick();
    const [, scrollArgs] = invoke.mock.calls.at(-1) as unknown as [string, unknown];
    expect(bytes(scrollArgs)).toBeLessThan(200);
  });
});

describe("CoreWasm", () => {
  it("serves the empty model and refuses actions loudly", () => {
    const errors: string[] = [];
    const core = CoreWasm.make((e: string) => errors.push(e));
    const seen: unknown[] = [];
    core.subscribe((m: unknown) => seen.push(m));
    expect(seen.length).toBe(1);
    core.dispatch(Core.actionOfJson({ type: "Connect" })._0);
    expect(errors.length).toBe(1);
  });
});
