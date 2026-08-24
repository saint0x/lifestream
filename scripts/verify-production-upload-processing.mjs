#!/usr/bin/env node

import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const API_BASE = process.env.VANTA_API_BASE || "https://api-production-4becb.up.railway.app";
const DATABASE_URL = process.env.VANTA_DATABASE_URL || process.env.DATABASE_URL;

if (!DATABASE_URL) {
  throw new Error("VANTA_DATABASE_URL or DATABASE_URL is required for cleanup verification.");
}

async function request(method, path, { token, body, headers = {} } = {}) {
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
  if (!response.ok) {
    throw new Error(`${method} ${path} failed with ${response.status}: ${JSON.stringify(json)}`);
  }
  return json;
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

function generateFixtureMp4() {
  const dir = mkdtempSync(join(tmpdir(), "vanta-production-processing-"));
  const path = join(dir, "source.mp4");
  const result = spawnSync(
    "ffmpeg",
    [
      "-y",
      "-f",
      "lavfi",
      "-i",
      "testsrc=size=320x240:rate=24:duration=1",
      "-f",
      "lavfi",
      "-i",
      "sine=frequency=1000:sample_rate=48000:duration=1",
      "-c:v",
      "libx264",
      "-pix_fmt",
      "yuv420p",
      "-c:a",
      "aac",
      "-shortest",
      path,
    ],
    { encoding: "utf8" },
  );
  if (result.status !== 0) {
    rmSync(dir, { force: true, recursive: true });
    throw new Error(`ffmpeg fixture generation failed: ${result.stderr}`);
  }
  return { dir, bytes: readFileSync(path) };
}

async function waitForReadyAsset(token, uploadJobId) {
  const deadline = Date.now() + 180_000;
  let lastJob = null;
  let lastAsset = null;
  while (Date.now() < deadline) {
    const jobs = await request("GET", "/api/v1/creator/me/upload-jobs", { token });
    lastJob = jobs.find((job) => job.id === uploadJobId) ?? lastJob;
    try {
      lastAsset = await request("GET", `/api/v1/creator/me/upload-jobs/${uploadJobId}/media-asset`, {
        token,
      });
    } catch {
      lastAsset = null;
    }
    if (lastJob?.status === "ready" && lastAsset?.status === "ready") {
      return { job: lastJob, asset: lastAsset };
    }
    if (lastJob?.status === "failed" || lastAsset?.status === "failed") {
      throw new Error(`media processing failed: ${JSON.stringify({ job: lastJob, asset: lastAsset })}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 3_000));
  }
  throw new Error(`media processing did not become ready: ${JSON.stringify({ job: lastJob, asset: lastAsset })}`);
}

async function main() {
  const stamp = Date.now();
  const email = `vanta-processing-${stamp}@example.com`;
  const password = `Vanta-${stamp}-password`;
  const title = `Production Processing ${stamp}`;
  const slug = `production-processing-${stamp}`;
  const storageKey = `smoke/${stamp}/production-processing-check.mp4`;
  const fixture = generateFixtureMp4();

  let uploadJobId = null;
  let uploadId = null;

  try {
    const auth = await request("POST", "/api/auth/sign-up/email", {
      body: { email, password, displayName: `Vanta Processing ${stamp}` },
    });
    const token = auth.accessToken;
    assert(token, "sign-up did not return accessToken", auth);

    const created = await request("POST", "/api/v1/creator/me/upload-jobs", {
      token,
      body: {
        kind: "film",
        sourceType: "resumable-upload",
        title,
        intendedVisibility: "public",
        bytesExpected: fixture.bytes.byteLength,
        storageKey,
        mimeType: "video/mp4",
      },
    });
    uploadJobId = created.id;
    assert(uploadJobId, "upload job create did not return id", created);

    const ticket = await request("POST", `/api/v1/creator/me/upload-jobs/${uploadJobId}/ingest`, {
      token,
    });
    await requestBytes(
      "PUT",
      `/api/v1/creator/me/upload-jobs/${uploadJobId}/ingest/chunk?offset=0`,
      { token, uploadToken: ticket.uploadToken, body: fixture.bytes },
    );
    const completed = await request(
      "POST",
      `/api/v1/creator/me/upload-jobs/${uploadJobId}/ingest/complete`,
      { token, headers: { "x-upload-token": ticket.uploadToken } },
    );
    assert(completed.status === "uploaded", "completed job did not move to uploaded", completed);

    const { asset } = await waitForReadyAsset(token, uploadJobId);
    assert(asset.playbackPath, "ready asset did not include playback path", asset);
    assert(asset.variants.some((variant) => variant.variantType === "playback"), "ready asset has no playback variants", asset);
    assert(asset.variants.some((variant) => variant.variantType === "thumbnail"), "ready asset has no thumbnail variants", asset);

    const published = await request("POST", `/api/v1/creator/me/upload-jobs/${uploadJobId}/publish`, {
      token,
      body: {
        slug,
        visibility: "public",
        description: "Production smoke processing and publish, cleaned up after verification.",
        accessPolicy: "free",
      },
    });
    uploadId = published.id;
    assert(published.status === "published", "publish did not return published status", published);

    console.log(
      JSON.stringify(
        {
          ok: true,
          apiBase: API_BASE,
          email,
          uploadJobId,
          uploadId,
          slug,
          assetId: asset.id,
          playbackPath: asset.playbackPath,
          variantCount: asset.variants.length,
          status: published.status,
        },
        null,
        2,
      ),
    );
  } finally {
    rmSync(fixture.dir, { force: true, recursive: true });
    psql(`
      WITH smoke_events AS (
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
