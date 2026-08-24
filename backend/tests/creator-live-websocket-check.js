const BASE = "http://127.0.0.1:8080";
const HOST = "Bearer vanta-local-dev-token";
const SUFFIX = String(Date.now());

async function req(path, { method = "GET", token = null, body = null, headers = {} } = {}) {
  const finalHeaders = { ...headers };
  if (token) finalHeaders.Authorization = token;
  if (body !== null) finalHeaders["Content-Type"] = "application/json";

  const response = await fetch(BASE + path, {
    method,
    headers: finalHeaders,
    body: body === null ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  return [response.status, text ? JSON.parse(text) : null];
}

function createSocketClient(url) {
  const socket = new WebSocket(url);
  const queue = [];
  const waiters = [];

  socket.addEventListener("message", (event) => {
    const payload = JSON.parse(event.data.toString());
    const index = waiters.findIndex((waiter) => waiter.predicate(payload));
    if (index >= 0) {
      const [waiter] = waiters.splice(index, 1);
      waiter.resolve(payload);
      return;
    }
    queue.push(payload);
  });

  socket.addEventListener("close", () => {
    while (waiters.length > 0) {
      const waiter = waiters.shift();
      waiter.reject(new Error("websocket closed before expected event"));
    }
  });

  return {
    socket,
    async open() {
      await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("websocket open timeout")), 4000);
        socket.addEventListener(
          "open",
          () => {
            clearTimeout(timer);
            resolve();
          },
          { once: true },
        );
        socket.addEventListener(
          "error",
          (event) => {
            clearTimeout(timer);
            reject(event.error || new Error("websocket open error"));
          },
          { once: true },
        );
      });
    },
    waitFor(predicate, timeoutMs = 4000) {
      const queuedIndex = queue.findIndex(predicate);
      if (queuedIndex >= 0) {
        const [payload] = queue.splice(queuedIndex, 1);
        return Promise.resolve(payload);
      }

      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          const waiterIndex = waiters.findIndex((waiter) => waiter.resolve === resolve);
          if (waiterIndex >= 0) waiters.splice(waiterIndex, 1);
          reject(new Error("timed out waiting for websocket event"));
        }, timeoutMs);

        waiters.push({
          predicate,
          resolve: (payload) => {
            clearTimeout(timer);
            resolve(payload);
          },
          reject: (error) => {
            clearTimeout(timer);
            reject(error);
          },
        });
      });
    },
  };
}

async function cleanupLiveState() {
  const [liveStatus, live] = await req("/api/v1/creator/me/live", { token: HOST });
  if (liveStatus !== 200) throw new Error("failed to fetch creator live state");
  for (const key of ["currentBroadcast", "pendingBroadcast"]) {
    const broadcast = live[key];
    if (!broadcast) continue;
    const [endedStatus] = await req(`/api/v1/creator/me/broadcasts/${broadcast.id}/end`, {
      method: "POST",
      token: HOST,
    });
    if (endedStatus !== 200) throw new Error(`failed to end ${key}`);
  }
}

