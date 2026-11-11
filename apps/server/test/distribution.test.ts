import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { test } from "node:test";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

test("release downloads require current room authority and verify the installed bytes", async () => {
  const root = mkdtempSync(join(tmpdir(), "freedomstream-release-"));
  const archive = Buffer.from("operator-installed test archive");
  const manifest = {
    filename: "freedomstream-windows-x64-0.1.0-aaaaaaaaaaaa.exe",
    version: "0.1.0",
    commit: "a".repeat(40),
    sha256: createHash("sha256").update(archive).digest("hex"),
    size: archive.length,
  };
  writeFileSync(join(root, manifest.filename), archive);
  writeFileSync(join(root, "publisher.json"), JSON.stringify(manifest));
  const origin = "http://localhost:5173";
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: "test-administrator-key-with-32-characters",
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "test-livekit-secret-with-32-characters",
    DATABASE_PATH: join(root, "rooms.db"),
    PUBLISHER_DIRECTORY: root,
  });
  const app = await buildApp(config, {
    mediaDirectory: {
      members: async () => [],
      async participant() {
        return null;
      },
      async attribute() {},
      async remove() {},
    },
  });
  try {
    const created = await app.inject({
      method: "POST",
      url: "/api/rooms",
      headers: { origin, authorization: `Bearer ${config.adminToken}` },
      payload: { name: "Friends", displayName: "Friend" },
    });
    const id = created.json().room.id;
    const url = `/api/rooms/${id}/publisher`;
    const cookie = String(created.headers["set-cookie"]).split(";")[0]!;
    assert.equal((await app.inject({ url })).statusCode, 401);
    assert.equal(
      (await app.inject({ url: `${url}/windows.zip` })).statusCode,
      401,
    );
    const release = await app.inject({ url, headers: { cookie } });
    assert.equal(release.json().sha256, manifest.sha256);
    const download = await app.inject({
      url: release.json().downloadUrl,
      headers: { cookie },
    });
    assert.deepEqual(download.rawPayload, archive);
    assert.equal(
      download.headers["content-type"],
      "application/vnd.microsoft.portable-executable",
    );
    assert.match(String(download.headers["content-disposition"]), /\.exe"$/);
    const legacy = await app.inject({
      url: `${url}/windows.zip`,
      headers: { cookie },
    });
    assert.equal(legacy.statusCode, 307);
    assert.equal(legacy.headers.location, release.json().downloadUrl);
    assert.equal(download.headers["cache-control"], "no-store");
    assert.equal(
      (await app.inject({ url: "/publisher.json" })).statusCode,
      404,
    );
    await app.inject({
      method: "POST",
      url: `/api/rooms/${id}/leave`,
      headers: { cookie, origin },
    });
    assert.equal(
      (await app.inject({ url: `${url}/windows.zip`, headers: { cookie } }))
        .statusCode,
      401,
    );
  } finally {
    await app.close();
  }
  writeFileSync(join(root, manifest.filename), "tampered");
  await assert.rejects(buildApp(config), /release manifest/);
  const resolved = realpathSync(root);
  assert.equal(dirname(resolved), realpathSync(tmpdir()));
  assert.ok(basename(resolved).startsWith("freedomstream-release-"));
  rmSync(resolved, { recursive: true });
});
