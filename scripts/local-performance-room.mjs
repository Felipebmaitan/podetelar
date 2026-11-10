// Real isolated SFU/API/native-UI workload. Every invitation stays in memory/stdin.
import { spawn } from "node:child_process";
import { randomBytes, randomUUID, createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { networkInterfaces } from "node:os";
import { setTimeout as delay } from "node:timers/promises";

const [
  label = "baseline",
  countText = "1",
  codec = "h265",
  uiPath = "target/release/examples/room-benchmark.exe",
  phaseText = "5",
] = process.argv.slice(2);
const count = Number(countText),
  phaseSeconds = Number(phaseText);
const fps = Number(process.env.FREEDOMSTREAM_BENCH_FPS ?? 60);
const publishFixture = process.env.FREEDOMSTREAM_BENCH_PUBLISH === "1";
if (
  !/^[a-z0-9-]{1,50}$/.test(label) ||
  ![0, 1, 4].includes(count) ||
  ![15, 30, 60].includes(fps) ||
  !["h264", "h265", "av1", "av1-software"].includes(codec) ||
  !Number.isInteger(phaseSeconds) ||
  phaseSeconds < 5 ||
  phaseSeconds > 10
)
  throw new Error("Invalid local workload.");
const root = resolve(import.meta.dirname, "..");
process.chdir(root);
const run = join(
  root,
  ".local/experiments",
  `${label}-${count}-${codec}-${randomUUID()}`,
);
mkdirSync(run, { recursive: true });
const origin = "http://localhost:3020";
for (const url of [origin + "/api/health", "http://127.0.0.1:18880"]) {
  try {
    await fetch(url, { signal: AbortSignal.timeout(500) });
    throw new Error("Occupied benchmark port.");
  } catch (error) {
    if (error.message === "Occupied benchmark port.") throw error;
  }
}
const nodeIp = Object.values(networkInterfaces())
  .flat()
  .find(
    (ip) =>
      ip &&
      ip.family === "IPv4" &&
      !ip.internal &&
      /^(192\.168\.|10\.|172\.(1[6-9]|2[0-9]|3[01])\.)/.test(ip.address),
  )?.address;
if (!nodeIp) throw new Error("No local RTC address available.");
const key = randomBytes(16).toString("hex"),
  secret = randomBytes(32).toString("hex"),
  admin = randomBytes(32).toString("hex");
const config = join(run, "livekit.json");
writeFileSync(
  config,
  JSON.stringify({
    port: 18880,
    bind_addresses: ["127.0.0.1"],
    rtc: {
      tcp_port: 18881,
      udp_port: 18882,
      node_ip: nodeIp,
      use_external_ip: false,
    },
    keys: { [key]: secret },
    logging: { level: "error" },
  }),
  { flag: "wx" },
);
const children = [];
function start(file, args, env = process.env, input) {
  const child = spawn(file, args, {
    cwd: root,
    env,
    windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"],
  });
  let output = "";
  child.stdout.on("data", (bytes) => {
    output = (output + bytes.toString()).slice(-128000);
  });
  child.stderr.on("data", () => {});
  child.stdin.on("error", () => {});
  child.on("error", () => {});
  if (input !== undefined) child.stdin.end(JSON.stringify(input));
  else child.stdin.end();
  const closed = new Promise((resolve) => {
    child.once("exit", (code) => resolve(code));
    child.once("error", () => resolve(-1));
  });
  children.push(child);
  return { child, closed, output: () => output };
}
async function ready(url) {
  for (let i = 0; i < 80; i++) {
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(500) })).ok) return;
    } catch {}
    await delay(125);
  }
  throw new Error("Isolated service startup failed.");
}
const nativeEnv = {
  ...process.env,
  PATH: `${join(root, ".local/gstreamer/bin")};${process.env.PATH}`,
};
const exe = resolve(
  process.env.FREEDOMSTREAM_PERFORMANCE_PUBLISHER ??
    ".local/experiments/baseline/freedomstream-publisher.exe",
);
const pin = JSON.parse(readFileSync("deploy/test-livekit.json"));
const sfu = join(root, `.local/livekit-test-${pin.version}/livekit-server.exe`);
if (
  createHash("sha256").update(readFileSync(sfu)).digest("hex") !==
  pin.executableSha256
)
  throw new Error("Unqualified local SFU.");
