import assert from "node:assert/strict";
import { test } from "node:test";
import { productionConfiguration } from "./prepare-production.mjs";
import { PREVIEW_MAX_BYTES } from "@freedomstream/contracts";
const secrets = Object.fromEntries(
  ["admin", "apiKey", "apiSecret", "turn"].map((key, i) => [
    key,
    String(i + 1).repeat(64),
  ]),
);

test("split domains retain the gateway and keep bulk traffic off the website listener", () => {
  const config = productionConfiguration(
    "/opt/freedomstream",
    "203.0.113.9",
    "stream.maitan.me",
    secrets,
    "turn.maitan.me",
    "stream-direct.maitan.me",
  );
  assert.match(
    config.environment,
    /PUBLIC_URL=https:\/\/stream.maitan.me\nDIRECT_URL=https:\/\/stream-direct.maitan.me:8443\n/,
  );
  const [website, direct] = config.nginx.split("\n\n");
  assert.match(website, /listen 443 ssl/);
  assert.match(website, /network-test\(\?:\/\|\$\).*return 421/);
  assert.ok(!website.includes("client_max_body_size 48m"));
  assert.match(direct, /listen 8443 ssl/);
  assert.match(direct, /server_name stream-direct.maitan.me/);
  assert.match(direct, /client_max_body_size 48m/);
  assert.match(direct, /proxy_set_header Host \$http_host;/);
  assert.ok(!config.nginx.includes("proxy_set_header Host $host;"));
  assert.match(
    config.acme,
    /server_name stream.maitan.me stream-direct.maitan.me turn.maitan.me/,
  );
  assert.match(config.livekit, /host: turn.maitan.me/);
  for (const host of [
    "stream.maitan.me",
    "203.0.113.9",
    "direct.test;",
    "direct.test\nX=bad",
  ])
    assert.throws(
      () =>
        productionConfiguration(
          "/opt/freedomstream",
          "203.0.113.9",
          "stream.maitan.me",
          secrets,
          "turn.maitan.me",
          host,
        ),
      /DNS/,
    );
});
test("production config preserves the admission gateway and isolated certificate paths", () => {
  const config = productionConfiguration(
    "/opt/freedomstream",
    "203.0.113.9",
    "203.0.113.9",
    secrets,
    "relay.example.com",
  );
  assert.match(config.environment, /PUBLIC_URL=https:\/\/203\.0\.113\.9:8443/);
  assert.match(config.nginx, /proxy_pass http:\/\/127\.0\.0\.1:18080/);
  assert.match(config.nginx, /X-Forwarded-For \$realip_remote_addr/);
  assert.match(config.nginx, /access_log off/);
  // Legal native previews must pass nginx before the API validates their bytes.
  const previewLimit = config.nginx.match(/client_max_body_size (\d+)k;/);
  assert.ok(previewLimit);
  assert.equal(Number(previewLimit[1]) * 1024, PREVIEW_MAX_BYTES);
  assert.match(
    config.nginx,
    /location ~ "\^\/api\/rooms\/\[A-Za-z0-9_-\]\{22\}\/network-test\/upload\$" \{/,
  );
  const upload = config.nginx
    .split('network-test/upload$" {')[1]
    ?.split("\n  }")[0];
  assert.ok(upload);
  assert.match(upload, /client_max_body_size 48m/);
  assert.match(upload, /proxy_request_buffering off/);
  assert.match(upload, /X-Forwarded-For \$realip_remote_addr/);
  assert.match(config.coturn, /cert=\/run\/tls\/current\/fullchain.pem/);
  assert.match(config.livekit, /ttl: 300/);
  assert.match(config.livekit, /host: relay.example.com/);
  assert.match(config.acme, /location \/ \{ return 404/);
  assert.ok(!config.nginx.includes(secrets.admin));
});
test("deployment identifiers cannot inject config or shell lines", () => {
  assert.throws(
    () =>
      productionConfiguration(
        "/opt/freedomstream",
        "203.0.113.9",
        "203.0.113.9",
        secrets,
      ),
    /DNS certificate/,
  );
  for (const root of [
    "/",
    "/opt/freedomstream\nX=bad",
    "/opt/../etc",
    "C:/app",
    "/opt/a b",
  ])
    assert.throws(() =>
      productionConfiguration(root, "203.0.113.9", "stream.example", secrets),
    );
  for (const host of [
    "stream.example;",
    "../etc",
    "host\nlisten 22",
    "https://host",
    "[::1]",
  ])
    assert.throws(() =>
      productionConfiguration(
        "/opt/freedomstream",
        "203.0.113.9",
        host,
        secrets,
      ),
    );
});
