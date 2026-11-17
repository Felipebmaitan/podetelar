import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, basename } from "node:path";
import { test } from "node:test";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { Accounts } from "../src/accounts.js";
import { DatabaseSync } from "node:sqlite";
const origin = "http://localhost:5173",
  admin = "account-test-admin-not-a-production-secret",
  user = "123456789012345678";
test("Discord sign-in binds browser state and native proof, persists opaque logins and supports admin revocation", async () => {
  const dir = mkdtempSync(join(tmpdir(), "podetelar-auth-"));
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: admin,
    LIVEKIT_API_KEY: "auth-test-key",
    LIVEKIT_API_SECRET: "auth-test-media-not-a-production-secret",
    DATABASE_PATH: join(dir, "rooms.db"),
    DISCORD_APPLICATION_ID: "223456789012345678",
    DISCORD_PUBLIC_KEY: "0".repeat(64),
    DISCORD_GUILD_IDS: "*",
    DISCORD_CLIENT_SECRET: "synthetic-oauth-client-secret",
  });
  config.webRoot = join(dir, "no-web");
  const fetcher: typeof fetch = async (url, init) => {
    if (String(url).endsWith("/oauth2/token")) {
      assert.equal(init?.method, "POST");
      return new Response(
        JSON.stringify({
          access_token: "synthetic-access-token",
          token_type: "Bearer",
          scope: "identify",
          refresh_token: "must-not-be-persisted",
        }),
      );
    }
    assert.equal(String(url), "https://discord.com/api/v10/users/@me");
    return new Response(
      JSON.stringify({
        id: user,
        username: "friend",
        global_name: "Discord friend",
        avatar: null,
      }),
    );
  };
  const build = () =>
    buildApp(config, { usagePolling: false, discordFetch: fetcher });
  let app = await build();
  try {
    const proof = randomBytes(32).toString("base64url"),
      challenge = createHash("sha256").update(proof).digest("hex");
    const start = await app.inject({
      method: "POST",
      url: "/api/auth/discord/start",
      headers: { origin },
      payload: { challenge },
    });
    assert.equal(start.statusCode, 200);
    const requestId = start.json().requestId;
    const authorize = await app.inject({
      url:
        new URL(start.json().authorizationUrl).pathname +
        new URL(start.json().authorizationUrl).search,
    });
    assert.equal(authorize.statusCode, 302);
    const redirect = new URL(String(authorize.headers.location));
    assert.equal(redirect.origin, "https://discord.com");
    assert.equal(redirect.searchParams.get("scope"), "identify");
    const state = redirect.searchParams.get("state"),
      cookie = String(authorize.headers["set-cookie"]).split(";")[0]!;
    const callback = `/api/auth/discord/callback?state=${state}&code=synthetic-code`;
    assert.equal((await app.inject({ url: callback })).statusCode, 400);
    assert.equal(
      (await app.inject({ url: callback, headers: { cookie } })).headers
        .location,
      "/auth/discord/complete",
    );
    assert.equal(
      (await app.inject({ url: callback, headers: { cookie } })).statusCode,
      400,
      "callback is single-use",
    );
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: "/api/auth/discord/poll",
          headers: { origin },
          payload: { requestId, proof: randomBytes(32).toString("base64url") },
        })
      ).statusCode,
      401,
    );
    const poll = await app.inject({
      method: "POST",
      url: "/api/auth/discord/poll",
      headers: { origin },
      payload: { requestId, proof },
    });
    assert.equal(poll.statusCode, 200);
    const token = poll.json().token;
    assert.equal(poll.json().account.displayName, "Discord friend");
    assert.ok(!poll.body.includes("synthetic-access-token"));
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: "/api/auth/discord/poll",
          headers: { origin },
          payload: { requestId, proof },
        })
      ).statusCode,
      401,
    );
    await app.close();
    app = await build();
    assert.equal(
      (
        await app.inject({
          url: "/api/auth/session",
          headers: { authorization: `Bearer ${token}` },
        })
      ).statusCode,
      200,
    );
    assert.equal(
      (
        await app.inject({
          url: "/api/admin/accounts",
          headers: { authorization: `Bearer ${token}` },
        })
      ).statusCode,
      401,
    );
    const login = await app.inject({
      method: "POST",
      url: "/api/admin/login",
      headers: { origin },
      payload: { password: admin },
    });
    const adminCookie = String(login.headers["set-cookie"]).split(";")[0]!;
    const directory = await app.inject({
      url: "/api/admin/accounts",
      headers: { cookie: adminCookie },
    });
    assert.equal(directory.statusCode, 200);
    assert.ok(!directory.body.includes(token));
    assert.equal(directory.json().accounts[0].loginCount, 1);
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: `/api/admin/accounts/${user}`,
          headers: { origin, cookie: adminCookie },
          payload: { action: "disable" },
        })
      ).statusCode,
      204,
    );
    assert.equal(
      (
        await app.inject({
          url: "/api/auth/session",
          headers: { authorization: `Bearer ${token}` },
        })
      ).statusCode,
      401,
    );
  } finally {
    await app.close();
    const real = realpathSync(dir);
    assert.equal(dirname(real), realpathSync(tmpdir()));
    assert.ok(basename(real).startsWith("podetelar-auth-"));
    rmSync(real, { recursive: true });
  }
});

test("PodeTelar logins expire, are origin scoped, and store only credential hashes", () => {
  const dir = mkdtempSync(join(tmpdir(), "podetelar-auth-")),
    path = join(dir, "accounts.db");
  let now = 100000;
  const accounts = new Accounts(path, "https://one.example", () => now),
    other = new Accounts(path, "https://two.example", () => now);
  const issue = () => {
    const proof = randomBytes(32).toString("base64url"),
      flow = accounts.start(createHash("sha256").update(proof).digest("hex"))!,
      browser = accounts.authorize(flow.id)!;
    const id = accounts.consume(browser.state, browser.browser)!;
    accounts.complete(id, { id: user, username: "Friend" });
    const result = accounts.poll(flow.id, proof);
    assert.equal(result.status, "ready");
    if (result.status !== "ready") throw Error("No login");
    return result;
  };
  try {
    const first = issue();
    assert.ok(accounts.authenticate(first.token));
    assert.equal(other.authenticate(first.token), null);
    const db = new DatabaseSync(path, { readOnly: true });
    try {
      const row = db.prepare("SELECT token_hash FROM discord_logins").get();
      assert.notEqual(row!.token_hash, first.token);
      assert.equal(String(row!.token_hash).length, 64);
    } finally {
      db.close();
    }
    accounts.logout(first.token);
    assert.equal(accounts.authenticate(first.token), null);
    const second = issue();
    now += 31 * 86400_000;
    assert.equal(accounts.authenticate(second.token), null);
    const pending = accounts.start("a".repeat(64))!;
    now += 301000;
    assert.equal(accounts.authorize(pending.id), null);
  } finally {
    accounts.close();
    other.close();
    const real = realpathSync(dir);
    assert.equal(dirname(real), realpathSync(tmpdir()));
    assert.ok(basename(real).startsWith("podetelar-auth-"));
    rmSync(real, { recursive: true });
  }
});
