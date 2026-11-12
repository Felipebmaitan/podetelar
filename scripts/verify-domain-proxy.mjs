// Local Docker acceptance only: ephemeral self-signed TLS is deliberately used
// here; production and native clients retain trusted certificate verification.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { randomBytes, createHash } from "node:crypto";
import { request } from "node:https";
import { productionConfiguration } from "./prepare-production.mjs";
import { buildApp } from "../apps/server/dist/app.js";
import { loadConfig } from "../apps/server/dist/config.js";

const exec = promisify(execFile);
const docker = (...args) =>
  exec("docker", args, { windowsHide: true, maxBuffer: 1024 * 1024 });
const dir = mkdtempSync(join(tmpdir(), "freedomstream-nginx-domains-"));
const container = `freedomstream-nginx-domains-${randomBytes(6).toString("hex")}`;
const secrets = Object.fromEntries(
  ["admin", "apiKey", "apiSecret", "turn"].map((key) => [
    key,
    randomBytes(32).toString("hex"),
  ]),
);
const publicUrl = "https://stream.example.test";
const directUrl = "https://stream-direct.example.test:8443";
const bytes = Buffer.from("proxy download fixture, never executed");
const filename = "freedomstream-windows-x64-0.1.0-aaaaaaaaaaaa.exe";
writeFileSync(join(dir, filename), bytes);
writeFileSync(
  join(dir, "publisher.json"),
  JSON.stringify({
    filename,
    version: "0.1.0",
    commit: "a".repeat(40),
    sha256: createHash("sha256").update(bytes).digest("hex"),
    size: bytes.length,
  }),
);
const config = loadConfig({
  PUBLIC_URL: publicUrl,
  DIRECT_URL: directUrl,
  DATABASE_PATH: join(dir, "rooms.db"),
  PUBLISHER_DIRECTORY: dir,
  ADMIN_TOKEN: secrets.admin,
  LIVEKIT_API_KEY: secrets.apiKey,
  LIVEKIT_API_SECRET: secrets.apiSecret,
});
const app = await buildApp(config, {
  usagePolling: false,
  mediaDirectory: {
    async members() {
      return [];
    },
    async participant() {
      return null;
    },
    async attribute() {},
    async remove() {},
  },
});
try {
  const upstream = await app.listen({ host: "0.0.0.0", port: 0 });
  const generated = productionConfiguration(
    "/opt/freedomstream",
    "203.0.113.9",
    "stream.example.test",
    secrets,
    "turn.example.test",
    "stream-direct.example.test",
  );
  const nginx = generated.nginx.replaceAll(
    "127.0.0.1:18080",
    `host.docker.internal:${new URL(upstream).port}`,
  );
  mkdirSync(join(dir, "shared/tls/current"), { recursive: true });
  writeFileSync(join(dir, "nginx.conf"), nginx);
  await docker(
    "run",
    "--rm",
    "-v",
    `${dir}:/opt/freedomstream`,
    "nginx:stable-alpine",
    "sh",
    "-c",
    "apk add --no-cache openssl >/dev/null && openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=stream-direct.example.test -keyout /opt/freedomstream/shared/tls/current/privkey.pem -out /opt/freedomstream/shared/tls/current/fullchain.pem 2>/dev/null",
  );
  await docker(
    "run",
    "-d",
    "--name",
    container,
    "--add-host",
    "host.docker.internal:host-gateway",
    "-p",
    "127.0.0.1::8443",
    "-p",
    "127.0.0.1::443",
    "-v",
    `${dir}:/opt/freedomstream:ro`,
    "-v",
    `${join(dir, "nginx.conf")}:/etc/nginx/conf.d/freedomstream.conf:ro`,
    "nginx:stable-alpine",
  );
  await docker("exec", container, "nginx", "-t");
  const port = async (internal) =>
    Number(
      (await docker("port", container, `${internal}/tcp`)).stdout
        .trim()
        .split(":")
        .at(-1),
    );
  const directPort = await port(8443);
  const webPort = await port(443);
  const send = (direct, method, path, headers = {}, body) =>
    new Promise((resolveResponse, reject) => {
      const origin = direct ? directUrl : publicUrl;
      const req = request(
        {
          hostname: "127.0.0.1",
          port: direct ? directPort : webPort,
          servername: new URL(origin).hostname,
          rejectUnauthorized: false,
          method,
          path,
          headers: { host: new URL(origin).host, origin, ...headers },
        },
        (res) => {
          const chunks = [];
          res.on("data", (chunk) => chunks.push(chunk));
          res.on("end", () =>
            resolveResponse({
              status: res.statusCode,
              headers: res.headers,
              body: Buffer.concat(chunks),
            }),
          );
        },
      );
      req.on("error", reject);
      req.setTimeout(10_000, () =>
        req.destroy(new Error("Local proxy request timed out")),
      );
      req.end(body);
    });
  const created = await send(
    false,
    "POST",
    "/api/rooms",
    {
      authorization: `Bearer ${secrets.admin}`,
      "content-type": "application/json",
    },
    JSON.stringify({ name: "Proxy", displayName: "Fixture" }),
  );
  assert.equal(created.status, 201);
  const room = JSON.parse(created.body).room.id;
  const cookie = created.headers["set-cookie"][0].split(";")[0];
  const path = `/api/rooms/${room}/network-test`;
  assert.equal((await send(false, "POST", path, { cookie })).status, 421);
  const probe = await send(true, "POST", path, { cookie });
  assert.equal(probe.status, 200, "nginx must preserve the direct Host port");
  await send(true, "DELETE", path, {
    cookie,
    "x-network-probe": JSON.parse(probe.body).probe,
  });
  const redirect = await send(
    false,
    "GET",
    `/api/rooms/${room}/publisher/windows.exe`,
    { cookie },
  );
  assert.equal(redirect.status, 303);
  const ticket = new URL(redirect.headers.location).hash.slice(8);
  assert.equal(
    (
      await send(
        false,
        "POST",
        "/api/publisher/download",
        { "content-type": "application/x-www-form-urlencoded" },
        `ticket=${ticket}`,
      )
    ).status,
    421,
  );
  const downloaded = await send(
    true,
    "POST",
    "/api/publisher/download",
    { "content-type": "application/x-www-form-urlencoded" },
    `ticket=${ticket}`,
  );
  assert.equal(downloaded.status, 200);
  assert.deepEqual(downloaded.body, bytes);
  console.log(
    "PASS: generated nginx TLS configuration, public bulk denial, direct Host/port preservation, authenticated probe and ticket download.",
  );
} finally {
  await docker("rm", "-f", container).catch(() => {});
  await app.close();
  const target = resolve(dir);
  assert.equal(dirname(target), resolve(tmpdir()));
  assert.ok(
    target.split(/[\\/]/).at(-1).startsWith("freedomstream-nginx-domains-"),
  );
  rmSync(target, { recursive: true });
}
