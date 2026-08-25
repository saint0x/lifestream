import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { SceneGraphItem } from "@/engine/graph";
import { sourceRendererModel, SourceRenderer } from "@/engine/renderers";
import type { ObsRow } from "@/types";

const valueSourceKinds = [
  "camera",
  "microphone",
  "desktop_audio",
  "system_audio",
  "screen_capture",
  "display_capture",
  "window_capture",
  "browser_capture",
  "media_file",
  "image",
  "text",
  "lower_third",
  "branded_bumper",
  "pinned_cta",
  "qr_code",
  "promo_code",
  "sponsor_card",
  "countdown_timer",
  "chat_overlay",
  "alert_overlay",
  "guest_feed",
  "remote_contribution",
  "vanta_video_asset",
  "vanta_clip",
  "color_matte",
  "safe_area_guide",
] as const;

describe("source renderers", () => {
  it("keeps every valuable source kind on a concrete renderer path", () => {
    for (const kind of valueSourceKinds) {
      const model = sourceRendererModel(source(kind));

      expect(model.kind).toBe(kind);
      expect(model.tone).not.toBe("generic");
      expect(model.label).not.toBe("");
      expect(model.detail).not.toBe("");
    }
  });

  it("renders video and image asset sources with native media elements when URLs are available", () => {
    const videoMarkup = renderToStaticMarkup(
      <SourceRenderer item={item(source("vanta_clip", { media_url: "/clip.mp4" }))} />,
    );
    const imageMarkup = renderToStaticMarkup(
      <SourceRenderer item={item(source("image", { image_url: "/still.png" }))} />,
    );

    expect(videoMarkup).toContain("<video");
    expect(videoMarkup).toContain("src=\"/clip.mp4\"");
    expect(imageMarkup).toContain("<img");
    expect(imageMarkup).toContain("src=\"/still.png\"");
  });

  it("renders browser sources through a sandboxed no-referrer iframe", () => {
    const markup = renderToStaticMarkup(
      <SourceRenderer item={item(source("browser_capture", { browser_url: "https://example.test/overlay" }))} />,
    );

    expect(markup).toContain("<iframe");
    expect(markup).toContain("sandbox=\"allow-forms allow-presentation allow-scripts\"");
    expect(markup).toContain("referrerPolicy=\"no-referrer\"");
    expect(markup).not.toContain("allow-same-origin");
  });

  it("uses source settings for overlay labels instead of generic source names", () => {
    const promo = sourceRendererModel(source("promo_code", { promo_code: "NOVA20" }));
    const countdown = sourceRendererModel(source("countdown_timer", { seconds: 90 }));
    const lowerThird = sourceRendererModel(source("lower_third", { headline: "Live Demo", subhead: "Main stage" }));

    expect(promo.label).toBe("NOVA20");
    expect(countdown.label).toBe("90s");
    expect(lowerThird.label).toBe("Live Demo");
    expect(lowerThird.detail).toBe("Main stage");
  });
});

function source(kind: string, settings: Record<string, unknown> = {}): ObsRow {
  return {
    id: `source_${kind}`,
    source_kind: kind,
    display_name: kind.replaceAll("_", " "),
    browser_url: kind === "browser_capture" ? "https://vanta.local/live" : undefined,
    device_id: kind.includes("capture") || kind === "camera" ? "device_a" : undefined,
    media_asset_id: kind.includes("asset") || kind.includes("clip") || kind === "media_file" || kind === "image" ? "asset_a" : undefined,
    default_settings_json: settings,
  };
}

function item(sourceRow: ObsRow): SceneGraphItem {
  return {
    id: `instance_${sourceRow.id}`,
    source: sourceRow,
    sourceKind: String(sourceRow.source_kind),
    displayName: String(sourceRow.display_name),
    leftPct: 10,
    topPct: 12,
    widthPct: 30,
    heightPct: 20,
    opacity: 1,
    zIndex: 2,
  };
}
