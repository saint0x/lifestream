import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import {
  VANTA_OBS_PLUGIN_CAPABILITIES,
  buildHeaders,
  compactPluginState,
  pluginEndpoints,
} from "../../plugin/obs/dock/vanta-obs-dock.js";

const pluginRoot = resolve(__dirname, "../../plugin/obs");

describe("Vanta OBS Companion package", () => {
  it("declares only the value-filtered OBS companion capabilities", () => {
    const manifest = JSON.parse(readFileSync(resolve(pluginRoot, "manifest.json"), "utf8"));

    expect(manifest.capabilities).toEqual([...VANTA_OBS_PLUGIN_CAPABILITIES]);
    expect(manifest.non_goals).toContain("full_vanta_live_studio_ui");
    expect(manifest.non_goals).toContain("generic_obs_plugin_host");
    expect(manifest.entry).toBe("dock/index.html");
    expect(manifest.script).toBe("vanta_obs_bridge.lua");
  });

  it("builds authenticated Vanta requests for cue, proof, replay, health, and archive sync", () => {
    const headers = buildHeaders({
      token: "token_test",
      userId: "creator_plugin",
      role: "producer",
    });
    const endpoints = pluginEndpoints("broadcast_prime_launch");

    expect(headers.Authorization).toBe("Bearer token_test");
    expect(headers["X-Vanta-User-Id"]).toBe("creator_plugin");
    expect(headers["X-Vanta-Role"]).toBe("producer");
    expect(endpoints.dashboard).toBe("/api/v1/obs/me/dashboard");
    expect(endpoints.saveReplay).toContain("/replay-buffer/save");
    expect(endpoints.postShow).toContain("/post-show");
    expect(endpoints.triggerCue("cue_1")).toContain("/live-cues/cue_1/trigger");
    expect(endpoints.captureProof("inventory_1")).toContain("/sponsor/inventory/inventory_1/proof");
  });

  it("summarizes dashboard state into a compact OBS dock model", () => {
    const state = compactPluginState({
      runtime: { stream_state: "live", runtime_state: "healthy" },
      health: { status: "green", viewer_playback_ready: true },
      cues: [{ id: "cue_1", status: "ready", label: "Sponsor" }],
      sponsor: { inventory_json: [{ id: "inventory_1", status: "ready", label: "Nova" }] },
      replays: [{ status: "clip_draft_ready" }],
      post_show: { status: "packaging" },
    });

    expect(state.streamState).toBe("live");
    expect(state.healthStatus).toBe("green");
    expect(state.readySponsorCues).toHaveLength(1);
    expect(state.proofableInventory).toHaveLength(1);
    expect(state.replayReady).toBe(true);
    expect(state.archiveStatus).toBe("packaging");
  });
});
