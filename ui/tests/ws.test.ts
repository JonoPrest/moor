// CoreWs adapter with a mocked WebSocket: attach-first on open, commands
// queue until the socket opens, patch frames apply to the store, and a
// close reconnects with a fresh attach.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  url: string;
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }
  send(text: string) {
    this.sent.push(text);
  }
  open() {
    this.onopen?.();
  }
  message(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) });
  }
  close() {
    this.onclose?.();
  }
}
(globalThis as any).WebSocket = FakeWebSocket;

const CoreWs = await import("../src/core/CoreWs.res.mjs");
const Core = await import("../src/core/Core.res.mjs");

const fixtures = join(__dirname, "..", "..", "fixtures", "client");
const actionFixture = (name: string) =>
  JSON.parse(readFileSync(join(fixtures, "Action", `${name}.json`), "utf8"));
const patchFixture = (name: string) =>
  JSON.parse(readFileSync(join(fixtures, "ViewPatch", `${name}.json`), "utf8"));

const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.useRealTimers();
});

describe("CoreWs", () => {
  it("queues commands until open, then attaches first", async () => {
    const core = CoreWs.make("ws://test", undefined);
    const action = Core.actionOfJson(actionFixture("Viewport"));
    expect(action.TAG).toBe("Ok");
    core.dispatch(action._0);
    const ws = FakeWebSocket.instances[0];
    expect(ws.sent).toEqual([]);
    ws.open();
    expect(ws.sent.length).toBe(2);
    expect(JSON.parse(ws.sent[0])).toEqual({ cmd: "attach" });
    expect(JSON.parse(ws.sent[1])).toEqual({ cmd: "dispatch", action: actionFixture("Viewport") });
  });

  it("applies patch frames to the store", async () => {
    const core = CoreWs.make("ws://test", undefined);
    const ws = FakeWebSocket.instances[0];
    ws.open();
    let model: any;
    core.subscribe((m: any) => (model = m));
    ws.message([patchFixture("Connection")]);
    expect(model.connection.TAG ?? model.connection).toBeDefined();
    expect(JSON.stringify(model.connection)).not.toContain("Disconnected");
  });

  it("reconnects after close and re-attaches", async () => {
    vi.useFakeTimers();
    const core = CoreWs.make("ws://test", () => {});
    const first = FakeWebSocket.instances[0];
    first.open();
    first.close();
    vi.advanceTimersByTime(1500);
    expect(FakeWebSocket.instances.length).toBe(2);
    const second = FakeWebSocket.instances[1];
    second.open();
    expect(JSON.parse(second.sent[0])).toEqual({ cmd: "attach" });
    core.attach();
    expect(second.sent.length).toBe(2);
  });
});
