import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Activity } from "lucide-react";
import { Panel } from "../Panel";

describe("studio Panel", () => {
  it("starts collapsed when requested while preserving the summary", () => {
    const markup = renderToStaticMarkup(
      <Panel title="Runtime" icon={<Activity />} summary={<strong>live</strong>} defaultCollapsed>
        <button type="button">Hidden action</button>
      </Panel>,
    );

    expect(markup).toContain("is-collapsed");
    expect(markup).toContain("aria-expanded=\"false\"");
    expect(markup).toContain("Expand Runtime");
    expect(markup).toContain("<strong>live</strong>");
    expect(markup).toContain("hidden=\"\"");
  });

  it("starts collapsed by default so dense studio sections stay scannable", () => {
    const markup = renderToStaticMarkup(
      <Panel title="Scenes" icon={<Activity />} summary={<strong>5</strong>}>
        <button type="button">Scene row</button>
      </Panel>,
    );

    expect(markup).toContain("is-collapsed");
    expect(markup).toContain("aria-expanded=\"false\"");
    expect(markup).toContain("Expand Scenes");
    expect(markup).toContain("hidden=\"\"");
  });
});
