import { describe, expect, it } from "vitest";
import { clampReplayDuration, replayDurationFromPreset } from "@/engine/replay";

describe("replay options", () => {
  it("uses fixed save-length presets", () => {
    expect(replayDurationFromPreset(15, 42)).toBe(15);
    expect(replayDurationFromPreset(30, 42)).toBe(30);
    expect(replayDurationFromPreset(60, 42)).toBe(60);
  });

  it("keeps custom save lengths within the backend replay contract", () => {
    expect(replayDurationFromPreset("custom", 4)).toBe(5);
    expect(replayDurationFromPreset("custom", 301)).toBe(300);
    expect(replayDurationFromPreset("custom", Number.NaN)).toBe(30);
    expect(clampReplayDuration(44.6)).toBe(45);
  });
});
