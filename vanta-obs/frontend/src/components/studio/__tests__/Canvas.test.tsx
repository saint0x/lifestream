import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ProgramCanvas } from "../Canvas";
import type { ObsRow } from "@/types";

describe("studio ProgramCanvas", () => {
  it("keeps the player window focused while rendering scene source overlays", () => {
    const markup = renderToStaticMarkup(
      <ProgramCanvas
        title="Program"
        live
        scene={row("scene_program", { name: "Host Camera" })}
        instances={[
          row("instance_camera", {
            scene_id: "scene_program",
            source_id: "source_camera",
            visible: true,
            x: 192,
            y: 130,
            width: 1536,
            height: 886,
            order_index: 1,
            z_index: 1,
            opacity: 1,
          }),
        ]}
        sources={[
          row("source_camera", {
            display_name: "Sony FX3",
            source_kind: "camera",
            device_id: "camera_a",
          }),
        ]}
        streams={{}}
      />,
    );

    expect(markup).toContain("aria-label=\"Program canvas feed\"");
    expect(markup).toContain("Program");
    expect(markup).toContain("Host Camera");
    expect(markup).toContain("Sony FX3");
    expect(markup).toContain("camera_a");
  });
});

function row(id: string, fields: Record<string, unknown> = {}): ObsRow {
  return { id, ...fields };
}
