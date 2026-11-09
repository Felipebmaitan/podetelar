import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

test("only the configured proxy peer can supply per-viewer rate-limit addresses", async () => {
  const root = mkdtempSync(join(tmpdir(), "freedomstream-proxy-"));
  const config = loadConfig({
    TRUSTED_PROXY: "172.30.77.1",
    ADMIN_TOKEN: "test-administrator-secret-with-32-characters",
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "test-livekit-secret-with-32-characters",
    DATABASE_PATH: join(root, "rooms.db"),
  });
  const app = await buildApp(config, {
    mediaDirectory: {
      async participant() {
        return null;
      },
      async attribute() {},
      async remove() {},
    },
  });
  try {
    for (const remoteAddress of ["203.0.113.99", "172.30.77.1"]) {
      for (let i = 0; i < 6; i++) {
        const response = await app.inject({
          method: "POST",
          url: "/api/rooms",
          remoteAddress,
          headers: {
            origin: config.publicUrl,
            authorization: `Bearer ${config.adminToken}`,
            "x-forwarded-for": `198.51.100.${i + 1}`,
          },
          payload: { name: "Friends", displayName: "Friend" },
        });
        assert.equal(
          response.statusCode,
          remoteAddress === "203.0.113.99" && i === 5 ? 429 : 201,
        );
      }
    }
  } finally {
    await app.close();
    const resolved = realpathSync(root);
    assert.equal(dirname(resolved), realpathSync(tmpdir()));
    assert.ok(basename(resolved).startsWith("freedomstream-proxy-"));
    rmSync(resolved, { recursive: true });
  }
});
