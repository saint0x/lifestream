import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SceneList, SourceList } from "../Rails";
import type { ObsRow } from "@/types";

describe("studio rails", () => {
  it("renders scenes in deterministic order with active program and validation badges", () => {
    const markup = renderToStaticMarkup(
      <SceneList
        scenes={[
          scene("scene_b", 2, "Break", "warning"),
          scene("scene_a", 1, "Program", "ready"),
        ]}
        templates={[row("template_product", { label: "Product demo" })]}
        activeId="scene_a"
        selectedId="scene_b"
        onSelect={() => undefined}
        onDuplicate={() => undefined}
        onMove={() => undefined}
        onDelete={() => undefined}
        onCreateFromTemplate={() => undefined}
      />,
    );

    expect(markup.indexOf("Program")).toBeLessThan(markup.indexOf("Break"));
    expect(markup).toContain("Product demo");
    expect(markup).toContain("PGM");
    expect(markup).toContain("warning");
    expect(markup).toContain("is-selected");
  });

  it("renders source rows with Vanta sync state instead of generic labels", () => {
    const markup = renderToStaticMarkup(
      <SourceList
        selectedId="source_camera"
        onSelect={() => undefined}
        sources={[
          row("source_camera", {
            display_name: "Sony FX3",
            source_kind: "camera",
            device_id: "camera_a",
            source_contract_json: { renderer: "camera", obs_kind: "av_capture_input" },
            source_permission_json: { kind: "camera", required: true },
            source_sync_json: { status: "ready", transport: "native" },
            source_validation_json: { status: "ready" },
          }),
          row("source_chat", {
            display_name: "Live chat",
            source_kind: "chat_overlay",
            default_settings_json: { channel: "prime" },
            source_contract_json: { renderer: "chat_overlay", obs_kind: "browser_source" },
            source_permission_json: { kind: "none", required: false },
            source_sync_json: { status: "ready", transport: "websocket" },
            source_validation_json: { status: "ready" },
          }),
        ]}
      />,
    );

    expect(markup).toContain("Sony FX3");
    expect(markup).toContain("Live chat");
    expect(markup).toContain("camera / camera / native");
    expect(markup).toContain("chat_overlay / inline / websocket");
    expect(markup).toContain("ready");
    expect(markup).toContain("is-selected");
  });
});

function scene(id: string, order: number, name: string, validationStatus: string): ObsRow {
  return row(id, {
    name,
    order_index: order,
    transition_kind: "fade",
    transition_duration_ms: 320,
    scene_validation_json: { status: validationStatus },
  });
}

function row(id: string, fields: Record<string, unknown> = {}): ObsRow {
  return { id, ...fields };
}
