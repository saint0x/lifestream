import { describe, expect, it } from "vitest";
import { sourceBadgeTone, sourceFilterSummary, sourceSummary, sourceSyncState } from "@/engine/sourceSync";

describe("sourceSync", () => {
  it("summarizes ready device-backed source contracts", () => {
    const source = {
      id: "source_camera",
      source_kind: "camera",
      permission_state: "granted",
      source_contract_json: {
        renderer: "device_video",
        permission_kind: "camera",
        local_sync: "media_stream",
        obs_kind: "av_capture_input",
      },
      source_permission_json: { kind: "camera", required: true, state: "granted" },
      source_sync_json: { transport: "media_stream", status: "ready" },
      source_validation_json: { status: "ready", errors: [], warnings: [] },
    };

    expect(sourceSyncState(source)).toMatchObject({
      renderer: "device_video",
      permissionKind: "camera",
      permissionRequired: true,
      transport: "media_stream",
      status: "ready",
      validationStatus: "ready",
      obsKind: "av_capture_input",
    });
    expect(sourceSummary(source)).toBe("device_video / camera / media_stream");
    expect(sourceBadgeTone(source)).toBe("hd");
  });

  it("keeps blocked validation visible to the operator", () => {
    const source = {
      id: "source_browser",
      source_kind: "browser_capture",
      permission_state: "pending",
      source_contract_json: {
        renderer: "browser_frame",
        permission_kind: "network",
        local_sync: "browser_frame",
      },
      source_permission_json: { kind: "network", required: true, state: "pending" },
      source_sync_json: { transport: "browser_frame", status: "pending" },
      source_validation_json: {
        status: "blocked",
        errors: ["http_browser_url_required"],
        warnings: [],
      },
    };

    expect(sourceSyncState(source).issues).toEqual(["http_browser_url_required"]);
    expect(sourceBadgeTone(source)).toBe("premium");
  });

  it("summarizes backend-owned source filter contracts", () => {
    expect(sourceFilterSummary({
      id: "filter_color",
      filter_kind: "color_correction",
      filter_contract_json: { obs_kind: "color_filter" },
    })).toBe("color_correction / color_filter");
  });
});