async function main() {
  await cleanupLiveState();
  const [originalSettingsStatus, originalSettings] = await req("/api/v1/creator/me/live/settings", {
    token: HOST,
  });
  if (originalSettingsStatus !== 200) {
    throw new Error(`failed to fetch original creator live settings: ${originalSettingsStatus}`);
  }
  const nextScene =
    originalSettings.scenes.find((scene) => scene.id !== originalSettings.activeSceneId)?.id ??
    originalSettings.activeSceneId;
  const targetSettings = {
    subscriberOnly: !originalSettings.subscriberOnly,
    slowModeSeconds: originalSettings.slowModeSeconds === 8 ? 9 : 8,
    autoModLevel: originalSettings.autoModLevel === "strict" ? "standard" : "strict",
    notifyFollowersDefault: !originalSettings.notifyFollowersDefault,
    activeSceneId: nextScene,
    scenes: originalSettings.scenes.map((scene) => ({
      ...scene,
      active: scene.id === nextScene,
    })),
  };

  const socket = createSocketClient(
    "ws://127.0.0.1:8080/ws/creator/live?accessToken=vanta-local-dev-token",
  );
  await socket.open();

  const ready = await socket.waitFor((event) => event.type === "sessionReady");
  if (ready.resumed) throw new Error("creator live websocket should not resume on first connect");

  const initial = await socket.waitFor((event) => event.type === "creatorLiveState");
  if (initial.control.snapshot.profile.id !== initial.runtime.snapshot.profile.id) {
    throw new Error("creator live state profile mismatch");
  }
  if (initial.control.snapshot.currentBroadcast !== null) {
    throw new Error("expected no active broadcast before test");
  }

  socket.socket.close();
  await new Promise((resolve) => setTimeout(resolve, 150));

  const resumedSocket = createSocketClient(
    `ws://127.0.0.1:8080/ws/creator/live?accessToken=vanta-local-dev-token&sessionToken=${encodeURIComponent(
      ready.sessionToken,
    )}`,
  );
  await resumedSocket.open();
  const resumedReady = await resumedSocket.waitFor((event) => event.type === "sessionReady");
  if (!resumedReady.resumed) {
    throw new Error("creator live websocket should resume with a prior session token");
  }
  const resumedInitial = await resumedSocket.waitFor((event) => event.type === "creatorLiveState");
  if (resumedInitial.control.snapshot.profile.id !== resumedInitial.runtime.snapshot.profile.id) {
    throw new Error("resumed creator live state profile mismatch");
  }

  const [ephemeralStatus, ephemeral] = await req("/api/v1/me/sessions", {
    method: "POST",
    token: HOST,
    body: {
      label: `creator-live-revoke-${SUFFIX}`,
      scopes: ["user", "creator", "creator:write", "admin"],
      expiresInDays: 1,
    },
  });
  if (ephemeralStatus !== 200) {
    throw new Error(`failed to create ephemeral creator session: ${ephemeralStatus}`);
  }

  const revokedSocket = createSocketClient(
    `ws://127.0.0.1:8080/ws/creator/live?accessToken=${encodeURIComponent(
      ephemeral.accessToken,
    )}`,
  );
  await revokedSocket.open();
  await revokedSocket.waitFor((event) => event.type === "sessionReady");
  await revokedSocket.waitFor((event) => event.type === "creatorLiveState");

  const [revokeSocketStatus] = await req(`/api/v1/me/sessions/${ephemeral.session.id}`, {
    method: "DELETE",
    token: HOST,
  });
  if (revokeSocketStatus !== 204) {
    throw new Error(`failed to revoke creator websocket session: ${revokeSocketStatus}`);
  }

  await new Promise((resolve) => setTimeout(resolve, 1100));
  if (revokedSocket.socket.readyState !== WebSocket.CLOSED) {
    throw new Error("creator live websocket did not close after session revocation");
  }

  const [settingsStatus] = await req("/api/v1/creator/me/live/settings", {
    method: "PATCH",
    token: HOST,
    body: targetSettings,
  });
  if (settingsStatus !== 200) throw new Error(`update settings failed: ${settingsStatus}`);

  const settingsEvent = await resumedSocket.waitFor(
    (event) =>
      event.type === "creatorLiveState" &&
      event.control.settings.slowModeSeconds === targetSettings.slowModeSeconds &&
      event.control.settings.autoModLevel === targetSettings.autoModLevel &&
      event.control.settings.activeSceneId === targetSettings.activeSceneId,
  );
  if (settingsEvent.control.settings.subscriberOnly !== targetSettings.subscriberOnly) {
    throw new Error("settings update did not propagate to websocket");
  }

  const [startStatus, broadcast] = await req("/api/v1/creator/me/broadcasts/start", {
    method: "POST",
    token: HOST,
    body: {
      title: `Creator live websocket ${SUFFIX}`,
      category: "Tech",
      tags: ["creator", "live", "websocket"],
      isMature: false,
      notifyFollowers: false,
    },
  });
  if (startStatus !== 200) throw new Error(`start broadcast failed: ${startStatus}`);

  const readyEvent = await resumedSocket.waitFor(
    (event) =>
      event.type === "creatorLiveState" &&
      event.control.snapshot.pendingBroadcast?.id === broadcast.id &&
      event.control.snapshot.profile.liveStatus === "starting" &&
      event.control.snapshot.pendingBroadcast?.status === "scheduled",
  );
  if (readyEvent.control.isLive) {
    throw new Error("pending broadcast should not be marked live");
  }

  const streamKey = readyEvent.control.snapshot.profile.streamKey;
  const [connectStatus, connect] = await req("/api/v1/ingest/live/connect", {
    method: "POST",
    body: {
      streamKey,
      protocol: "rtmp",
      ingestServer: "rtmp-us-east-1-primary",
      broadcastId: broadcast.id,
    },
  });
  if (connectStatus !== 200) throw new Error(`connect ingest failed: ${connectStatus}`);

  const liveEvent = await resumedSocket.waitFor(
    (event) =>
      event.type === "creatorLiveState" &&
      event.control.snapshot.currentBroadcast?.id === broadcast.id &&
      event.control.snapshot.profile.liveStatus === "live" &&
      event.runtime.activeSession?.id === connect.session.id,
    6000,
  );
  if (!liveEvent.control.isLive) {
    throw new Error("live ingest connect did not mark broadcast live");
  }

  const [heartbeatStatus] = await req(`/api/v1/ingest/live/${connect.session.id}/heartbeat`, {
    method: "POST",
    body: {
      bitrateKbps: 6400,
      viewers: 1444,
      droppedFrames: 2,
      cpuPercent: 31,
      freeDiskGb: 712.4,
    },
    headers: { "x-ingest-token": connect.ingestToken },
  });
  if (heartbeatStatus !== 200) throw new Error(`heartbeat failed: ${heartbeatStatus}`);

  const heartbeatEvent = await resumedSocket.waitFor(
    (event) =>
      event.type === "creatorLiveState" &&
      event.control.currentViewers === 1444 &&
      event.runtime.activeSession?.bitrateKbps === 6400 &&
      event.runtime.health.samples.at(-1)?.viewers === 1444,
    6000,
  );
  if (heartbeatEvent.runtime.activeSession.droppedFrames !== 2) {
    throw new Error("heartbeat update did not reach creator runtime state");
  }

  const [terminateStatus] = await req(`/api/v1/creator/me/live/ingest/${connect.session.id}/terminate`, {
    method: "POST",
    token: HOST,
    body: { reason: "creator websocket regression cleanup" },
  });
  if (terminateStatus !== 200) throw new Error(`terminate ingest failed: ${terminateStatus}`);

  const offlineEvent = await resumedSocket.waitFor(
    (event) =>
      event.type === "creatorLiveState" &&
      event.runtime.activeSession === null &&
      event.control.snapshot.profile.liveStatus === "offline" &&
      event.control.snapshot.currentBroadcast === null,
    6000,
  );
  if (offlineEvent.control.isLive) {
    throw new Error("terminated ingest should leave creator offline");
  }

  const [restoreStatus] = await req("/api/v1/creator/me/live/settings", {
    method: "PATCH",
    token: HOST,
    body: {
      subscriberOnly: originalSettings.subscriberOnly,
      slowModeSeconds: originalSettings.slowModeSeconds,
      autoModLevel: originalSettings.autoModLevel,
      notifyFollowersDefault: originalSettings.notifyFollowersDefault,
      activeSceneId: originalSettings.activeSceneId,
      scenes: originalSettings.scenes,
    },
  });
  if (restoreStatus !== 200) {
    throw new Error(`failed to restore creator live settings: ${restoreStatus}`);
  }

  resumedSocket.socket.close();
  console.log("creator-live-ws|settings|ingest|offline");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
