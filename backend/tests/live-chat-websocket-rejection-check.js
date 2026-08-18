const BASE = "http://127.0.0.1:8080";
const HOST = "Bearer lifestream-local-dev-token";
const VIEWER = "Bearer lifestream-viewer-token";

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

async function ensureLive() {
  const [streamsStatus, streams] = await req("/api/v1/live/streams");
  if (streamsStatus !== 200) throw new Error("failed to list live streams");

  const existing = streams.find((item) => item.streamer.handle === "deepsaint");
  if (existing) {
    return { streamId: existing.id, created: null };
  }

  const [liveStatus, live] = await req("/api/v1/creator/me/live", { token: HOST });
  if (liveStatus !== 200) throw new Error("failed to fetch creator live snapshot");

  const current = live.currentBroadcast || live.pendingBroadcast;
  let broadcastId = current?.id ?? null;
  if (!broadcastId) {
    const [startStatus, started] = await req("/api/v1/creator/me/broadcasts/start", {
      method: "POST",
      token: HOST,
      body: {
        title: "chat websocket rejection validation",
        category: "Tech",
        tags: ["chat", "websocket", "rejection"],
        isMature: false,
        notifyFollowers: false,
      },
    });
    if (startStatus !== 200) throw new Error("failed to start validation broadcast");
    broadcastId = started.id;
  }

  const [liveAfterStatus, liveAfter] = await req("/api/v1/creator/me/live", { token: HOST });
  if (liveAfterStatus !== 200) throw new Error("failed to refetch creator live snapshot");

  const [connectStatus, connected] = await req("/api/v1/ingest/live/connect", {
    method: "POST",
    body: {
      streamKey: liveAfter.profile.streamKey,
      protocol: "rtmp",
      ingestServer: "rtmp-us-east-1",
      broadcastId,
    },
  });
  if (connectStatus !== 200) throw new Error("failed to connect ingest");

  const [beatStatus] = await req(`/api/v1/ingest/live/${connected.session.id}/heartbeat`, {
    method: "POST",
    body: {
      bitrateKbps: 4200,
      viewers: 32,
      droppedFrames: 0,
      cpuPercent: 18,
      freeDiskGb: 512,
    },
    headers: { "x-ingest-token": connected.ingestToken },
  });
  if (beatStatus !== 200) throw new Error("failed to heartbeat ingest");

  return { streamId: connected.liveStreamId, created: connected };
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

async function main() {
  const live = await ensureLive();
  const streamId = live.streamId;

  const [settingsStatus, originalSettings] = await req("/api/v1/creator/me/live/settings", {
    token: HOST,
  });
  if (settingsStatus !== 200) throw new Error("failed to fetch creator live settings");

  const [updatedStatus] = await req("/api/v1/creator/me/live/settings", {
    method: "PATCH",
    token: HOST,
    body: {
      subscriberOnly: false,
      slowModeSeconds: 30,
      autoModLevel: originalSettings.autoModLevel,
      notifyFollowersDefault: originalSettings.notifyFollowersDefault,
      activeSceneId: originalSettings.activeSceneId,
      scenes: originalSettings.scenes,
    },
  });
  if (updatedStatus !== 200) throw new Error("failed to enable slow mode chat validation");

  const socket = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/${encodeURIComponent(streamId)}?accessToken=${encodeURIComponent(
      VIEWER.replace("Bearer ", ""),
    )}`,
  );
  await socket.open();
  await socket.waitFor((event) => event.type === "sessionReady");
  await socket.waitFor((event) => event.type === "chatHistory");

  socket.socket.send(JSON.stringify({ body: "ws accepted first message", color: "#fafafa" }));
  await socket.waitFor(
    (event) => event.type === "chatMessage" && event.message.body === "ws accepted first message",
  );
  socket.socket.send(JSON.stringify({ body: "ws should reject this", color: "#fafafa" }));
  const rejected = await socket.waitFor((event) => event.type === "chatMessageRejected");
  if (!String(rejected.reason || "").includes("slow mode is active")) {
    throw new Error(`unexpected rejection reason: ${JSON.stringify(rejected)}`);
  }

  socket.socket.close();

  const [restoreStatus] = await req("/api/v1/creator/me/live/settings", {
    method: "PATCH",
    token: HOST,
    body: originalSettings,
  });
  if (restoreStatus !== 200) throw new Error("failed to restore creator live settings");

  if (live.created !== null) {
    const [disconnectStatus] = await req(`/api/v1/ingest/live/${live.created.session.id}/disconnect`, {
      method: "POST",
      headers: { "x-ingest-token": live.created.ingestToken },
    });
    if (disconnectStatus !== 200) throw new Error("failed to disconnect validation ingest");
  }

  console.log("live-chat-websocket-rejection-pass");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
