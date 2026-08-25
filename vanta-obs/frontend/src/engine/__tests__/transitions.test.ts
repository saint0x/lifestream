import { describe, expect, it } from "vitest";
import {
  transitionLabel,
  transitionPhaseSummary,
  transitionPlanFromPreview,
  transitionRenderer,
} from "../transitions";

describe("transitions", () => {
  it("summarizes dip to black phases", () => {
    const plan = {
      kind: "dip_to_black",
      renderer: "dip_color",
      phases: [
        { action: "fade_out_to_black", duration_ms: 250 },
        { action: "swap_program_under_black", duration_ms: 0 },
        { action: "fade_in_from_black", duration_ms: 250 },
      ],
    };

    expect(transitionLabel(plan)).toBe("Dip Black");
    expect(transitionRenderer(plan)).toBe("dip_color");
    expect(transitionPhaseSummary(plan)).toEqual(["Black Out 250ms", "Swap 0ms", "Black In 250ms"]);
  });

  it("summarizes stinger overlay cut point", () => {
    const preview = {
      id: "transition_preview_scene",
      transition: {
        kind: "stinger",
        renderer: "stinger_overlay",
        phases: [
          { action: "play_stinger_overlay", duration_ms: 900 },
          { action: "swap_program_at_cut_point", duration_ms: 0 },
          { action: "clear_stinger_overlay", duration_ms: 0 },
        ],
      },
    };

    const plan = transitionPlanFromPreview(preview);
    expect(transitionLabel(plan)).toBe("Stinger");
    expect(transitionPhaseSummary(plan)).toEqual(["Overlay 900ms", "Cut Point 0ms", "Clear 0ms"]);
  });
});
