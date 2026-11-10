import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

const password = "test-administrator-password-not-for-production";
const origin = "https://admin.example";
test("single-administrator sessions protect all usage, reject CSRF, expire and revoke on logout/restart", async () => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-admin-"));
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: password,
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "test-media-secret-not-used-for-production",
    DATABASE_PATH: join(dir, "test.db"),
  });
  config.webRoot = join(dir, "no-assets");
  let now = Date.now();
  let app = await buildApp(config, { now: () => now, usagePolling: false });
  const login = (value = password) =>
    app.inject({
      method: "POST",
      url: "/api/admin/login",
      headers: { origin },
      payload: { password: value },
    });
  const url = `/api/admin/usage?from=${now - 60000}&to=${now}`;
  try {
    assert.equal((await app.inject(url)).statusCode, 401);
    assert.equal(
      (
        await app.inject({
          url,
          headers: { authorization: `Bearer ${password}` },
        })
      ).statusCode,
      401,
    );
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: "/api/admin/login",
          payload: { password },
        })
      ).statusCode,
      403,
    );
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: "/api/admin/login",
          headers: { origin: "https://evil.example" },
          payload: { password },
        })
      ).statusCode,
      403,
    );
    assert.equal((await login("wrong")).statusCode, 401);
    const logged = await login();
    assert.equal(logged.statusCode, 200);
    const setCookie = String(logged.headers["set-cookie"]);
    for (const flag of [
      "HttpOnly",
      "Secure",
      "SameSite=Strict",
      "Path=/api/admin",
    ])
      assert.ok(setCookie.includes(flag));
    assert.ok(!setCookie.includes(password));
    const cookie = setCookie.split(";")[0]!;
    assert.equal(
      (await app.inject({ url, headers: { cookie } })).statusCode,
      200,
    );
    assert.equal(
      (
        await app.inject({
          url: "/api/admin/usage?from=NaN&to=Infinity",
          headers: { cookie },
        })
      ).statusCode,
      400,
    );
    const report = (await app.inject({ url, headers: { cookie } })).json();
    assert.equal(report.available, false);
    assert.equal(report.uploadedBytes, 0);
    assert.deepEqual(report.users, []);
    assert.equal(
      (await app.inject({ url, headers: { cookie } })).headers["cache-control"],
      "no-store",
    );
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: "/api/admin/logout",
          headers: { origin, cookie },
        })
      ).statusCode,
      204,
    );
    assert.equal(
      (await app.inject({ url, headers: { cookie } })).statusCode,
      401,
    );
    const second = String((await login()).headers["set-cookie"]).split(";")[0]!;
    now += 8 * 3600000 + 1;
    assert.equal(
      (
        await app.inject({
          url: "/api/admin/session",
          headers: { cookie: second },
        })
      ).statusCode,
      401,
    );
    const third = String((await login()).headers["set-cookie"]).split(";")[0]!;
    await app.close();
    app = await buildApp(config, { now: () => now, usagePolling: false });
    assert.equal(
      (
        await app.inject({
          url: "/api/admin/session",
          headers: { cookie: third },
        })
      ).statusCode,
      401,
    );
    for (let i = 0; i < 5; i++)
      assert.equal((await login("wrong")).statusCode, 401);
    assert.equal((await login("wrong")).statusCode, 429);
  } finally {
    await app.close();
    const resolved = realpathSync(dir);
    assert.equal(dirname(resolved), realpathSync(tmpdir()));
    assert.ok(basename(resolved).startsWith("freedomstream-admin-"));
    rmSync(resolved, { recursive: true });
  }
});
