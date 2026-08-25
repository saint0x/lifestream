import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("../src/pages/studio/StudioPage.tsx", import.meta.url), "utf8");

function railSource(side: "left" | "right"): string {
  const className = `obs-rail obs-rail--${side}`;
  const start = source.indexOf(`<aside className="${className}">`);
  const nextAside = source.indexOf("<aside", start + 1);
  const end = nextAside === -1 ? source.indexOf("</aside>", start) : source.lastIndexOf("</aside>", nextAside);
  return source.slice(start, end);
}

function panelCount(markup: string): number {
  return (markup.match(/<(?:Panel|[A-Z][A-Za-z]+Panel|AudioMixer|Inspector)\b/g) ?? []).length;
}

describe("studio rail layout", () => {
  it("keeps collapsed sections balanced around the player", () => {
    const left = railSource("left");
    const right = railSource("right");

    expect(left).toContain("<AudioMixer");
    expect(left).toContain("<HotkeysPanel");
    expect(left).toContain("<CuePanel");
    expect(right).toContain("<ChannelPanel");
    expect(right).toContain("<GuestsPanel");
    expect(right).toContain("<HealthPanel");
    expect(right).toContain("<CompatibilityPanel");
    expect(right).not.toContain("<AudioMixer");
    expect(right).not.toContain("<HotkeysPanel");
    expect(right).not.toContain("<CuePanel");
    expect(Math.abs(panelCount(left) - panelCount(right))).toBeLessThanOrEqual(1);
  });

  it("does not opt dense studio panels open by default", () => {
    expect(source).not.toContain("defaultCollapsed={false}");
    expect(source).not.toContain('defaultCollapsed="false"');
  });
});
