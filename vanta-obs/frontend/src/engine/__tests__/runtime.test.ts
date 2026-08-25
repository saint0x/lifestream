import { describe, expect, it } from "vitest";
import { dashboardFromRuntimeMessage, runtimeSocketTone } from "../runtime";

describe("runtime stream", () => {
  it("extracts dashboard snapshots from runtime websocket payloads", () => {
    const dashboard = dashboardFromRuntimeMessage(JSON.stringify({
      event_kind: "runtime_snapshot",
      dashboard: {
        broadcast: { id: "broadcast_a" },
        runtime: { stream_state: "live" },
      },
    }));

    expect(dashboard?.broadcast.id).toBe("broadcast_a");
  });

  it("rejects malformed runtime messages", () => {
    expect(dashboardFromRuntimeMessage("not-json")).toBeNull();
    expect(dashboardFromRuntimeMessage(JSON.stringify({ event_kind: "runtime_snapshot" }))).toBeNull();
  });

  it("maps socket state into compact badge tone", () => {
    expect(runtimeSocketTone("live")).toBe("hd");
    expect(runtimeSocketTone("offline")).toBe("premium");
    expect(runtimeSocketTone("connecting")).toBe("neutral");
  });
});
