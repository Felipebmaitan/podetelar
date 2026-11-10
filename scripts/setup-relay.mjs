import { randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  chmodSync,
} from "node:fs";
import { isIP } from "node:net";
import { fileURLToPath, pathToFileURL } from "node:url";

export function relayConfiguration(env, secret) {
  const host = env.TURN_PUBLIC_HOST;
  const ip = env.TURN_PUBLIC_IP;
  const peer = env.SFU_PUBLIC_IP ?? ip;
  const port = Number(env.TURN_TLS_PORT ?? 5349);
  if (
    !host ||
    host.length > 253 ||
    !host
      .split(".")
      .every((label) => /^[a-z\d](?:[a-z\d-]{0,61}[a-z\d])?$/i.test(label)) ||
    !ip ||
    isIP(ip) !== 4 ||
    !peer ||
    isIP(peer) !== 4
  )
    throw new Error(
      "Set TURN_PUBLIC_HOST to a certificate hostname/IP and TURN_PUBLIC_IP (optionally SFU_PUBLIC_IP) to IPv4 addresses.",
    );
  if (
    !Number.isInteger(port) ||
    port < 1 ||
    port > 65535 ||
    [7880, 7881, 7882].includes(port) ||
    (port >= 30000 && port <= 30127)
  )
    throw new Error(
      "TURN_TLS_PORT must be a free TCP port outside the SFU/relay range.",
    );
  if (!/^[a-f0-9]{64}$/.test(secret))
    throw new Error(
      "The private TURN secret must contain 32 random bytes as hexadecimal.",
    );
  return {
    livekit: `port: 7880
prometheus:
  port: 6789
bind_addresses: [0.0.0.0]
rtc:
  tcp_port: 7881
  udp_port: 7882
  node_ip: ${peer}
  use_external_ip: false
  allow_tcp_fallback: true
  turn_servers:
    - host: ${host}
      port: ${port}
      protocol: tls
      secret_file: /run/secrets/turn_secret
      ttl: 300
logging:
  level: error
`,
    coturn: `listening-ip=0.0.0.0
relay-ip=${ip}
tls-listening-port=${port}
listening-port=3478
min-port=30000
max-port=30127
realm=freedomstream
server-name=freedomstream
use-auth-secret
static-auth-secret=${secret}
fingerprint
no-udp
no-tcp
no-dtls
no-tlsv1
no-tlsv1_1
no-cli
no-tcp-relay
no-multicast-peers
no-auth-pings
no-dynamic-ip-list
no-dynamic-realms
allowed-peer-ip=${peer}
denied-peer-ip=0.0.0.0-255.255.255.255
denied-peer-ip=::-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff
user-quota=8
total-quota=64
max-bps=100000000
# Capacity accounts for reservations at allocation time, even for idle ICE
# candidates. Reserving the whole port for each allocation admits only one.
# Per-allocation policing stays enabled; aggregate host traffic is not shaped.
bps-capacity=0
relay-threads=1
max-allocate-timeout=10
max-allocate-lifetime=120
cert=/run/tls/fullchain.pem
pkey=/run/tls/privkey.pem
pidfile=/tmp/turn.pid
userdb=/tmp/turn.db
log-file=/dev/null
simple-log
`,
  };
}

function main() {
  const root = new URL("../.local/relay/", import.meta.url);
  const secretPath = new URL("turn-secret", root);
  const secret = existsSync(secretPath)
    ? readFileSync(secretPath, "utf8").trim()
    : randomBytes(32).toString("hex");
  const files = relayConfiguration(process.env, secret);
  mkdirSync(root, { recursive: true, mode: 0o700 });
  for (const [name, content] of [
    ["turn-secret", secret],
    ["livekit.yaml", files.livekit],
    ["turnserver.conf", files.coturn],
  ]) {
    const path = new URL(name, root);
    writeFileSync(path, content, { mode: 0o600 });
    chmodSync(path, 0o600);
  }
  console.log(
    `Prepared private relay files in ${fileURLToPath(root)}. Keep them private and supply a valid TLS certificate/key before starting the Linux relay overlay.`,
  );
}
if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url)
  main();
