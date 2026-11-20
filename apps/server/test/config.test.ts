import assert from "node:assert/strict";
import { test } from "node:test";
import { loadConfig } from "../src/config.js";

const valid = {
  ADMIN_TOKEN: "admin-secret-with-at-least-thirty-two-characters",
  LIVEKIT_API_KEY: "test-api-key",
  LIVEKIT_API_SECRET: "livekit-secret-with-at-least-thirty-two-characters",
};

test("proxy trust is opt-in and limited to one private peer address", () => {
  assert.equal(loadConfig(valid).trustedProxy, undefined);
  assert.equal(
    loadConfig({ ...valid, TRUSTED_PROXY: "172.30.77.1" }).trustedProxy,
    "172.30.77.1",
  );
  for (const TRUSTED_PROXY of [
    "true",
    "0.0.0.0/0",
    "loopback",
    "172.30.77.1,1.2.3.4",
  ])
    assert.throws(
      () => loadConfig({ ...valid, TRUSTED_PROXY }),
      /TRUSTED_PROXY/,
    );
});

test("configuration rejects missing/placeholder credentials and invalid ports", () => {
  assert.throws(() => loadConfig({}), /ADMIN_TOKEN/);
  assert.throws(
    () => loadConfig({ ...valid, ADMIN_TOKEN: "GENERATE_WITH_NPM_RUN_SETUP" }),
    /ADMIN_TOKEN/,
  );
  for (const PORT of ["-1", "0", "65536", "abc", "1.5"])
    assert.throws(() => loadConfig({ ...valid, PORT }), /PORT/);
});

test("remote deployments require HTTPS and encrypted media URLs", () => {
  assert.throws(
    () => loadConfig({ ...valid, PUBLIC_URL: "http://stream.example" }),
    /HTTPS/,
  );
  assert.throws(
    () => loadConfig({ ...valid, PUBLIC_URL: "https://stream.example/path" }),
    /origin/,
  );
  assert.throws(
    () =>
      loadConfig({ ...valid, PUBLIC_URL: "https://user:pass@stream.example" }),
    /origin/,
  );
  assert.throws(
    () => loadConfig({ ...valid, LIVEKIT_URL: "https://stream.example" }),
    /derived/,
  );
  const production = loadConfig({
    ...valid,
    PUBLIC_URL: "https://stream.example",
    LIVEKIT_INTERNAL_URL: "http://livekit:7880",
  });
  assert.equal(production.publicUrl, "https://stream.example");
  assert.equal(production.livekitUrl, "wss://stream.example/media");
  assert.equal(production.livekitInternalUrl, "http://livekit:7880");
});

test("administrative media access uses a separately configured private HTTP(S) origin", () => {
  assert.equal(loadConfig(valid).livekitUrl, "ws://127.0.0.1:5173/media");
  assert.equal(
    loadConfig({ ...valid, LIVEKIT_INTERNAL_URL: "http://livekit:7880" })
      .livekitInternalUrl,
    "http://livekit:7880",
  );
  for (const value of [
    "ws://livekit",
    "https://user:secret@livekit",
    "http://livekit/path",
    "http://livekit?token=secret",
    "http://livekit#secret",
  ])
    assert.throws(
      () => loadConfig({ ...valid, LIVEKIT_INTERNAL_URL: value }),
      /INTERNAL/,
    );
});

test("direct origin defaults to the website and rejects paths, secrets and TLS downgrade", () => {
  assert.equal(loadConfig(valid).directUrl, loadConfig(valid).publicUrl);
  const split = loadConfig({
    ...valid,
    PUBLIC_URL: "https://stream.maitan.me",
    DIRECT_URL: "https://stream-direct.maitan.me:8443",
  });
  assert.equal(split.publicUrl, "https://stream.maitan.me");
  assert.equal(split.livekitUrl, "wss://stream-direct.maitan.me:8443/media");
  for (const DIRECT_URL of [
    "http://remote.test",
    "https://user:secret@direct.test",
    "https://direct.test/path",
    "https://direct.test?q=secret",
    "https://direct.test/#secret",
    "http://localhost:3000",
    "file:///tmp",
  ]) {
    assert.throws(
      () =>
        loadConfig({
          ...valid,
          PUBLIC_URL: "https://stream.maitan.me",
          DIRECT_URL,
        }),
      /DIRECT_URL/,
    );
  }
});
