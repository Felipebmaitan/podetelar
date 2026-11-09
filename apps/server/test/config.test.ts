import assert from "node:assert/strict";
import { test } from "node:test";
import { loadConfig } from "../src/config.js";

const valid = {
  ADMIN_TOKEN: "admin-secret-with-at-least-thirty-two-characters",
  LIVEKIT_API_KEY: "test-api-key",
  LIVEKIT_API_SECRET: "livekit-secret-with-at-least-thirty-two-characters",
};

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
