#!/usr/bin/env node

const API_BASE = process.env.VANTA_API_BASE || "https://api-production-4becb.up.railway.app";
const DUMMY = {
  slug: "smoke-missing",
  id: "smoke-missing",
  userId: "guest-smoke-missing",
  creatorId: "creator-smoke-missing",
  streamId: "stream-smoke-missing",
  actionId: "action-smoke-missing",
  reportId: "report-smoke-missing",
  notificationId: "notification-smoke-missing",
  purchaseId: "purchase-smoke-missing",
  tierId: "tier-smoke-missing",
  sessionId: "session-smoke-missing",
  socketId: "socket-smoke-missing",
  inviteId: "invite-smoke-missing",
  participantId: "participant-smoke-missing",
  grantId: "grant-smoke-missing",
  deliveryId: "delivery-smoke-missing",
  jobId: "job-smoke-missing",
  uploadId: "upload-smoke-missing",
  contentId: "content-smoke-missing",
};

const json = (body = {}) => ({
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

const ok = (...codes) => new Set(codes);
const readOnlyOk = ok(200, 204, 400, 401, 403, 404, 405, 422);
const mutationOk = ok(200, 201, 202, 204, 400, 401, 403, 404, 409, 415, 422);
const adminDisabledOk = ok(404);

const checks = [
  ["GET", "/health", ok(200)],
  ["GET", "/health/live", ok(204)],
  ["GET", "/health/ready", ok(204)],
  ["GET", "/metrics", ok(200)],
  ["GET", "/api/v1/home", ok(200)],
  ["GET", "/api/v1/bootstrap", ok(200, 401)],
  ["POST", "/api/auth/sign-in/anonymous", ok(200)],
  ["POST", "/api/auth/sign-in/email", ok(401, 422), json({ email: "missing@example.com", password: "bad-password" })],
  ["POST", "/api/auth/sign-in/social", ok(400, 302), json({ provider: "google", token: "smoke" })],
  ["GET", "/api/auth/sign-in/google", ok(400, 302)],
  ["GET", "/api/v1/catalog/series", ok(200)],
  ["GET", "/api/v1/catalog/series/page", ok(200)],
  ["GET", `/api/v1/catalog/episodes/${DUMMY.id}/series`, ok(404)],
  ["GET", `/api/v1/catalog/series/${DUMMY.slug}`, ok(404)],
  ["GET", "/api/v1/catalog/films", ok(200)],
  ["GET", "/api/v1/catalog/films/page", ok(200)],
  ["GET", `/api/v1/catalog/films/${DUMMY.slug}`, ok(404)],
  ["GET", `/api/v1/catalog/content/${DUMMY.contentId}`, ok(404)],
  ["GET", "/api/v1/catalog/creator/series", ok(200)],
  ["GET", `/api/v1/catalog/creator/series/${DUMMY.slug}`, ok(404)],
  ["GET", "/api/v1/catalog/creator/films", ok(200)],
  ["GET", `/api/v1/catalog/creator/films/${DUMMY.slug}`, ok(404)],
  ["GET", "/api/v1/live/streams", ok(200)],
  ["GET", `/api/v1/live/streams/${DUMMY.slug}`, ok(404)],
  ["GET", "/api/v1/live/discovery", ok(200)],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/notify`, mutationOk, json()],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/clip`, mutationOk, json()],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/report`, mutationOk, json({ reason: "smoke" })],
  ["GET", `/api/v1/live/streams/${DUMMY.streamId}/moderation/moderators`, readOnlyOk],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/moderation/moderators`, mutationOk, json({ userId: DUMMY.userId })],
  ["DELETE", `/api/v1/live/streams/${DUMMY.streamId}/moderation/moderators/${DUMMY.userId}`, mutationOk],
  ["GET", `/api/v1/live/streams/${DUMMY.streamId}/moderation/actions`, readOnlyOk],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/moderation/actions`, mutationOk, json()],
  ["GET", `/api/v1/live/streams/${DUMMY.streamId}/moderation/actions/${DUMMY.actionId}`, readOnlyOk],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/moderation/actions/${DUMMY.actionId}/reconcile`, mutationOk, json()],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/moderation/actions/${DUMMY.actionId}/revoke`, mutationOk, json()],
  ["GET", `/api/v1/live/streams/${DUMMY.streamId}/moderation/reports`, readOnlyOk],
  ["PATCH", `/api/v1/live/streams/${DUMMY.streamId}/moderation/reports/${DUMMY.reportId}`, mutationOk, json()],
  ["GET", `/api/v1/live/streams/${DUMMY.streamId}/moderation/audit`, readOnlyOk],
  ["GET", `/api/v1/live/streams/${DUMMY.streamId}/viewers`, readOnlyOk],
  ["GET", `/api/v1/live/streams/${DUMMY.streamId}/chat`, readOnlyOk],
  ["POST", `/api/v1/live/streams/${DUMMY.streamId}/chat/messages`, mutationOk, json({ message: "smoke" })],
  ["GET", "/api/v1/categories", ok(200)],
  ["GET", `/api/v1/categories/${DUMMY.slug}`, ok(404)],
  ["GET", `/api/v1/categories/${DUMMY.slug}/browse`, ok(404)],
  ["GET", "/api/v1/streamers", ok(200)],
  ["GET", `/api/v1/streamers/${DUMMY.userId}`, ok(404)],
  ["GET", "/api/v1/search?q=smoke", ok(200)],

  ["GET", "/api/v1/me", ok(200), null, true],
  ["GET", "/api/v1/me/state", readOnlyOk, null, true],
  ["GET", "/api/v1/me/library", readOnlyOk, null, true],
  ["GET", "/api/v1/me/entitlements", readOnlyOk, null, true],
  ["GET", `/api/v1/me/entitlements/memberships/${DUMMY.creatorId}`, readOnlyOk, null, true],
  ["POST", `/api/v1/me/entitlements/memberships/${DUMMY.creatorId}/reconcile`, mutationOk, json(), true],
  ["GET", `/api/v1/me/entitlements/purchases/${DUMMY.purchaseId}`, readOnlyOk, null, true],
  ["POST", `/api/v1/me/entitlements/purchases/${DUMMY.purchaseId}/reconcile`, mutationOk, json(), true],
  ["GET", "/api/v1/me/watchlist", readOnlyOk, null, true],
  ["GET", "/api/v1/me/notifications", readOnlyOk, null, true],
  ["POST", `/api/v1/me/notifications/${DUMMY.notificationId}/read`, mutationOk, json(), true],
  ["GET", "/api/v1/me/profile", readOnlyOk, null, true],
  ["PATCH", "/api/v1/me/profile", mutationOk, json({ displayName: "Guest Creator" }), true],
  ["GET", "/api/v1/me/settings", readOnlyOk, null, true],
  ["PATCH", "/api/v1/me/settings", mutationOk, json({}), true],
  ["GET", "/api/v1/me/plan", readOnlyOk, null, true],
  ["GET", "/api/v1/me/sessions", readOnlyOk, null, true],
  ["POST", "/api/v1/me/sessions", mutationOk, json(), true],
  ["DELETE", `/api/v1/me/sessions/${DUMMY.sessionId}`, mutationOk, null, true],
  ["POST", `/api/v1/me/watchlist/${DUMMY.contentId}`, mutationOk, null, true],
  ["DELETE", `/api/v1/me/watchlist/${DUMMY.contentId}`, mutationOk, null, true],
  ["POST", `/api/v1/me/following/${DUMMY.userId}`, mutationOk, null, true],
  ["DELETE", `/api/v1/me/following/${DUMMY.userId}`, mutationOk, null, true],
  ["GET", "/api/v1/me/following", readOnlyOk, null, true],
  ["PUT", "/api/v1/me/progress", mutationOk, json({ contentId: DUMMY.contentId, positionSec: 0 }), true],
  ["DELETE", `/api/v1/me/progress/${DUMMY.contentId}`, mutationOk, null, true],
  ["DELETE", `/api/v1/me/history/${DUMMY.contentId}`, mutationOk, null, true],

  ["GET", "/api/v1/creator/me/dashboard", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/state", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/analytics/summary", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/revenue/summary", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/operations", readOnlyOk, null, true],
  ["PATCH", "/api/v1/creator/me/operations", mutationOk, json({}), true],
  ["GET", "/api/v1/creator/me/upload-operations", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/subscriber-tiers", readOnlyOk, null, true],
  ["POST", "/api/v1/creator/me/subscriber-tiers", mutationOk, json({}), true],
  ["PATCH", `/api/v1/creator/me/subscriber-tiers/${DUMMY.tierId}`, mutationOk, json({}), true],
  ["POST", `/api/v1/creator/me/subscriber-tiers/${DUMMY.tierId}/retire`, mutationOk, json(), true],
  ["GET", "/api/v1/creator/me/series", readOnlyOk, null, true],
  ["POST", "/api/v1/creator/me/series", mutationOk, json({}), true],
  ["PATCH", `/api/v1/creator/me/series/${DUMMY.id}`, mutationOk, json({}), true],
  ["POST", `/api/v1/creator/subscriptions/${DUMMY.creatorId}/tiers/${DUMMY.tierId}`, mutationOk, json(), true],
  ["DELETE", `/api/v1/creator/subscriptions/${DUMMY.creatorId}`, mutationOk, null, true],
  ["GET", "/api/v1/creator/me/analytics", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/revenue", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/notifications", readOnlyOk, null, true],
  ["POST", `/api/v1/creator/me/notifications/${DUMMY.notificationId}/read`, mutationOk, json(), true],
  ["GET", "/api/v1/creator/me/ad-hub", ok(200), null, true],
  ["POST", `/api/v1/creator/me/ad-offers/${DUMMY.id}/accept`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/ad-offers/${DUMMY.id}/decline`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/ad-offers/${DUMMY.id}/submissions`, mutationOk, json({ submissionUrl: "https://example.com/smoke" }), true],

  ["GET", "/api/v1/creator/me/uploads", readOnlyOk, null, true],
  ["GET", "/api/v1/creator/me/content", readOnlyOk, null, true],
  ["PATCH", `/api/v1/creator/me/uploads/${DUMMY.uploadId}`, mutationOk, json({}), true],
  ["PATCH", `/api/v1/creator/me/uploads/${DUMMY.uploadId}/lifecycle`, mutationOk, json({}), true],
  ["POST", `/api/v1/creator/me/uploads/${DUMMY.uploadId}/unpublish`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/uploads/${DUMMY.uploadId}/takedown`, mutationOk, json(), true],
  ["POST", "/api/v1/creator/me/uploads/bulk", mutationOk, json({ uploads: [] }), true],

  ["GET", "/api/v1/creator/me/upload-jobs", readOnlyOk, null, true],
  ["POST", "/api/v1/creator/me/upload-jobs", mutationOk, json({}), true],
  ["PATCH", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}`, mutationOk, json({}), true],
  ["GET", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}/ingest`, readOnlyOk, null, true],
  ["POST", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}/ingest`, mutationOk, json(), true],
  ["PUT", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}/ingest/chunk`, mutationOk, { body: "smoke" }, true],
  ["POST", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}/ingest/complete`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}/retry`, mutationOk, json(), true],
  ["GET", "/api/v1/creator/me/media-assets", readOnlyOk, null, true],
  ["GET", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}/media-asset`, readOnlyOk, null, true],
  ["POST", `/api/v1/creator/me/upload-jobs/${DUMMY.jobId}/publish`, mutationOk, json(), true],

  ["GET", "/api/v1/creator/me/live/collabs", readOnlyOk, null, true],
  ["POST", "/api/v1/creator/me/live/collabs/sessions", mutationOk, json({}), true],
  ["GET", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}`, readOnlyOk, null, true],
  ["GET", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/events`, readOnlyOk, null, true],
  ["GET", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/control`, readOnlyOk, null, true],
  ["GET", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/socket-sessions/${DUMMY.socketId}`, readOnlyOk, null, true],
  ["POST", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/socket-sessions/${DUMMY.socketId}/reconcile`, mutationOk, json(), true],
  ["GET", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/runtime`, readOnlyOk, null, true],
  ["POST", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/reconcile`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/end`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/invites`, mutationOk, json({}), true],
  ["POST", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/invites/${DUMMY.inviteId}/revoke`, mutationOk, json(), true],
  ["PATCH", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/participants/${DUMMY.participantId}`, mutationOk, json({}), true],
  ["POST", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/participants/${DUMMY.participantId}/remove`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/live/collabs/sessions/${DUMMY.sessionId}/participants/${DUMMY.participantId}/grants/mirror`, mutationOk, json(), true],
  ["GET", "/api/v1/me/live/collabs/invites", readOnlyOk, null, true],
  ["GET", "/api/v1/me/live/collabs/sessions", readOnlyOk, null, true],
  ["GET", `/api/v1/me/live/collabs/sessions/${DUMMY.sessionId}`, readOnlyOk, null, true],
  ["POST", `/api/v1/me/live/collabs/sessions/${DUMMY.sessionId}/leave`, mutationOk, json(), true],
  ["GET", `/api/v1/me/live/collabs/sessions/${DUMMY.sessionId}/events`, readOnlyOk, null, true],
  ["GET", `/api/v1/me/live/collabs/sessions/${DUMMY.sessionId}/runtime`, readOnlyOk, null, true],
  ["GET", `/api/v1/me/live/collabs/sessions/${DUMMY.sessionId}/grants`, readOnlyOk, null, true],
  ["POST", `/api/v1/live/collabs/invites/${DUMMY.inviteId}/accept`, mutationOk, json(), true],
  ["POST", `/api/v1/live/collabs/invites/${DUMMY.inviteId}/decline`, mutationOk, json(), true],
  ["POST", `/api/v1/live/collabs/grants/${DUMMY.grantId}/redeem`, mutationOk, json(), true],

  ["GET", "/api/v1/admin/notifications/deliveries", adminDisabledOk],
  ["GET", `/api/v1/admin/notifications/deliveries/${DUMMY.deliveryId}`, adminDisabledOk],
  ["POST", `/api/v1/admin/notifications/deliveries/${DUMMY.deliveryId}/reconcile`, adminDisabledOk, json()],
  ["POST", `/api/v1/admin/notifications/deliveries/${DUMMY.deliveryId}/retry`, adminDisabledOk, json()],
  ["GET", "/api/v1/admin/media/upload-jobs", adminDisabledOk],
  ["GET", `/api/v1/admin/media/upload-jobs/${DUMMY.jobId}`, adminDisabledOk],
  ["POST", `/api/v1/admin/media/upload-jobs/${DUMMY.jobId}/reconcile`, adminDisabledOk, json()],
  ["POST", `/api/v1/admin/media/upload-jobs/${DUMMY.jobId}/retry`, adminDisabledOk, json()],
  ["GET", "/api/v1/admin/live/ingest/sessions", adminDisabledOk],
  ["GET", "/api/v1/admin/live/ingest/overview", adminDisabledOk],
  ["GET", `/api/v1/admin/live/ingest/sessions/${DUMMY.sessionId}`, adminDisabledOk],
  ["POST", `/api/v1/admin/live/ingest/sessions/${DUMMY.sessionId}/reconcile`, adminDisabledOk, json()],
  ["POST", `/api/v1/admin/live/ingest/sessions/${DUMMY.sessionId}/terminate`, adminDisabledOk, json()],
  ["POST", `/api/v1/admin/live/ingest/sessions/${DUMMY.sessionId}/runtime/repair`, adminDisabledOk, json()],
  ["GET", `/api/v1/admin/creators/${DUMMY.creatorId}/enforcement`, adminDisabledOk],
  ["POST", `/api/v1/admin/creators/${DUMMY.creatorId}/enforcement/actions`, adminDisabledOk, json()],
  ["GET", `/api/v1/admin/creators/${DUMMY.creatorId}/enforcement/actions/${DUMMY.actionId}`, adminDisabledOk],
  ["POST", `/api/v1/admin/creators/${DUMMY.creatorId}/enforcement/actions/${DUMMY.actionId}/reconcile`, adminDisabledOk, json()],
  ["POST", `/api/v1/admin/creators/${DUMMY.creatorId}/enforcement/actions/${DUMMY.actionId}/release`, adminDisabledOk, json()],
  ["GET", "/api/v1/admin/playback/sessions", adminDisabledOk],
  ["GET", `/api/v1/admin/playback/sessions/${DUMMY.sessionId}`, adminDisabledOk],
  ["POST", `/api/v1/admin/playback/sessions/${DUMMY.sessionId}/reconcile`, adminDisabledOk, json()],
  ["POST", `/api/v1/admin/playback/sessions/${DUMMY.sessionId}/revoke`, adminDisabledOk, json()],

  ["POST", "/api/v1/creator/me/broadcasts/start", mutationOk, json({}), true],
  ["POST", `/api/v1/creator/me/broadcasts/${DUMMY.id}/end`, mutationOk, json(), true],
  ["POST", "/api/v1/creator/me/stream-key/rotate", mutationOk, json(), true],
  ["GET", "/api/v1/creator/me/live/ingest", readOnlyOk, null, true],
  ["GET", `/api/v1/creator/me/live/ingest/${DUMMY.sessionId}`, readOnlyOk, null, true],
  ["GET", `/api/v1/creator/me/live/ingest/${DUMMY.sessionId}/events`, readOnlyOk, null, true],
  ["POST", `/api/v1/creator/me/live/ingest/${DUMMY.sessionId}/reconcile`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/live/ingest/${DUMMY.sessionId}/terminate`, mutationOk, json(), true],
  ["POST", `/api/v1/creator/me/live/ingest/${DUMMY.sessionId}/runtime/repair`, mutationOk, json(), true],
  ["POST", "/api/v1/ingest/live/connect", mutationOk, json({ streamKey: "smoke" })],
  ["POST", `/api/v1/ingest/live/${DUMMY.sessionId}/heartbeat`, mutationOk, json()],
  ["POST", `/api/v1/ingest/live/${DUMMY.sessionId}/disconnect`, mutationOk, json()],
  ["POST", `/api/v1/ingest/live/${DUMMY.sessionId}/terminate`, mutationOk, json()],
  ["POST", `/api/v1/ingest/live/${DUMMY.sessionId}/runtime`, mutationOk, json()],

  ["POST", `/api/v1/playback/uploads/${DUMMY.uploadId}/session`, mutationOk, json(), true],
  ["POST", `/api/v1/playback/content/${DUMMY.contentId}/session`, mutationOk, json(), true],
  ["POST", `/api/v1/playback/live/${DUMMY.streamId}/session`, mutationOk, json(), true],
  ["POST", `/api/v1/uploads/${DUMMY.uploadId}/purchase`, mutationOk, json(), true],
  ["POST", `/api/v1/content/${DUMMY.contentId}/purchase`, mutationOk, json(), true],
  ["GET", `/api/v1/playback/sessions/${DUMMY.sessionId}`, readOnlyOk, null, true],
  ["POST", `/api/v1/playback/sessions/${DUMMY.sessionId}/refresh`, mutationOk, json(), true],
  ["GET", `/api/v1/playback/sessions/${DUMMY.sessionId}/cdn-cookie`, readOnlyOk, null, true],
  ["GET", `/api/v1/playback/sessions/${DUMMY.sessionId}/manifest`, readOnlyOk, null, true],
  ["GET", "/api/v1/media/smoke-missing/master.m3u8", readOnlyOk],
];

async function request(method, url, init = {}) {
  const started = performance.now();
  const response = await fetch(url, {
    method,
    redirect: "manual",
    ...init,
    headers: {
      ...(init.headers || {}),
    },
  });
  await response.arrayBuffer();
  return {
    status: response.status,
    ms: Math.round(performance.now() - started),
  };
}

async function main() {
  const failures = [];
  const rows = [];

  const auth = await request("POST", `${API_BASE}/api/auth/sign-in/anonymous`);
  if (auth.status !== 200) {
    throw new Error(`anonymous auth failed with ${auth.status}`);
  }
  const authJson = await fetch(`${API_BASE}/api/auth/sign-in/anonymous`, { method: "POST" }).then((r) =>
    r.json(),
  );
  const token = authJson.accessToken;
  if (!token) {
    throw new Error("anonymous auth did not return accessToken");
  }
  if (!token.startsWith("session_")) {
    throw new Error(`anonymous auth returned non-Better-Auth session token: ${token.slice(0, 12)}`);
  }

  for (const [method, path, expected, init, needsAuth] of checks) {
    const headers = { ...(init?.headers || {}) };
    if (needsAuth) headers.authorization = `Bearer ${token}`;
    const result = await request(method, `${API_BASE}${path}`, { ...(init || {}), headers });
    const pass = expected.has(result.status) && result.status < 500;
    rows.push({ scope: "api", method, path, status: result.status, ms: result.ms, pass });
    if (!pass) failures.push(rows[rows.length - 1]);
  }

  const apiRows = rows.filter((row) => row.scope === "api");
  const latencies = apiRows.map((row) => row.ms).sort((a, b) => a - b);
  const p95 = latencies[Math.floor(latencies.length * 0.95)] || 0;
  const max = latencies[latencies.length - 1] || 0;
  const byStatus = rows.reduce((acc, row) => {
    acc[row.status] = (acc[row.status] || 0) + 1;
    return acc;
  }, {});

  console.log(
    JSON.stringify(
      {
        ok: failures.length === 0,
        apiBase: API_BASE,
        checked: rows.length,
        apiChecked: apiRows.length,
        failures,
        stats: {
          byStatus,
          latencyMs: {
            p95,
            max,
          },
        },
      },
      null,
      2,
    ),
  );

  if (failures.length) process.exit(1);
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
