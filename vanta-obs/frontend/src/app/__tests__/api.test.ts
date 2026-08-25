import { afterEach, describe, expect, it, vi } from "vitest";
import { createRuntimeSourcePlayout, ingestRuntimeSourceFrame, mediaCaptureKind } from "@/app/api";
import type { ObsRow } from "@/types";

describe("media API helpers", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("maps browser and remote sources to runtime browser-surface capture", () => {
    expect(mediaCaptureKind(row("browser_capture"))).toBe("browser_surface");
    expect(mediaCaptureKind(row("remote_contribution"))).toBe("browser_surface");
    expect(mediaCaptureKind(row("display_capture"))).toBe("display");
    expect(mediaCaptureKind(row("window_capture"))).toBe("window");
  });

  it("posts authoritative runtime source-frame payloads with source health counters", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ id: "frame_1", frame_kind: "runtime_browser_surface_png" }),
    })) as unknown as typeof fetch;
    vi.stubGlobal("fetch", fetchMock);

    await ingestRuntimeSourceFrame(
      "capture_browser",
      "data:image/png;base64,AA==",
      "runtime_headless_browser",
      7,
      "browser_source",
      { droppedFrames: 72, reconnectCount: 1, ingestLatencyMs: 1330 },
    );

    const call = vi.mocked(fetchMock).mock.calls[0];
    expect(call).toBeDefined();
    const init = call?.[1];
    expect(String(init?.body)).toContain("\"surface_kind\":\"browser_source\"");
    expect(String(init?.body)).toContain("\"dropped_frames\":72");
    expect(String(init?.body)).toContain("\"reconnect_count\":1");
    expect(String(init?.body)).toContain("\"ingest_latency_ms\":1330");
  });

  it("posts sustained runtime source playout requests", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ id: "artifact_1", artifact_kind: "runtime_browser_surface_playout_mp4" }),
    })) as unknown as typeof fetch;
    vi.stubGlobal("fetch", fetchMock);

    await createRuntimeSourcePlayout("capture_browser", 12, 60);

    const call = vi.mocked(fetchMock).mock.calls[0];
    expect(call?.[0]).toContain("/api/v1/media/capture/sessions/capture_browser/source-playout");
    expect(String(call?.[1]?.body)).toContain("\"frame_count\":12");
    expect(String(call?.[1]?.body)).toContain("\"target_frame_rate\":60");
  });
});

function row(sourceKind: string): ObsRow {
  return {
    id: `source_${sourceKind}`,
    source_kind: sourceKind,
  };
}
