const BASE = "http://127.0.0.1:8080";
const HOST = "Bearer vanta-local-dev-token";
const COLLAB = "Bearer vanta-local-collaborator-token";
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
  let closeResolve;
  let closeReject;
  const closePromise = new Promise((resolve, reject) => {
    closeResolve = resolve;
    closeReject = reject;
  });

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
    closeResolve();
    while (waiters.length > 0) {
      const waiter = waiters.shift();
      waiter.reject(new Error("websocket closed before expected event"));
    }
  });

  socket.addEventListener("error", (event) => {
    if (closeReject) {
      closeReject(event.error || new Error("websocket error before close"));
    }
  });

  return {
    socket,
    waitForClose(timeoutMs = 4000) {
      return Promise.race([
        closePromise,
        new Promise((_, reject) =>
          setTimeout(() => reject(new Error("timed out waiting for websocket close")), timeoutMs),
        ),
      ]);
    },
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

async function expectSocketRejected(url) {
  await new Promise((resolve, reject) => {
    const socket = new WebSocket(url);
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        socket.close();
        reject(new Error("expected websocket rejection"));
      }
    }, 4000);

    socket.addEventListener("open", () => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        socket.close();
        reject(new Error("websocket unexpectedly opened"));
      }
    });

    socket.addEventListener("error", () => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        resolve();
      }
    });

    socket.addEventListener("close", () => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        resolve();
      }
    });
  });
}

async function expectNoSocketEvent(client, predicate, timeoutMs = 600) {
  try {
    const payload = await client.waitFor(predicate, timeoutMs);
    throw new Error(`unexpected websocket event: ${JSON.stringify(payload)}`);
  } catch (error) {
    if (error instanceof Error && error.message === "timed out waiting for websocket event") {
      return;
    }
    throw error;
  }
}

async function cleanupLiveAndCollabs() {
  const [liveStatus, live] = await req("/api/v1/creator/me/live", { token: HOST });
  if (liveStatus !== 200) throw new Error("failed to fetch creator live state");
  for (const key of ["currentBroadcast", "pendingBroadcast"]) {
    const broadcast = live[key];
    if (broadcast) {
      const [endedStatus] = await req(`/api/v1/creator/me/broadcasts/${broadcast.id}/end`, {
        method: "POST",
        token: HOST,
      });
      if (endedStatus !== 200) throw new Error("failed to end existing broadcast");
    }
  }

  const [collabStatus, collabs] = await req("/api/v1/creator/me/live/collabs", { token: HOST });
  if (collabStatus !== 200) throw new Error("failed to list collaboration sessions");
  for (const session of collabs) {
    if (session.status === "pending" || session.status === "active") {
      const [endStatus] = await req(
        `/api/v1/creator/me/live/collabs/sessions/${session.id}/end`,
        { method: "POST", token: HOST },
      );
      if (endStatus !== 200) throw new Error("failed to end existing collaboration session");
    }
  }
}

