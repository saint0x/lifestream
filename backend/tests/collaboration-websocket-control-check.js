const BASE = "http://127.0.0.1:8080";
const HOST = "Bearer lifestream-local-dev-token";
const COLLAB = "Bearer lifestream-local-collaborator-token";
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
      title: `Collab websocket control ${SUFFIX}`,
      category: "Tech",
      tags: ["collaboration", "websocket", "control"],
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

  const [revokedInviteStatus, revokedInvite] = await req(
    `/api/v1/creator/me/live/collabs/sessions/${session.id}/invites`,
    {
      method: "POST",
      token: HOST,
      body: {
        inviteeUserId: "usr-2",
        role: "guest",
        mirrorToGuestChannel: false,
        message: "pending invite revoke validation",
        expiresInMinutes: 30,
      },
    },
  );
  if (revokedInviteStatus !== 200) {
    throw new Error(`revoked invite create failed: ${revokedInviteStatus}`);
  }

  const [inviteStatus, invite] = await req(
    `/api/v1/creator/me/live/collabs/sessions/${session.id}/invites`,
    {
      method: "POST",
      token: HOST,
      body: {
        inviteeUserId: "usr-2",
        role: "co_streamer",
        mirrorToGuestChannel: true,
        message: "websocket control validation",
        expiresInMinutes: 30,
      },
    },
  );
  if (inviteStatus !== 200) throw new Error(`invite failed: ${inviteStatus}`);

  const [acceptStatus, participant] = await req(
    `/api/v1/live/collabs/invites/${invite.id}/accept`,
    { method: "POST", token: COLLAB },
  );
  if (acceptStatus !== 200 || participant.state !== "backstage") {
    throw new Error(`accept failed: ${acceptStatus}`);
  }

  const hostSocket = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=lifestream-local-dev-token`,
  );
  const guestSocket = createSocketClient(
    `ws://127.0.0.1:8080/ws/live/collabs/${session.id}?access_token=lifestream-local-collaborator-token`,
  );
  await hostSocket.open();
  await guestSocket.open();
  await hostSocket.waitFor((event) => event.type === "sessionReady");
  await hostSocket.waitFor((event) => event.type === "collaborationSnapshot");
  await guestSocket.waitFor((event) => event.type === "sessionReady");
  await guestSocket.waitFor((event) => event.type === "collaborationSnapshot");

  guestSocket.socket.send(
    JSON.stringify({
      type: "requestStateChange",
      state: "live",
    }),
  );
  const guestAck = await guestSocket.waitFor(
    (event) =>
      event.type === "collaborationCommandAccepted" &&
      event.commandType === "requestStateChange",
  );
  if (guestAck.participantId !== participant.id || guestAck.state !== "live") {
    throw new Error("guest state request was not acknowledged correctly");
  }

  const hostSawRequest = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationEvent" &&
      event.event.eventType === "participant_state_requested",
  );
  if (hostSawRequest.event.payload.participantId !== participant.id) {
    throw new Error("host did not receive the correct state request event");
  }

  guestSocket.socket.send(
    JSON.stringify({
      type: "updateParticipant",
      participantId: participant.id,
      state: "live",
    }),
  );
  const guestRejected = await guestSocket.waitFor(
    (event) =>
      event.type === "collaborationCommandRejected" &&
      event.commandType === "updateParticipant",
  );
  if (!guestRejected.reason.includes("only the collaboration host")) {
    throw new Error("guest update rejection reason was unexpected");
  }

  hostSocket.socket.send(
    JSON.stringify({
      type: "revokeInvite",
      inviteId: revokedInvite.id,
    }),
  );
  const revokeInviteAck = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationCommandAccepted" &&
      event.commandType === "revokeInvite",
  );
  if (revokeInviteAck.state !== "revoked") {
    throw new Error("host invite revoke was not acknowledged correctly");
  }

  const revokedInviteEvent = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationEvent" &&
      event.event.eventType === "invite_revoked" &&
      event.event.payload.inviteId === revokedInvite.id,
  );
  if (revokedInviteEvent.event.payload.reason !== "host_revoked") {
    throw new Error("invite revoke event did not include the host revoke reason");
  }

  const [revokedInviteRuntimeStatus, revokedInviteRuntime] = await req(
    `/api/v1/me/live/collabs/sessions/${session.id}/events`,
    { token: HOST },
  );
  if (revokedInviteRuntimeStatus !== 200) {
    throw new Error(`host event feed lookup failed: ${revokedInviteRuntimeStatus}`);
  }
  if (
    !revokedInviteRuntime.some(
      (event) =>
        event.eventType === "invite_revoked" &&
        event.payload.inviteId === revokedInvite.id &&
        event.payload.reason === "host_revoked",
    )
  ) {
    throw new Error("host event feed did not persist the invite revoke event");
  }

  hostSocket.socket.send(
    JSON.stringify({
      type: "updateParticipant",
      participantId: participant.id,
      state: "live",
      publishToHost: true,
      mirrorToGuestChannel: true,
      canSpeakInChat: true,
    }),
  );
  const hostAck = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationCommandAccepted" &&
      event.commandType === "updateParticipant",
  );
  if (hostAck.participantId !== participant.id || hostAck.state !== "live") {
    throw new Error("host participant update was not acknowledged correctly");
  }

  const updateEvent = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationEvent" &&
      event.event.eventType === "participant_updated" &&
      event.event.payload.participantId === participant.id,
  );
  if (updateEvent.event.payload.state !== "live") {
    throw new Error("participant update event did not move the participant live");
  }

  const [runtimeStatus, runtime] = await req(`/api/v1/me/live/collabs/sessions/${session.id}/runtime`, {
    token: COLLAB,
  });
  if (runtimeStatus !== 200) throw new Error(`runtime lookup failed: ${runtimeStatus}`);
  const updatedParticipant = runtime.session.participants.find((item) => item.id === participant.id);
  if (!updatedParticipant || updatedParticipant.state !== "live") {
    throw new Error("runtime state did not reflect the live participant update");
  }

  hostSocket.socket.send(
    JSON.stringify({
      type: "issueMirrorGrant",
      participantId: participant.id,
    }),
  );
  const issueMirrorGrantAck = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationCommandAccepted" &&
      event.commandType === "issueMirrorGrant",
  );
  if (issueMirrorGrantAck.participantId !== participant.id) {
    throw new Error("mirror grant issue acknowledgement targeted the wrong participant");
  }

  const issuedGrantEvent = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationEvent" &&
      event.event.eventType === "mirror_grant_issued" &&
      event.event.participantId === participant.id,
  );
  if (issuedGrantEvent.event.payload.guestCreatorId !== "crt-atlas") {
    throw new Error("mirror grant issue event targeted the wrong guest creator");
  }

  const [grantsStatus, grants] = await req(`/api/v1/me/live/collabs/sessions/${session.id}/grants`, {
    token: COLLAB,
  });
  if (grantsStatus !== 200) throw new Error(`grant list failed: ${grantsStatus}`);
  if (!Array.isArray(grants) || grants.length !== 1 || grants[0].state !== "issued") {
    throw new Error("guest grant list did not surface the issued mirror grant");
  }

  hostSocket.socket.send(
    JSON.stringify({
      type: "revokeMirrorGrants",
      participantId: participant.id,
    }),
  );
  const revokeMirrorGrantAck = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationCommandAccepted" &&
      event.commandType === "revokeMirrorGrants",
  );
  if (revokeMirrorGrantAck.participantId !== participant.id) {
    throw new Error("mirror grant revoke acknowledgement targeted the wrong participant");
  }

  const revokedGrantEvent = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationEvent" &&
      event.event.eventType === "mirror_grant_revoked" &&
      event.event.payload.participantId === participant.id,
  );
  if (revokedGrantEvent.event.payload.reason !== "host_revoked") {
    throw new Error("mirror grant revoke event did not include the host revoke reason");
  }

  const [revokedGrantsStatus, revokedGrants] = await req(
    `/api/v1/me/live/collabs/sessions/${session.id}/grants`,
    { token: COLLAB },
  );
  if (revokedGrantsStatus !== 200) {
    throw new Error(`revoked grant list failed: ${revokedGrantsStatus}`);
  }
  if (!Array.isArray(revokedGrants) || revokedGrants.length !== 1 || revokedGrants[0].state !== "revoked") {
    throw new Error("guest grant list did not surface the revoked mirror grant");
  }

  hostSocket.socket.send(
    JSON.stringify({
      type: "removeParticipant",
      participantId: participant.id,
    }),
  );
  const removeParticipantAck = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationCommandAccepted" &&
      event.commandType === "removeParticipant",
  );
  if (removeParticipantAck.participantId !== participant.id || removeParticipantAck.state !== "removed") {
    throw new Error("participant removal acknowledgement was incorrect");
  }

  const removedParticipantEvent = await hostSocket.waitFor(
    (event) =>
      event.type === "collaborationEvent" &&
      event.event.eventType === "participant_removed" &&
      event.event.payload.participantId === participant.id,
  );
  if (removedParticipantEvent.event.payload.reason !== "host_removed") {
    throw new Error("participant removal event did not include the host removal reason");
  }

  hostSocket.socket.close();
  guestSocket.socket.close();

  const [endStatus] = await req(`/api/v1/creator/me/broadcasts/${broadcast.id}/end`, {
    method: "POST",
    token: HOST,
  });
  if (endStatus !== 200) throw new Error(`cleanup end broadcast failed: ${endStatus}`);

  console.log("collab-ws|control|host-guest-authority");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
