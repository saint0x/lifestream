import { describe, expect, it } from "vitest";
import { eventBinding, matchingHotkey } from "../hotkeys";
import type { ObsRow } from "@/types";

describe("hotkeys", () => {
  it("normalizes modifier bindings", () => {
    expect(eventBinding({
      altKey: true,
      ctrlKey: false,
      metaKey: false,
      shiftKey: true,
      code: "Digit1",
    })).toBe("Alt+Shift+Digit1");
  });

  it("matches enabled hotkeys by persisted binding", () => {
    const hotkeys: ObsRow[] = [
      { id: "disabled", binding: "Alt+KeyR", enabled: 0 },
      { id: "replay", binding: "Alt+KeyR", enabled: 1 },
    ];
    expect(matchingHotkey(hotkeys, {
      altKey: true,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      code: "KeyR",
    })?.id).toBe("replay");
  });
});
