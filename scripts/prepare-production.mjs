import { randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  chmodSync,
} from "node:fs";
import { isIP } from "node:net";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { relayConfiguration } from "./setup-relay.mjs";

export function productionConfiguration(
  root,
  ip,
  host,
  secrets,
  turnHost = host,
  directHost,
) {
  if (
    !/^\/[a-zA-Z0-9_/-]+$/.test(root) ||
    root.length < 8 ||
    root.includes("..") ||
    isIP(ip) !== 4
  )
    throw new Error(
      "Use an absolute private Linux installation path and public IPv4 address.",
    );
  if (
    !host ||
    !host
      .split(".")
      .every((label) => /^[a-z\d](?:[a-z\d-]{0,61}[a-z\d])?$/i.test(label))
  )
    throw new Error("Use a certificate IPv4 address or DNS hostname.");
  for (const key of ["admin", "apiKey", "apiSecret", "turn"])
    if (!/^[a-f0-9]{64}$/.test(secrets[key]))
      throw new Error("Invalid private deployment secrets.");
  if (
    isIP(turnHost) ||
    !turnHost ||
    !turnHost
      .split(".")
      .every((label) => /^[a-z\d](?:[a-z\d-]{0,61}[a-z\d])?$/i.test(label))
  )
    throw new Error(
      "Native TURN/TLS needs a DNS certificate name. Supply TURN_HOST separately when the room uses an IP address.",
    );
  const state = `${root}/shared`;
  if (
    directHost &&
    (isIP(host) ||
      isIP(directHost) ||
      directHost === host ||
      !directHost
        .split(".")
        .every((label) => /^[a-z\d](?:[a-z\d-]{0,61}[a-z\d])?$/i.test(label)))
  )
    throw new Error(
      "Split deployment requires distinct public and direct DNS hostnames.",
    );
  const publicUrl = directHost ? `https://${host}` : `https://${host}:8443`;
  const directUrl = directHost ? `https://${directHost}:8443` : publicUrl;
  const relay = relayConfiguration(
    { TURN_PUBLIC_HOST: turnHost, TURN_PUBLIC_IP: ip },
    secrets.turn,
  );
  const result = {
    environment: `STATE_DIR=${state}\nPUBLIC_URL=${publicUrl}\nDIRECT_URL=${directUrl}\nTURN_HOST=${turnHost}\nADMIN_TOKEN=${secrets.admin}\nLIVEKIT_API_KEY=${secrets.apiKey}\nLIVEKIT_API_SECRET=${secrets.apiSecret}\n`,
    livekit: `${relay.livekit}keys:\n  ${secrets.apiKey}: ${secrets.apiSecret}\n`,
    coturn: relay.coturn
      .replace("/run/tls/fullchain.pem", "/run/tls/current/fullchain.pem")
      .replace("/run/tls/privkey.pem", "/run/tls/current/privkey.pem"),
    acme: `# FreedomStream only. Existing port-80 virtual hosts remain unchanged.\nserver {\n  listen 80;\n  server_name ${host} ${turnHost};\n  access_log off;\n  error_log /dev/null;\n  location ^~ /.well-known/acme-challenge/ { root /var/www/freedomstream-acme; default_type text/plain; try_files $uri =404; }\n  location / { return 404; }\n}\n`,
    nginx: `# FreedomStream HTTPS; deliberately separate from existing business 443.\nserver {\n  listen 8443 ssl;\n  server_name ${host};\n  ssl_certificate ${state}/tls/current/fullchain.pem;\n  ssl_certificate_key ${state}/tls/current/privkey.pem;\n  ssl_protocols TLSv1.2 TLSv1.3;\n  access_log off;\n  error_log /dev/null;\n  client_max_body_size 256k;\n  location ~ "^/api/rooms/[A-Za-z0-9_-]{22}/network-test/upload$" {\n    client_max_body_size 48m;\n    proxy_request_buffering off;\n    proxy_buffering off;\n    proxy_pass http://127.0.0.1:18080;\n    proxy_http_version 1.1;\n    proxy_set_header Host $host;\n    proxy_set_header X-Forwarded-For $realip_remote_addr;\n    proxy_set_header X-Forwarded-Proto https;\n    proxy_set_header Connection "";\n    proxy_read_timeout 8s;\n    proxy_send_timeout 8s;\n  }\n  location / {\n    proxy_pass http://127.0.0.1:18080;\n    proxy_http_version 1.1;\n    proxy_set_header Host $host;\n    proxy_set_header X-Forwarded-For $realip_remote_addr;\n    proxy_set_header X-Forwarded-Proto https;\n    proxy_set_header Upgrade $http_upgrade;\n    proxy_set_header Connection "upgrade";\n    proxy_read_timeout 75s;\n    proxy_send_timeout 75s;\n    proxy_buffering off;\n  }\n}\n`,
  };
  if (directHost) {
    result.acme = result.acme.replace(
      `server_name ${host} ${turnHost};`,
      `server_name ${host} ${directHost} ${turnHost};`,
    );
    const direct = result.nginx.replace(
      `server_name ${host};`,
      `server_name ${directHost};`,
    );
    // The website vhost has no bulk transfer path. Native APIs use DIRECT_URL;
    // the direct listener stays on 8443 to preserve existing 443 firewall rules.
    const website = result.nginx
      .replace("listen 8443 ssl;", "listen 443 ssl;")
      .replace(
        /  location ~ [\s\S]*?\n  }\n  location \/ \{/,
        '  location ~ "^/api/rooms/[A-Za-z0-9_-]{22}/network-test(?:/|$)" { return 421; }\n  location = /api/publisher/download { return 421; }\n  location / {',
      );
    result.nginx = `${website}\n${direct}`;
  }
  // $host drops a non-default port. The app validates the complete direct
  // authority (including 8443) before serving bulk traffic.
  result.nginx = result.nginx.replaceAll(
    "proxy_set_header Host $host;",
    "proxy_set_header Host $http_host;",
  );
  return result;
}

