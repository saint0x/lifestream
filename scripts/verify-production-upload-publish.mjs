#!/usr/bin/env node

import { execFileSync } from "node:child_process";

const API_BASE = process.env.VANTA_API_BASE || "https://api-production-4becb.up.railway.app";
const DATABASE_URL = process.env.VANTA_DATABASE_URL || process.env.DATABASE_URL;

if (!DATABASE_URL) {
  throw new Error("VANTA_DATABASE_URL or DATABASE_URL is required for publish verification.");
}

async function request(method, path, { token, body, headers = {}, expectOk = true } = {}) {
  const response = await fetch(`${API_BASE}${path}`, {
    method,
    headers: {
      ...(body === undefined ? {} : { "content-type": "application/json" }),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...headers,
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let json = null;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch {
      json = { raw: text };
    }
  }
  if (expectOk && !response.ok) {
    throw new Error(`${method} ${path} failed with ${response.status}: ${JSON.stringify(json)}`);
  }
  return { status: response.status, ok: response.ok, json };
}

async function requestBytes(method, path, { token, uploadToken, body } = {}) {
  const response = await fetch(`${API_BASE}${path}`, {
    method,
    headers: {
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(uploadToken ? { "x-upload-token": uploadToken } : {}),
      "content-type": "application/octet-stream",
    },
    body,
  });
  const text = await response.text();
  const json = text ? JSON.parse(text) : null;
  if (!response.ok) {
    throw new Error(`${method} ${path} failed with ${response.status}: ${text}`);
  }
  return json;
}

function assert(condition, message, context) {
  if (!condition) {
    throw new Error(`${message}${context ? `: ${JSON.stringify(context)}` : ""}`);
  }
}

function sqlLiteral(value) {
  if (value === null || value === undefined) return "NULL";
  return `'${String(value).replaceAll("'", "''")}'`;
}

function psql(sql) {
  try {
    return execFileSync("psql", [DATABASE_URL, "-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-c", sql], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  } catch (error) {
    const stderr = String(error.stderr || "")
      .replaceAll(DATABASE_URL, "[redacted-database-url]")
      .replace(/postgres(?:ql)?:\/\/[^\s]+/g, "[redacted-database-url]")
      .trim();
    throw new Error(`psql command failed${stderr ? `: ${stderr}` : ""}`);
  }
}

async function main() {
  const stamp = Date.now();
  const email = `vanta-publish-${stamp}@example.com`;
  const password = `Vanta-${stamp}-password`;
  const title = `Production Publish ${stamp}`;
  const slug = `production-publish-${stamp}`;
  const uploadBytes = Buffer.from(`vanta-publish-check-${stamp}`);
  const storageKey = `smoke/${stamp}/production-publish-check.mp4`;

  let uploadJobId = null;
  let uploadId = null;

  try {
    const auth = (
      await request("POST", "/api/auth/sign-up/email", {
        body: { email, password, displayName: `Vanta Publish ${stamp}` },
      })
    ).json;
    const token = auth.accessToken;
    assert(token, "sign-up did not return accessToken", auth);

    const created = (
      await request("POST", "/api/v1/creator/me/upload-jobs", {
        token,
        body: {
          kind: "film",
          sourceType: "resumable-upload",
          title,
          intendedVisibility: "public",
          bytesExpected: uploadBytes.byteLength,
          storageKey,
          mimeType: "video/mp4",
        },
      })
    ).json;
    uploadJobId = created.id;
    assert(uploadJobId, "upload job create did not return id", created);

    const ticket = (
      await request("POST", `/api/v1/creator/me/upload-jobs/${uploadJobId}/ingest`, { token })
    ).json;
    assert(ticket.uploadToken, "ingest start did not return upload token", ticket);

    await requestBytes(
      "PUT",
      `/api/v1/creator/me/upload-jobs/${uploadJobId}/ingest/chunk?offset=0`,
      { token, uploadToken: ticket.uploadToken, body: uploadBytes },
    );

    const completed = (
      await request("POST", `/api/v1/creator/me/upload-jobs/${uploadJobId}/ingest/complete`, {
        token,
        headers: { "x-upload-token": ticket.uploadToken },
      })
    ).json;
    assert(completed.status === "uploaded", "completed job did not move to uploaded", completed);

    psql(`
      UPDATE media_assets
      SET status = 'ready',
          playback_relative_path = source_relative_path,
          processed_at = COALESCE(processed_at, updated_at),
          updated_at = updated_at
      WHERE upload_job_id = ${sqlLiteral(uploadJobId)};

      UPDATE upload_jobs
      SET status = 'ready'
      WHERE id = ${sqlLiteral(uploadJobId)};
    `);

    const published = (
      await request("POST", `/api/v1/creator/me/upload-jobs/${uploadJobId}/publish`, {
        token,
        body: {
          slug,
          visibility: "public",
          description: "Production smoke publish, cleaned up after verification.",
          accessPolicy: "free",
        },
      })
    ).json;
    uploadId = published.id;
    assert(uploadId, "publish did not return upload id", published);
    assert(published.slug === slug, "publish did not persist requested slug", published);
    assert(published.status === "published", "publish did not return published status", published);
    assert(published.visibility === "public", "publish did not return public visibility", published);

    const state = psql(`
      SELECT uj.status || ',' || ma.status || ',' || u.status || ',' || COUNT(nd.id)::TEXT
      FROM upload_jobs uj
      JOIN media_assets ma ON ma.upload_job_id = uj.id
      JOIN uploads u ON u.id = uj.upload_id
      LEFT JOIN notification_events ne ON ne.payload_json LIKE '%' || u.id || '%'
      LEFT JOIN notification_deliveries nd ON nd.event_id = ne.id
      WHERE uj.id = ${sqlLiteral(uploadJobId)}
      GROUP BY uj.status, ma.status, u.status;
    `);
    assert(
      state === "published,published,published,1",
      "publish did not persist upload/job/asset/notification state",
      { state },
    );

    console.log(
      JSON.stringify(
        {
          ok: true,
          apiBase: API_BASE,
          email,
          uploadJobId,
          uploadId,
          slug,
          status: published.status,
        },
        null,
        2,
      ),
    );
  } finally {
    psql(`
      WITH smoke_upload AS (
        SELECT ${uploadId ? sqlLiteral(uploadId) : "upload_id"} AS id
        FROM upload_jobs
        WHERE id = ${sqlLiteral(uploadJobId)}
        UNION
        SELECT ${uploadId ? sqlLiteral(uploadId) : "NULL"} AS id
      ),
      smoke_events AS (
        SELECT id FROM notification_events
        WHERE payload_json LIKE ${sqlLiteral(`%${uploadJobId ?? "no-job"}%`)}
           OR payload_json LIKE ${sqlLiteral(`%${uploadId ?? "no-upload"}%`)}
      )
      DELETE FROM notification_deliveries WHERE event_id IN (SELECT id FROM smoke_events);

      DELETE FROM notification_events
      WHERE payload_json LIKE ${sqlLiteral(`%${uploadJobId ?? "no-job"}%`)}
         OR payload_json LIKE ${sqlLiteral(`%${uploadId ?? "no-upload"}%`)};

      DELETE FROM uploads WHERE id = ${sqlLiteral(uploadId)};
      DELETE FROM upload_jobs WHERE id = ${sqlLiteral(uploadJobId)};
      DELETE FROM users
      WHERE id IN (SELECT user_id FROM user_profiles WHERE email = ${sqlLiteral(email)});
    `);
  }
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
