// Runs the native acceptance suite against an isolated Windows SFU and app API.
// It neither changes Docker/Windows settings nor deploys anything to the VPS.
import { createHash, randomUUID } from "node:crypto";
import { spawn, execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { networkInterfaces, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

if (
  process.platform !== "win32" ||
  process.versions.node.split(".")[0] !== "24"
)
  throw new Error("Use Windows and Node 24 for the native room smoke test.");
const root = resolve(import.meta.dirname, "..");
process.chdir(root);
const soak = process.argv.includes("--soak");
const appPort = Number(
  process.env.FREEDOMSTREAM_SMOKE_APP_PORT ?? (soak ? 13010 : 3010),
);
const mediaPort = Number(
  process.env.FREEDOMSTREAM_SMOKE_SFU_PORT ?? (soak ? 18880 : 17880),
);
if (
  ![appPort, mediaPort].every(
    (port) => Number.isInteger(port) && port >= 1024 && port <= 65533,
  ) ||
  Math.abs(appPort - mediaPort) <= 2
)
  throw new Error("Choose distinct unprivileged native smoke port ranges.");
const origin = `http://localhost:${appPort}`;
const mediaOrigin = `http://127.0.0.1:${mediaPort}`;
const occupied = await Promise.all(
  [`${origin}/api/health`, mediaOrigin].map(async (url) => {
    try {
      await fetch(url, { signal: AbortSignal.timeout(1000) });
      return true;
    } catch {
      return false;
    }
  }),
);
if (occupied.some(Boolean))
  throw new Error(
    "Native acceptance ports are already in use. Stop the other local run first.",
  );
const pin = JSON.parse(readFileSync(join(root, "deploy/test-livekit.json")));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const downloads = join(root, ".local/downloads");
mkdirSync(downloads, { recursive: true });
const archive = join(downloads, `livekit_${pin.version}_windows_amd64.zip`);
if (!existsSync(archive)) {
  const response = await fetch(pin.url, {
    signal: AbortSignal.timeout(60_000),
  });
  if (!response.ok || !response.body)
    throw new Error("Test SFU download failed.");
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > 128 * 1024 * 1024)
      throw new Error("Test SFU archive exceeded its limit.");
    chunks.push(chunk);
  }
  const bytes = Buffer.concat(chunks);
  if (hash(bytes) !== pin.sha256)
    throw new Error("Test SFU checksum mismatch.");
  writeFileSync(archive, bytes, { flag: "wx" });
}
if (hash(readFileSync(archive)) !== pin.sha256)
  throw new Error("Test SFU checksum mismatch.");
const run = join(root, ".local", `native-room-smoke-${randomUUID()}`);
mkdirSync(run);
const entries = execFileSync("tar", ["-tf", archive], {
  encoding: "utf8",
  windowsHide: true,
})
  .trim()
  .split(/\r?\n/);
if (entries.some((entry) => !["LICENSE", "livekit-server.exe"].includes(entry)))
  throw new Error("Unexpected test SFU archive paths.");
const serverDirectory = join(root, ".local", `livekit-test-${pin.version}`);
mkdirSync(serverDirectory, { recursive: true });
const serverExecutable = join(serverDirectory, "livekit-server.exe");
if (!existsSync(serverExecutable)) {
  execFileSync("tar", ["-xf", archive, "-C", serverDirectory], {
    windowsHide: true,
  });
}
if (hash(readFileSync(serverExecutable)) !== pin.executableSha256)
  throw new Error("Cached test SFU checksum mismatch.");
for (const key of ["ADMIN_TOKEN", "LIVEKIT_API_KEY", "LIVEKIT_API_SECRET"])
  if (!process.env[key])
    throw new Error("Run setup and load the local .env before testing.");
const candidates = Object.values(networkInterfaces())
  .flat()
  .filter(
    (ip) =>
      ip &&
      ip.family === "IPv4" &&
      !ip.internal &&
      /^(192\.168\.|10\.|172\.(1[6-9]|2[0-9]|3[01])\.)/.test(ip.address),
  );
const nodeIp =
  process.env.FREEDOMSTREAM_TEST_NODE_IP ??
  candidates.find((ip) => ip.address.startsWith("192.168."))?.address ??
  candidates[0]?.address;
if (!nodeIp || !/^\d{1,3}(\.\d{1,3}){3}$/.test(nodeIp))
  throw new Error(
    "Set FREEDOMSTREAM_TEST_NODE_IP to a reachable local IPv4 address.",
  );
const config = join(run, "livekit.yaml");
writeFileSync(
  config,
  JSON.stringify({
    port: mediaPort,
    room: { sync_streams: true },
    bind_addresses: ["127.0.0.1"],
    rtc: {
      tcp_port: mediaPort + 1,
      udp_port: mediaPort + 2,
      node_ip: nodeIp,
      use_external_ip: false,
    },
    keys: { [process.env.LIVEKIT_API_KEY]: process.env.LIVEKIT_API_SECRET },
    logging: { level: "error" },
  }),
  { flag: "wx" },
);
const children = [];
const start = (file, args, env) => {
  const child = spawn(file, args, {
    cwd: root,
    env,
    windowsHide: true,
    stdio: "ignore",
  });
  child.on("error", () => {});
  children.push(child);
  return child;
};
const ready = async (url) => {
  for (let i = 0; i < 40; i++) {
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(1000) })).ok) return;
    } catch {
      /* Startup is bounded below. */
    }
    await delay(250);
  }
  throw new Error(
    `Isolated native smoke did not start on app ${appPort}, SFU ${mediaPort}–${mediaPort + 2}.`,
  );
};
try {
  const sfu = start(
    join(serverDirectory, "livekit-server.exe"),
    ["--config", config],
    process.env,
  );
  await ready(mediaOrigin);
  start(process.execPath, ["apps/server/dist/main.js"], {
    ...process.env,
    PUBLIC_URL: origin,
    HOST: "127.0.0.1",
    PORT: String(appPort),
    DATABASE_PATH: join(run, "rooms.db"),
    LIVEKIT_INTERNAL_URL: mediaOrigin,
  });
  await ready(`${origin}/api/health`);
  const gst = resolve(
    process.env.FREEDOMSTREAM_GSTREAMER ?? ".local/gstreamer",
  );
  const args = [
    "node_modules/@playwright/test/cli.js",
    "test",
    "-c",
    process.argv.includes("--soak")
      ? "playwright.soak.config.ts"
      : "playwright.native.config.ts",
    ...process.argv.slice(2).filter((arg) => arg !== "--soak"),
    "--output",
    mkdtempSync(join(tmpdir(), "PodeTelar-native-")),
  ];
  const tests = spawn(process.execPath, args, {
    cwd: root,
    windowsHide: true,
    stdio: "inherit",
    env: {
      ...process.env,
      E2E_BASE_URL: origin,
      FREEDOMSTREAM_TEST_SFU_PID: String(sfu.pid),
      FREEDOMSTREAM_TEST_SFU_EXE: serverExecutable,
      FREEDOMSTREAM_NATIVE_EXE: resolve(
        process.env.FREEDOMSTREAM_NATIVE_EXE ??
          "target/debug/freedomstream-publisher.exe",
      ),
      PATH: `${join(gst, "bin")};${process.env.PATH}`,
    },
  });
  children.push(tests);
  process.exitCode = await new Promise((resolve) => {
    tests.once("error", () => resolve(1));
    tests.once("exit", (code) => resolve(code ?? 1));
  });
} finally {
  for (const child of children.reverse())
    if (child.exitCode === null) child.kill();
}
