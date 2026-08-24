#!/usr/bin/env node

const API_BASE = process.env.VANTA_API_BASE || "https://api-production-4becb.up.railway.app";

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
    throw new Error(
      `${method} ${path} failed with ${response.status}: ${JSON.stringify(json)}`,
    );
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
  let json = null;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch {
      json = { raw: text };
    }
  }
  if (!response.ok) {
    throw new Error(
      `${method} ${path} failed with ${response.status}: ${JSON.stringify(json)}`,
    );
  }
  return json;
}

function assert(condition, message, context) {
  if (!condition) {
    throw new Error(`${message}${context ? `: ${JSON.stringify(context)}` : ""}`);
  }
}

async function main() {
  const stamp = Date.now();
  const email = `vanta-upload-${stamp}@example.com`;
  const displayName = `Vanta Upload ${stamp}`;
  const auth = await request("POST", "/api/auth/sign-up/email", {
    body: {
      email,
      password: `Vanta-${stamp}-password`,
      displayName,
    },
  });
  const token = auth.accessToken;
  assert(token, "sign-up did not return accessToken", auth);

  const uploadBytes = Buffer.from(`vanta-upload-check-${stamp}`);
  const storageKey = `smoke/${stamp}/production-upload-check.mp4`;
  const created = await request("POST", "/api/v1/creator/me/upload-jobs", {
    token,
    body: {
      kind: "film",
      sourceType: "resumable-upload",
      title: `Production Upload ${stamp}`,
      intendedVisibility: "private",
      bytesExpected: uploadBytes.byteLength,
      storageKey,
      mimeType: "video/mp4",
    },
  });
  assert(created.id, "upload job create did not return id", created);
  assert(created.status === "created", "upload job has unexpected status", created);
  assert(created.storageKey === storageKey, "upload job storageKey was not persisted", created);
  assert(
    created.bytesExpected === uploadBytes.byteLength,
    "upload job bytesExpected was not persisted",
    created,
  );

  const listed = await request("GET", "/api/v1/creator/me/upload-jobs", { token });
  assert(Array.isArray(listed), "upload job list did not return an array", listed);
  assert(
    listed.some((job) => job.id === created.id),
    "created upload job was not present in list response",
    listed,
  );

  const patchedTitle = `Production Upload Patched ${stamp}`;
  const patched = await request("PATCH", `/api/v1/creator/me/upload-jobs/${created.id}`, {
    token,
    body: {
      title: patchedTitle,
      intendedVisibility: "unlisted",
      mimeType: "video/quicktime",
    },
  });
  assert(patched.id === created.id, "patched upload job id changed", patched);
  assert(patched.title === patchedTitle, "patched title was not returned", patched);
  assert(patched.intendedVisibility === "unlisted", "patched visibility was not returned", patched);
  assert(patched.mimeType === "video/quicktime", "patched mime type was not returned", patched);

  const relisted = await request("GET", "/api/v1/creator/me/upload-jobs", { token });
  const relistedJob = relisted.find((job) => job.id === created.id);
  assert(relistedJob, "patched upload job was not present in relist response", relisted);
  assert(relistedJob.title === patchedTitle, "patched title was not persisted", relistedJob);

  const ticket = await request("POST", `/api/v1/creator/me/upload-jobs/${created.id}/ingest`, {
    token,
  });
  assert(ticket.uploadToken, "ingest start did not return upload token", ticket);
  assert(ticket.session?.status === "active", "ingest session is not active", ticket);
  assert(ticket.session.bytesReceived === 0, "new ingest session should start at zero bytes", ticket);

  const chunked = await requestBytes(
    "PUT",
    `/api/v1/creator/me/upload-jobs/${created.id}/ingest/chunk?offset=0`,
    {
      token,
      uploadToken: ticket.uploadToken,
      body: uploadBytes,
    },
  );
  assert(
    chunked.bytesReceived === uploadBytes.byteLength,
    "chunk upload did not advance ingest byte counter",
    chunked,
  );

  const completed = await request(
    "POST",
    `/api/v1/creator/me/upload-jobs/${created.id}/ingest/complete`,
    {
      token,
      headers: { "x-upload-token": ticket.uploadToken },
    },
  );
  assert(completed.status === "uploaded", "completed job did not move to uploaded", completed);
  assert(
    completed.bytesReceived === uploadBytes.byteLength,
    "completed job byte count does not match upload",
    completed,
  );
  assert(completed.checksumSha256, "completed job did not record checksum", completed);

  const asset = await request(
    "GET",
    `/api/v1/creator/me/upload-jobs/${created.id}/media-asset`,
    { token },
  );
  assert(asset.uploadJobId === created.id, "media asset is not linked to upload job", asset);
  assert(asset.status === "uploaded", "media asset shell status is not uploaded", asset);
  assert(asset.fileSizeBytes === uploadBytes.byteLength, "media asset size is wrong", asset);
  assert(asset.sourcePath.includes(storageKey), "media asset source path does not include storage key", asset);

  const assets = await request("GET", "/api/v1/creator/me/media-assets", { token });
  assert(Array.isArray(assets), "media asset list did not return an array", assets);
  assert(
    assets.some((item) => item.uploadJobId === created.id),
    "created media asset was not present in media asset list",
    assets,
  );

  console.log(
    JSON.stringify(
      {
        ok: true,
        apiBase: API_BASE,
        email,
        uploadJobId: created.id,
        mediaAssetId: asset.id,
        storageKey,
        status: completed.status,
        intendedVisibility: completed.intendedVisibility,
        bytesReceived: completed.bytesReceived,
      },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