async function main() {
  await cleanupLiveAndCollabs();

  const [startStatus, broadcast] = await req("/api/v1/creator/me/broadcasts/start", {
    method: "POST",
    token: HOST,
    body: {
      title: `Collab websocket access ${SUFFIX}`,
      category: "Tech",
      tags: ["collaboration", "websocket", "resume"],
      isMature: false,
      notifyFollowers: false,
    },
  });
  if (startStatus !== 200) throw new Error(`start broadcast failed: ${startStatus}`);

  const [createStatus, session] = await req("/api/v1/creator/me/live/collabs/sessions", {
    method: "POST",
    token: HOST,
    body: {
      broadcastId: broadcast.id,
      title: `Websocket control ${SUFFIX}`,
      chatMode: "shared",
      recordingPolicy: "host_archive",
    },
  });
  if (createStatus !== 200) throw new Error(`create collaboration failed: ${createStatus}`);

  const [inviteStatus, invite] = await req(
    `/api/v1/creator/me/live/collabs/sessions/${session.id}/invites`,
    {
      method: "POST",
      token: HOST,
      body: {
        inviteeUserId: "usr-2",
        role: "co_streamer",
        mirrorToGuestChannel: true,
        message: "websocket access validation",
        expiresInMinutes: 30,
      },
    },
  );
  if (inviteStatus !== 200) throw new Error(`invite failed: ${inviteStatus}`);

  const [acceptStatus, participant] = await req(
    `/api/v1/live/collabs/invites/${invite.id}/accept`,
    { method: "POST", token: COLLAB },
  );
  if (acceptStatus !== 200) throw new Error(`accept failed: ${acceptStatus}`);

  const hostSocket = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=vanta-local-dev-token`,
  );
  await hostSocket.open();
  await hostSocket.waitFor((event) => event.type === "sessionReady");
  await hostSocket.waitFor((event) => event.type === "collaborationSnapshot");

  const socket1 = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=vanta-local-collaborator-token`,
  );
  await socket1.open();
  const ready1 = await socket1.waitFor((event) => event.type === "sessionReady");
  if (ready1.resumed) throw new Error("first collaboration socket should not be resumed");
  const snapshot1 = await socket1.waitFor((event) => event.type === "collaborationSnapshot");
  const lastSeq = snapshot1.events.at(-1)?.sequence ?? 0;
  socket1.socket.close();
  await new Promise((resolve) => setTimeout(resolve, 150));

  const socket2 = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=vanta-local-collaborator-token&session_token=${encodeURIComponent(
      ready1.sessionToken,
    )}&after_seq=${lastSeq}`,
  );
  await socket2.open();
  const ready2 = await socket2.waitFor((event) => event.type === "sessionReady");
  if (!ready2.resumed) throw new Error("expected resumed collaboration socket");
  const snapshot2 = await socket2.waitFor((event) => event.type === "collaborationSnapshot");
  if (!Array.isArray(snapshot2.grants)) {
    throw new Error("expected collaboration snapshot grants array");
  }
  const replay = await socket2.waitFor((event) => event.type === "collaborationReplay");
  if (replay.afterSeq !== lastSeq) {
    throw new Error("unexpected collaboration replay cursor");
  }
  if (!Array.isArray(replay.events)) {
    throw new Error("expected collaboration replay events array");
  }
  if (replay.events.some((event) => event.sequence <= lastSeq)) {
    throw new Error("collaboration replay included non-advancing events");
  }

  const [hiddenInviteStatus] = await req(
    `/api/v1/creator/me/live/collabs/sessions/${session.id}/invites`,
    {
      method: "POST",
      token: HOST,
      body: {
        inviteeUserId: "usr-viewer",
        role: "guest",
        mirrorToGuestChannel: false,
        message: "should stay host-only to other collaborators",
        expiresInMinutes: 30,
      },
    },
  );
  if (hiddenInviteStatus !== 200) {
    throw new Error(`hidden invite failed: ${hiddenInviteStatus}`);
  }

  const [ephemeralStatus, ephemeral] = await req("/api/v1/me/sessions", {
    method: "POST",
    token: HOST,
    body: {
      label: `collab-ws-revoke-${SUFFIX}`,
      scopes: ["user", "creator", "creator:write", "admin"],
      expiresInDays: 1,
    },
  });
  if (ephemeralStatus !== 200) {
    throw new Error(`failed to create ephemeral collaboration session: ${ephemeralStatus}`);
  }

  const revokedSocket = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=${encodeURIComponent(
      ephemeral.accessToken,
    )}`,
  );
  await revokedSocket.open();
  await revokedSocket.waitFor((event) => event.type === "sessionReady");
  await revokedSocket.waitFor((event) => event.type === "collaborationSnapshot");

  const [revokeEphemeralStatus] = await req(`/api/v1/me/sessions/${ephemeral.session.id}`, {
    method: "DELETE",
    token: HOST,
  });
  if (revokeEphemeralStatus !== 204) {
    throw new Error(`failed to revoke ephemeral collaboration session: ${revokeEphemeralStatus}`);
  }

  await revokedSocket.waitForClose(5000);

  const [removedStatus] = await req(
    `/api/v1/creator/me/live/collabs/sessions/${session.id}/participants/${participant.id}/remove`,
    {
      method: "POST",
      token: HOST,
      body: { reason: "websocket access revoked" },
    },
  );
  if (removedStatus !== 200) throw new Error(`remove participant failed: ${removedStatus}`);

  const hostEvent = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationEvent" &&
      event.event.eventType === "participant_removed" &&
      event.event.payload.participantId === participant.id,
  );
  if (hostEvent.event.payload.removedAt === undefined) {
    throw new Error("valid host socket did not receive the participant removal event");
  }

  await socket2.waitForClose(4000);

  await expectSocketRejected(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=vanta-local-collaborator-token&session_token=${encodeURIComponent(
      ready1.sessionToken,
    )}&after_seq=${lastSeq}`,
  );

  await expectSocketRejected(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=${encodeURIComponent(
      ephemeral.accessToken,
    )}`,
  );

  hostSocket.socket.close();

  const [endStatus] = await req(`/api/v1/creator/me/broadcasts/${broadcast.id}/end`, {
    method: "POST",
    token: HOST,
  });
  if (endStatus !== 200) throw new Error(`cleanup end broadcast failed: ${endStatus}`);

  console.log("collab-ws|resume|revoked-denied");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
