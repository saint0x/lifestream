#!/usr/bin/env node

const API_BASE = process.env.VANTA_API_BASE || "https://api-production-4becb.up.railway.app";

async function request(method, path, { token, body } = {}) {
  const response = await fetch(`${API_BASE}${path}`, {
    method,
    headers: {
      ...(body === undefined ? {} : { "content-type": "application/json" }),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
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

  const storageKey = `smoke/${stamp}/production-upload-check.mp4`;
  const created = await request("POST", "/api/v1/creator/me/upload-jobs", {
    token,
    body: {
      kind: "film",
      sourceType: "resumable-upload",
      title: `Production Upload ${stamp}`,
      intendedVisibility: "private",
      bytesExpected: 4096,
      storageKey,
      mimeType: "video/mp4",
    },
  });
  assert(created.id, "upload job create did not return id", created);
  assert(created.status === "created", "upload job has unexpected status", created);
  assert(created.storageKey === storageKey, "upload job storageKey was not persisted", created);
  assert(created.bytesExpected === 4096, "upload job bytesExpected was not persisted", created);

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

  console.log(
    JSON.stringify(
      {
        ok: true,
        apiBase: API_BASE,
        email,
        uploadJobId: created.id,
        storageKey,
        status: patched.status,
        intendedVisibility: patched.intendedVisibility,
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
