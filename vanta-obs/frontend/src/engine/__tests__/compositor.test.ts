import { describe, expect, it } from "vitest";
import { canvasDrawableMediaUrl, compositorBackendLabel, renderRects, streamForSource } from "../compositor";
import { sceneGraphItems } from "../graph";
import type { ObsRow } from "@/types";
import type { CaptureKind, CaptureSession } from "../devices";

describe("canvas compositor engine", () => {
  it("converts scene graph percentages into canvas pixels", () => {
    const rects = renderRects(
      [
        {
          id: "item_camera",
          source: source("source_camera", "camera", "Sony FX3"),
          sourceKind: "camera",
          displayName: "Sony FX3",
          leftPct: 25,
          topPct: 10,
          widthPct: 50,
          heightPct: 40,
          opacity: 1.4,
          zIndex: 3,
        },
      ],
      1920,
      1080,
    );

    expect(rects[0]).toMatchObject({
      x: 480,
      y: 108,
      width: 960,
      height: 432,
      opacity: 1,
      zIndex: 3,
    });
  });

  it("maps browser capture source kinds to live stream sessions", () => {
    const camera = session("camera");
    const display = session("display");

    expect(streamForSource("camera", { camera })).toEqual({ kind: "camera", stream: camera.stream });
    expect(streamForSource("screen_capture", { display })).toEqual({ kind: "display", stream: display.stream });
    expect(streamForSource("browser_capture", { camera, display })).toBeNull();
  });

  it("preserves instance z-order before rect generation", () => {
    const items = sceneGraphItems(
      [
        instance("item_top", "source_top", 5),
        instance("item_bottom", "source_bottom", 1),
      ],
      [
        source("source_top", "sponsor_card", "Sponsor"),
        source("source_bottom", "camera", "Camera"),
      ],
    );

    expect(items.map((item) => item.id)).toEqual(["item_bottom", "item_top"]);
    expect(renderRects(items, 1920, 1080).map((rect) => rect.displayName)).toEqual(["Camera", "Sponsor"]);
  });

  it("carries source renderer detail into captured canvas rects", () => {
    const rect = renderRects(
      [
        {
          id: "item_promo",
          source: {
            ...source("source_promo", "promo_code", "Promo"),
            default_settings_json: { promo_code: "NOVA20" },
          },
          sourceKind: "promo_code",
          displayName: "Promo",
          leftPct: 0,
          topPct: 0,
          widthPct: 100,
          heightPct: 100,
          opacity: 1,
          zIndex: 1,
        },
      ],
      1920,
      1080,
    )[0];

    expect(rect).toMatchObject({
      displayName: "NOVA20",
      detail: "promo",
      tone: "promo",
    });
  });

  it("keeps media source URLs on captured canvas rects", () => {
    const rect = renderRects(
      [
        {
          id: "item_video",
          source: {
            ...source("source_video", "vanta_video_asset", "Video"),
            default_settings_json: { media_url: "/media/program.mp4" },
          },
          sourceKind: "vanta_video_asset",
          displayName: "Video",
          leftPct: 0,
          topPct: 0,
          widthPct: 100,
          heightPct: 100,
          opacity: 1,
          zIndex: 1,
        },
      ],
      1920,
      1080,
    )[0];

    expect(rect).toMatchObject({
      tone: "media",
      mediaUrl: "/media/program.mp4",
    });
  });

  it("only allows local, blob, data, and same-origin media into canvas output", () => {
    const base = "https://studio.vanta.test/live";

    expect(canvasDrawableMediaUrl("/asset.mp4", base)).toBe("/asset.mp4");
    expect(canvasDrawableMediaUrl("blob:https://studio.vanta.test/clip", base)).toContain("blob:");
    expect(canvasDrawableMediaUrl("data:image/png;base64,AAA", base)).toContain("data:");
    expect(canvasDrawableMediaUrl("https://studio.vanta.test/media/still.png", base)).toBe("https://studio.vanta.test/media/still.png");
    expect(canvasDrawableMediaUrl("https://cdn.example.test/still.png", base)).toBe("");
  });

  it("reports GPU compositor output as captured runtime output when WebGL is active", () => {
    expect(compositorBackendLabel("webgl_gpu", true)).toBe("gpu capture");
    expect(compositorBackendLabel("webgl_gpu", false)).toBe("gpu preview");
    expect(compositorBackendLabel("canvas_2d", true)).toBe("capture");
  });

  it("flattens nested scene group references into the parent frame", () => {
    const items = sceneGraphItems(
      [
        { ...instance("group_item", "source_group", 1), scene_id: "scene_parent", x: 480, y: 270, width: 960, height: 540 },
        { ...instance("child_camera", "source_camera", 1), scene_id: "scene_child", x: 0, y: 0, width: 960, height: 540 },
      ],
      [
        {
          ...source("source_group", "scene_group", "Guest group"),
          default_settings_json: { scene_id: "scene_child" },
        },
        source("source_camera", "camera", "Nested camera"),
      ],
      "scene_parent",
    );

    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      id: "child_camera",
      displayName: "Nested camera",
      leftPct: 25,
      topPct: 25,
      widthPct: 25,
      heightPct: 25,
    });
  });
});

function source(id: string, source_kind: string, display_name: string): ObsRow {
  return {
    id,
    source_kind,
    display_name,
  };
}

function session(kind: CaptureKind): CaptureSession {
  return {
    kind,
    status: "ready",
    label: kind,
    stream: {} as MediaStream,
    tracks: ["video:live"],
    deviceId: `${kind}_device`,
    reconnectAttempts: 0,
    error: null,
  };
}

function instance(id: string, source_id: string, order_index: number): ObsRow {
  return {
    id,
    source_id,
    order_index,
    visible: 1,
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
    opacity: 1,
  };
}