const hash = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
try {
  start(sfu, ["--config", config]);
  await ready("http://127.0.0.1:18880");
  start(process.execPath, ["apps/server/dist/main.js"], {
    ...process.env,
    PUBLIC_URL: origin,
    HOST: "127.0.0.1",
    PORT: "3020",
    DATABASE_PATH: join(run, "rooms.db"),
    LIVEKIT_INTERNAL_URL: "http://127.0.0.1:18880",
    ADMIN_TOKEN: admin,
    LIVEKIT_API_KEY: key,
    LIVEKIT_API_SECRET: secret,
  });
  await ready(origin + "/api/health");
  const response = await fetch(origin + "/api/rooms", {
    method: "POST",
    headers: {
      Origin: origin,
      Authorization: `Bearer ${admin}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      name: "Synthetic local performance room",
      displayName: "Fixture administrator",
    }),
  });
  if (response.status !== 201) throw new Error("Local room creation failed.");
  const { invitationUrl } = await response.json();
  const publishers = [];
  for (let i = 0; i < count; i++) {
    const publisher = start(exe, ["--publish-fixture"], nativeEnv, {
      invitationUrl,
      displayName: `Synthetic publisher ${i + 1}`,
      codec: codec.replace("-software", ""),
      seconds: phaseSeconds * 6 + 35,
      quality: {
        resolution: "p1080",
        fps,
        profile: "motion",
        bitrate: { mode: "manual", bps: 6_000_000 },
      },
    });
    publishers.push(publisher);
    for (let n = 0; n < 100; n++) {
      if (publisher.output().includes('"published"')) break;
      if (publisher.child.exitCode !== null)
        throw new Error("Publisher startup failed.");
      await delay(100);
    }
    if (!publisher.output().includes('"published"'))
      throw new Error("Publisher readiness timed out.");
  }
  const uiExe = resolve(uiPath),
    appReport = join(run, "app.json"),
    osReport = join(run, "process.json");
  const appEnv = { ...nativeEnv, LOCALAPPDATA: join(run, "LocalAppData") };
  if (publishFixture) appEnv.FREEDOMSTREAM_PERFORMANCE_FIXTURE = "1";
  mkdirSync(appEnv.LOCALAPPDATA);
  if (codec === "av1-software") appEnv.FREEDOMSTREAM_AV1_DECODER = "software";
  if (process.env.FREEDOMSTREAM_BENCH_TOKIO)
    appEnv.TOKIO_WORKER_THREADS = process.env.FREEDOMSTREAM_BENCH_TOKIO;
  const appStartedAt = Date.now();
  const app = start(uiExe, [], appEnv, {
    invitationUrl,
    expectedStreams: count,
    report: appReport,
    phaseSeconds,
    emptyRoom: count === 0,
    publishFixture,
  });
  const sampler = start("pwsh.exe", [
    "-NoProfile",
    "-NonInteractive",
    "-WindowStyle",
    "Hidden",
    "-File",
    resolve("scripts/sample-native-process.ps1"),
    "-ProcessId",
    String(app.child.pid),
    "-Output",
    osReport,
    "-Seconds",
    "150",
    "-RestoreAfterSeconds",
    String(phaseSeconds),
  ]);
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    app.child.kill();
  }, 150000);
  const code = await app.closed;
  clearTimeout(timer);
  await sampler.closed;
  if (timedOut || code !== 0 || !existsSync(appReport))
    throw new Error("Native UI benchmark failed to finish.");
  const appResult = JSON.parse(readFileSync(appReport));
  if (!appResult.completed)
    throw new Error(
      `Native UI benchmark did not complete: ${appResult.fault ?? "incomplete"}`,
    );
  const aggregate = {
    schema: 1,
    label,
    count,
    codec,
    phaseSeconds,
    appStartedAt,
    appSha256: hash(uiExe),
    publisherSha256: hash(exe),
    source:
      process.env.FREEDOMSTREAM_PERFORMANCE_SOURCE ?? "see git source manifest",
    app: appResult,
    process: existsSync(osReport)
      ? JSON.parse(readFileSync(osReport, "utf8").replace(/^\uFEFF/, ""))
      : null,
    limits:
      "Actual native UI/SDK/SFU decode, no microphone or personal screen publication. Publishers are separate synthetic processes. GPU engine/VRAM counters are per receiver; these are not game frame times.",
  };
  writeFileSync(join(run, "result.json"), JSON.stringify(aggregate, null, 2));
  console.log(
    JSON.stringify({
      completed: true,
      label,
      count,
      codec,
      result: join(run, "result.json"),
    }),
  );
} finally {
  for (const child of children.reverse())
    if (child.exitCode === null) child.kill();
}
