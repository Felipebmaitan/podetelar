import assert from "node:assert/strict";
import { test } from "node:test";
import { relayConfiguration } from "./setup-relay.mjs";
const env = {
  TURN_PUBLIC_HOST: "relay.example.com",
  TURN_PUBLIC_IP: "192.0.2.1",
};
test("relay configuration bounds the service and scopes authentication to expiring credentials and the SFU peer", () => {
  const secret = "a".repeat(64);
  const output = relayConfiguration(env, secret);
  assert.ok(!output.livekit.includes(secret));
  assert.match(output.livekit, /protocol: tls/);
  assert.match(output.livekit, /ttl: 300/);
  assert.match(output.livekit, /secret_file:/);
  for (const expected of [
    "use-auth-secret",
    "allowed-peer-ip=192.0.2.1",
    "denied-peer-ip=0.0.0.0-255.255.255.255",
    "no-tcp-relay",
    "no-cli",
    "total-quota=32",
    "user-quota=4",
    "relay-threads=1",
  ])
    assert.ok(output.coturn.split("\n").includes(expected));
  assert.ok(!output.coturn.includes("no-auth\n"));
});
test("relay configuration rejects URLs, config injection, malformed secrets and conflicting ports", () => {
  for (const changes of [
    { TURN_PUBLIC_HOST: "https://relay.example.com" },
    { TURN_PUBLIC_HOST: "relay.example.com\nno-auth" },
    { TURN_PUBLIC_IP: "0.0.0.0\nno-auth" },
    { TURN_TLS_PORT: "7880" },
    { TURN_TLS_PORT: "30031" },
    { TURN_TLS_PORT: "65536" },
  ])
    assert.throws(() =>
      relayConfiguration({ ...env, ...changes }, "a".repeat(64)),
    );
  assert.throws(() => relayConfiguration(env, "not-a-secret"));
});
