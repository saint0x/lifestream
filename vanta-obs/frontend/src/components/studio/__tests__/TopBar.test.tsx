import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TopBar } from "../TopBar";
import type { ObsDashboard, ObsRow } from "@/types";

describe("studio TopBar", () => {
  it("renders idle stream controls with replay presets and recording start", () => {
    const markup = renderToStaticMarkup(
      <TopBar
        data={dashboard({ stream_state: "scheduled", recording_state: "pending" })}
        status={null}
        onStart={() => undefined}
        onEnd={() => undefined}
        onRecord={() => undefined}
        onPauseRecord={() => undefined}
        onResumeRecord={() => undefined}
        onStopRecord={() => undefined}
        onReplay={() => undefined}
      />,
    );

    expect(markup).toContain("Prime Launch");
    expect(markup).toContain("15s");
    expect(markup).toContain("30s");
    expect(markup).toContain("60s");
    expect(markup).toContain("Custom");
    expect(markup).toContain("Proof");
    expect(markup).toContain("Record");
    expect(markup).toContain("Go Live");
    expect(markup).not.toContain("Stop Rec");
    expect(markup).not.toContain("End</button>");
  });

  it("renders live recording controls without losing playback and bitrate context", () => {
    const markup = renderToStaticMarkup(
      <TopBar
        data={dashboard({ stream_state: "live", recording_state: "recording" })}
        status="Saving replay"
        onStart={() => undefined}
        onEnd={() => undefined}
        onRecord={() => undefined}
        onPauseRecord={() => undefined}
        onResumeRecord={() => undefined}
        onStopRecord={() => undefined}
        onReplay={() => undefined}
      />,
    );

    expect(markup).toContain("playback ready");
    expect(markup).toContain("6,250 kbps");
    expect(markup).toContain("Saving replay");
    expect(markup).toContain("Pause");
    expect(markup).toContain("Stop Rec");
    expect(markup).toContain("End");
    expect(markup).not.toContain("Go Live");
  });
});

function dashboard(runtime: Record<string, unknown>): ObsDashboard {
  return {
    broadcast: row("broadcast_prime", {
      title: "Prime Launch",
      output_quality_target: "1080p30",
    }),
    collection: row("collection_prime", { name: "Prime Live Kit" }),
    scenes: [],
    scene_templates: [],
    sources: [],
    instances: [],
    audio: [],
    cues: [],
    runtime: row("runtime_prime", {
      runtime_target_json: { protocol: "rtmp" },
      playback_readiness_json: { status: "ready" },
      ...runtime,
    }),
    health: row("health_prime", { status: "ready", bitrate_kbps: 6250 }),
    preflight: row("preflight_prime"),
    replays: [],
    events: [],
    safety: row("safety_prime"),
    moderation: row("moderation_prime"),
    audience: row("audience_prime"),
    engagement: row("engagement_prime"),
    sponsor: row("sponsor_prime"),
    post_show: row("post_show_prime"),
    guests: row("guests_prime"),
    hotkeys: [],
  };
}

function row(id: string, fields: Record<string, unknown> = {}): ObsRow {
  return { id, ...fields };
}
