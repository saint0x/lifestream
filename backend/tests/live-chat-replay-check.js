const BASE = "http://127.0.0.1:8080";
const HOST = "Bearer lifestream-local-dev-token";
const COLLAB = "Bearer lifestream-local-collaborator-token";
const DB_PATH = "/Users/deepsaint/Desktop/lifestream/backend/lifestream.db";

async function req(path, { method = "GET", token = null, body = null, headers = {} } = {}) {
  const finalHeaders = { ...headers };
  if (token) {
    finalHeaders.Authorization = token;
  }
  if (body !== null) {
    finalHeaders["Content-Type"] = "application/json";
  }

  const response = await fetch(BASE + path, {
    method,
    headers: finalHeaders,
    body: body === null ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  const json = text ? JSON.parse(text) : null;
  return [response.status, json];
}

async function ensureLive() {
  const [streamsStatus, streams] = await req("/api/v1/live/streams");
  if (streamsStatus !== 200) {
    throw new Error("failed to list live streams");
  }

  const existing = streams.find((item) => item.streamer.handle === "deepsaint");
  if (existing) {
    return { streamId: existing.id, created: null };
  }

  const [liveStatus, live] = await req("/api/v1/creator/me/live", { token: HOST });
  if (liveStatus !== 200) {
    throw new Error("failed to fetch creator live snapshot");
  }

  const current = live.currentBroadcast || live.pendingBroadcast;
  let broadcastId = current?.id ?? null;
  if (!broadcastId) {
    const [startStatus, started] = await req("/api/v1/creator/me/broadcasts/start", {
      method: "POST",
      token: HOST,
      body: {
        title: "chat replay validation",
        category: "Tech",
        tags: ["chat", "replay"],
        isMature: false,
        notifyFollowers: false,
      },
    });
    if (startStatus !== 200) {
      throw new Error("failed to start replay validation broadcast");
    }
    broadcastId = started.id;
  }

  const [liveAfterStatus, liveAfter] = await req("/api/v1/creator/me/live", {
    token: HOST,
  });
  if (liveAfterStatus !== 200) {
    throw new Error("failed to refetch creator live snapshot");
  }

  const [connectStatus, connected] = await req("/api/v1/ingest/live/connect", {
    method: "POST",
    body: {
      streamKey: liveAfter.profile.streamKey,
      protocol: "rtmp",
      ingestServer: "rtmp-us-east-1",
      broadcastId,
    },
  });
  if (connectStatus !== 200) {
    throw new Error("failed to connect ingest");
  }

  const [beatStatus] = await req(`/api/v1/ingest/live/${connected.session.id}/heartbeat`, {
    method: "POST",
    body: {
      bitrateKbps: 4200,
      viewers: 44,
      droppedFrames: 0,
      cpuPercent: 21,
      freeDiskGb: 512,
    },
    headers: { "x-ingest-token": connected.ingestToken },
  });
  if (beatStatus !== 200) {
    throw new Error("failed to heartbeat ingest");
  }

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
        const timer = setTimeout(
          () => reject(new Error("websocket open timeout")),
          4000,
        );
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
          if (waiterIndex >= 0) {
            waiters.splice(waiterIndex, 1);
          }
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

  const { execFileSync } = await import("node:child_process");
  execFileSync("sqlite3", [
    DB_PATH,
    `DELETE FROM chat_messages WHERE stream_id = '${streamId}'; DELETE FROM chat_stream_cursors WHERE stream_id = '${streamId}';`,
  ]);

  const ws1 = createSocketClient(`ws://127.0.0.1:8080/ws/live/${streamId}`);
  await ws1.open();
  const sessionReady = await ws1.waitFor((event) => event.type === "sessionReady");
  await ws1.waitFor((event) => event.type === "chatHistory");

  const [firstStatus] = await req(`/api/v1/live/streams/${streamId}/chat/messages`, {
    method: "POST",
    token: HOST,
    body: { body: "first replay message" },
  });
  if (firstStatus !== 200) {
    throw new Error(`first message failed: ${firstStatus}`);
  }

  const liveMessage = await ws1.waitFor(
    (event) => event.type === "chatMessage" && event.message.body === "first replay message",
  );
  const firstSequence = liveMessage.message.sequence;

  ws1.socket.close();
  await new Promise((resolve) => setTimeout(resolve, 150));

  const [secondStatus] = await req(`/api/v1/live/streams/${streamId}/chat/messages`, {
    method: "POST",
    token: HOST,
    body: { body: "second replay message" },
  });
  if (secondStatus !== 200) {
    throw new Error(`second message failed: ${secondStatus}`);
  }

  const ws2 = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/${streamId}?session_token=${encodeURIComponent(
      sessionReady.sessionToken,
    )}&after_seq=${firstSequence}`,
  );
  await ws2.open();
  const resumed = await ws2.waitFor((event) => event.type === "sessionReady");
  if (!resumed.resumed) {
    throw new Error("expected resumed session");
  }

  const replay = await ws2.waitFor((event) => event.type === "chatReplay");
  if (replay.afterSeq !== firstSequence) {
    throw new Error("unexpected replay cursor");
  }
  if (
    replay.messages.length !== 1 ||
    replay.messages[0].body !== "second replay message"
  ) {
    throw new Error("unexpected replay payload");
  }
  if (replay.messages[0].sequence <= firstSequence) {
    throw new Error("replay sequence did not advance");
  }

  ws2.socket.close();

  const [moderationStatus, moderationAction] = await req(
    `/api/v1/live/streams/${streamId}/moderation/actions`,
    {
      method: "POST",
      token: HOST,
      body: {
        subjectUserId: "usr-2",
        actionType: "shadowban",
        reason: "socket bootstrap validation",
        durationMinutes: 5,
      },
    },
  );
  if (moderationStatus !== 200) {
    throw new Error(`moderation bootstrap setup failed: ${moderationStatus}`);
  }

  const ws3 = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/${streamId}?access_token=lifestream-local-collaborator-token`,
  );
  await ws3.open();
  await ws3.waitFor((event) => event.type === "sessionReady");
  const moderationBootstrap = await ws3.waitFor(
    (event) =>
      event.type === "moderationAction" &&
      event.action.id === moderationAction.id &&
      event.action.subjectUserId === "usr-2" &&
      event.action.actionType === "shadowban" &&
      event.action.state === "active",
  );
  if (moderationBootstrap.action.reason !== "socket bootstrap validation") {
    throw new Error("unexpected moderation bootstrap payload");
  }
  await ws3.waitFor((event) => event.type === "chatHistory");
  ws3.socket.close();

  const [revokeStatus] = await req(
    `/api/v1/live/streams/${streamId}/moderation/actions/${moderationAction.id}/revoke`,
    {
      method: "POST",
      token: HOST,
    },
  );
  if (revokeStatus !== 200) {
    throw new Error(`moderation cleanup failed: ${revokeStatus}`);
  }

  const [ephemeralStatus, ephemeral] = await req("/api/v1/me/sessions", {
    method: "POST",
    token: HOST,
    body: {
      label: "live-chat-revoke-socket",
      scopes: ["user", "creator", "creator:write", "admin"],
      expiresInDays: 1,
    },
  });
  if (ephemeralStatus !== 200) {
    throw new Error(`ephemeral session create failed: ${ephemeralStatus}`);
  }

  const validHostSocket = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/${streamId}?access_token=lifestream-local-dev-token`,
  );
  await validHostSocket.open();
  await validHostSocket.waitFor((event) => event.type === "sessionReady");
  await validHostSocket.waitFor((event) => event.type === "chatHistory");

  const ws4 = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/${streamId}?access_token=${encodeURIComponent(
      ephemeral.accessToken,
    )}`,
  );
  await ws4.open();
  await ws4.waitFor((event) => event.type === "sessionReady");
  await ws4.waitFor((event) => event.type === "chatHistory");

  const [revokeViewerSocketStatus] = await req(
    `/api/v1/me/sessions/${ephemeral.session.id}`,
    {
      method: "DELETE",
      token: HOST,
    },
  );
  if (revokeViewerSocketStatus !== 204) {
    throw new Error(`viewer websocket revoke failed: ${revokeViewerSocketStatus}`);
  }

  const [postRevokeMessageStatus] = await req(
    `/api/v1/live/streams/${streamId}/chat/messages`,
    {
      method: "POST",
      token: HOST,
      body: { body: "same user valid socket still alive" },
    },
  );
  if (postRevokeMessageStatus !== 200) {
    throw new Error(`post-revoke host message failed: ${postRevokeMessageStatus}`);
  }

  const postRevokeMessage = await validHostSocket.waitFor(
    (event) =>
      event.type === "chatMessage" &&
      event.message.body === "same user valid socket still alive",
  );
  if (postRevokeMessage.message.userHandle !== "deepsaint") {
    throw new Error("valid host live socket did not receive the post-revoke chat event");
  }

  await new Promise((resolve) => setTimeout(resolve, 1100));
  if (ws4.socket.readyState !== WebSocket.CLOSED) {
    throw new Error("viewer websocket did not close after session revocation");
  }

  validHostSocket.socket.close();

  if (live.created) {
    const [disconnectStatus] = await req(
      `/api/v1/ingest/live/${live.created.session.id}/disconnect`,
      {
        method: "POST",
        headers: { "x-ingest-token": live.created.ingestToken },
      },
    );
    if (disconnectStatus !== 200) {
      throw new Error(`disconnect failed: ${disconnectStatus}`);
    }
  }

  console.log("live-chat-replay-pass");
}

main().catch((error) => {
  console.error(error.stack || String(error));
  process.exit(1);
});