if (
  process.argv[1] &&
  pathToFileURL(process.argv[1]).href === import.meta.url
) {
  const [rootArg, ip, host = ip, turnHost = host, directHost] =
    process.argv.slice(2);
  const root = rootArg && resolve(rootArg);
  if (!root || !ip)
    throw new Error(
      "Usage: prepare-production.mjs /opt/freedomstream PUBLIC_IPV4 ROOM_HOST TURN_HOST [DIRECT_HOST]",
    );
  const state = join(root, "shared");
  const path = join(state, "secrets.json");
  const secrets = existsSync(path)
    ? JSON.parse(readFileSync(path, "utf8"))
    : Object.fromEntries(
        ["admin", "apiKey", "apiSecret", "turn"].map((key) => [
          key,
          randomBytes(32).toString("hex"),
        ]),
      );
  const config = productionConfiguration(
    root,
    ip,
    host,
    secrets,
    turnHost,
    directHost,
  );
  for (const directory of [
    state,
    `${state}/data`,
    `${state}/publisher`,
    `${state}/backups`,
    `${state}/tls`,
    `${state}/certbot`,
  ])
    mkdirSync(directory, { recursive: true, mode: 0o700 });
  const files = {
    "secrets.json": JSON.stringify(secrets),
    ".env": config.environment,
    "turn-secret": secrets.turn,
    "livekit.yaml": config.livekit,
    "turnserver.conf": config.coturn,
    "nginx-acme.conf": config.acme,
    "nginx-https.conf": config.nginx,
  };
  for (const [name, data] of Object.entries(files)) {
    const file = join(state, name);
    writeFileSync(file, data, { mode: 0o600 });
    chmodSync(file, 0o600);
  }
  console.log(
    "Prepared production files; existing private keys were preserved. Supply an immutable APP_IMAGE in release.env, publisher artifacts and certificates before starting.",
  );
}
