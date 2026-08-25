export const VANTA_OBS_PLUGIN_CAPABILITIES = Object.freeze([
  "authenticate_to_vanta",
  "sponsor_cue_dock",
  "stream_health_dock",
  "trigger_proof_markers",
  "send_replay_markers",
  "sync_live_state",
  "sync_archive_status",
]);

const DEFAULT_CONFIG = Object.freeze({
  apiBaseUrl: "http://127.0.0.1:4127",
  userId: "user_creator_owner",
  role: "creator_owner",
  token: "",
  broadcastId: "broadcast_prime_launch",
});

export function normalizedConfig(input = {}) {
  const apiBaseUrl = stringValue(input.apiBaseUrl, DEFAULT_CONFIG.apiBaseUrl).replace(/\/+$/, "");
  return {
    apiBaseUrl,
    userId: stringValue(input.userId, DEFAULT_CONFIG.userId),
    role: stringValue(input.role, DEFAULT_CONFIG.role),
    token: stringValue(input.token, DEFAULT_CONFIG.token),
    broadcastId: stringValue(input.broadcastId, DEFAULT_CONFIG.broadcastId),
  };
}

export function buildHeaders(config = {}) {
  const normalized = normalizedConfig(config);
  return {
    Accept: "application/json",
    "Content-Type": "application/json",
    "X-Vanta-User-Id": normalized.userId,
    "X-Vanta-Role": normalized.role,
    ...(normalized.token ? { Authorization: `Bearer ${normalized.token}` } : {}),
  };
}

export function pluginEndpoints(broadcastId) {
  const id = encodeURIComponent(broadcastId);
  return {
    dashboard: "/api/v1/obs/me/dashboard",
    saveReplay: `/api/v1/obs/me/broadcasts/${id}/replay-buffer/save`,
    runtimeStream: `/api/v1/obs/me/broadcasts/${id}/runtime/stream`,
    postShow: `/api/v1/obs/me/broadcasts/${id}/post-show`,
    triggerCue: (cueId) => `/api/v1/obs/me/live-cues/${encodeURIComponent(cueId)}/trigger`,
    captureProof: (inventoryId) => `/api/v1/obs/me/sponsor/inventory/${encodeURIComponent(inventoryId)}/proof`,
  };
}

export function compactPluginState(dashboard = {}) {
  const runtime = objectValue(dashboard.runtime);
  const health = objectValue(dashboard.health);
  const sponsor = objectValue(dashboard.sponsor);
  const postShow = objectValue(dashboard.post_show);
  const cues = arrayValue(dashboard.cues);
  const inventory = arrayValue(sponsor.inventory_json);
  return {
    streamState: stringValue(runtime.stream_state, "unknown"),
    runtimeState: stringValue(runtime.runtime_state, "unknown"),
    healthStatus: stringValue(health.status, stringValue(runtime.runtime_state, "unknown")),
    viewerPlaybackReady: Boolean(health.viewer_playback_ready),
    readySponsorCues: cues.filter((cue) => stringValue(cue.status) === "ready"),
    proofableInventory: inventory.filter((item) => stringValue(item.status) !== "proof_captured"),
    archiveStatus: stringValue(postShow.status, "not_ready"),
    replayReady: arrayValue(dashboard.replays).some((replay) => stringValue(replay.status) === "clip_draft_ready"),
  };
}

export async function requestVantaJson(config, path, init = {}) {
  const normalized = normalizedConfig(config);
  const response = await fetch(`${normalized.apiBaseUrl}${path}`, {
    ...init,
    headers: {
      ...buildHeaders(normalized),
      ...(init.headers || {}),
    },
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `Vanta request failed: ${response.status}`);
  }
  return response.json();
}

export class VantaObsDock {
  constructor(config = {}) {
    this.config = normalizedConfig(config);
    this.endpoints = pluginEndpoints(this.config.broadcastId);
    this.state = compactPluginState({});
  }

  async refresh() {
    const dashboard = await requestVantaJson(this.config, this.endpoints.dashboard);
    this.state = compactPluginState(dashboard);
    return this.state;
  }

  async triggerSponsorCue(cueId) {
    await requestVantaJson(this.config, this.endpoints.triggerCue(cueId), { method: "POST" });
    return this.refresh();
  }

  async captureProof(inventoryId) {
    await requestVantaJson(this.config, this.endpoints.captureProof(inventoryId), {
      method: "POST",
      body: JSON.stringify({ proof_kind: "media_segment", media_time_seconds: 1 }),
    });
    return this.refresh();
  }

  async saveReplay(durationSeconds = 30) {
    await requestVantaJson(this.config, this.endpoints.saveReplay, {
      method: "POST",
      body: JSON.stringify({
        duration_seconds: durationSeconds,
        label: "OBS companion replay",
        sponsor_proof: true,
      }),
    });
    return this.refresh();
  }
}

export function renderDock(root, dock) {
  root.innerHTML = "";
  const status = document.createElement("section");
  status.className = "vanta-dock-status";
  status.innerHTML = `
    <strong>${escapeHtml(dock.state.streamState)}</strong>
    <span>${escapeHtml(dock.state.healthStatus)}</span>
    <span>Archive ${escapeHtml(dock.state.archiveStatus)}</span>
  `;
  root.append(status);

  const actions = document.createElement("section");
  actions.className = "vanta-dock-actions";
  const replay = actionButton("Replay", () => dock.saveReplay());
  const refresh = actionButton("Refresh", () => dock.refresh());
  actions.append(refresh, replay);
  for (const cue of dock.state.readySponsorCues.slice(0, 3)) {
    actions.append(actionButton(`Cue ${stringValue(cue.label, "Sponsor")}`, () => dock.triggerSponsorCue(String(cue.id))));
  }
  for (const item of dock.state.proofableInventory.slice(0, 2)) {
    actions.append(actionButton(`Proof ${stringValue(item.label, "Spot")}`, () => dock.captureProof(String(item.id))));
  }
  root.append(actions);
}

function actionButton(label, action) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      await action();
    } finally {
      button.disabled = false;
    }
  });
  return button;
}

function objectValue(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

function arrayValue(value) {
  return Array.isArray(value) ? value : [];
}

function stringValue(value, fallback = "") {
  return typeof value === "string" && value.trim() ? value : fallback;
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    "\"": "&quot;",
    "'": "&#39;",
  })[character]);
}

if (typeof document !== "undefined") {
  const params = new URLSearchParams(window.location.search);
  const dock = new VantaObsDock({
    apiBaseUrl: params.get("apiBaseUrl") || undefined,
    token: params.get("token") || undefined,
    userId: params.get("userId") || undefined,
    role: params.get("role") || undefined,
    broadcastId: params.get("broadcastId") || undefined,
  });
  const root = document.querySelector("[data-vanta-obs-dock]");
  if (root) {
    dock.refresh()
      .catch(() => dock.state)
      .finally(() => renderDock(root, dock));
  }
}
