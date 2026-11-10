import { spawn } from "node:child_process";
import { randomBytes, randomUUID, createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { performance } from "node:perf_hooks";
import { setTimeout as delay } from "node:timers/promises";
import { RoomServiceClient } from "livekit-server-sdk";
import { LivekitDirectory } from "../apps/server/dist/media.js";
import { CoalescedDirectory } from "./experiments/coalesced-directory.mjs";
const root = resolve(import.meta.dirname, "..");
const run = join(root, ".local/experiments/server", `media-${randomUUID()}`);
mkdirSync(run, { recursive: true });
try {
  await fetch("http://127.0.0.1:18880", { signal: AbortSignal.timeout(500) });
  throw new Error("Occupied local SFU port.");
} catch (error) {
  if (error.message === "Occupied local SFU port.") throw error;
}
const pin = JSON.parse(readFileSync(join(root, "deploy/test-livekit.json")));
const exe = join(root, `.local/livekit-test-${pin.version}/livekit-server.exe`);
if (
  createHash("sha256").update(readFileSync(exe)).digest("hex") !==
  pin.executableSha256
)
  throw new Error("Unqualified local SFU.");
const key = randomBytes(16).toString("hex"),
  secret = randomBytes(32).toString("hex");
const config = join(run, "sfu.json");
writeFileSync(
  config,
  JSON.stringify({
    port: 18880,
    bind_addresses: ["127.0.0.1"],
    rtc: { tcp_port: 18881, udp_port: 18882, use_external_ip: false },
    keys: { [key]: secret },
    logging: { level: "error" },
  }),
  { flag: "wx" },
);
const sfu = spawn(exe, ["--config", config], {
  windowsHide: true,
  stdio: ["ignore", "ignore", "ignore"],
});
const rows = [];
const endpoint = "http://127.0.0.1:18880";
try {
  let ready = false;
  for (let n = 0; n < 80; n++) {
    try {
      if ((await fetch(endpoint, { signal: AbortSignal.timeout(500) })).ok) {
        ready = true;
        break;
      }
    } catch {}
    await delay(125);
  }
  if (!ready) throw new Error("Local SFU startup failed.");
  const admin = new RoomServiceClient(endpoint, key, secret);
  await admin.createRoom({ name: "freedomstream_benchmark" });
  for (let pair = 1; pair <= 5; pair++)
    for (const mode of pair % 2
      ? ["direct", "coalesced"]
      : ["coalesced", "direct"]) {
      const direct = new LivekitDirectory({
        livekitInternalUrl: endpoint,
        livekitApiKey: key,
        livekitApiSecret: secret,
      });
      const rpc = direct.client.getParticipant.bind(direct.client);
      let calls = 0;
      direct.client.getParticipant = (...args) => {
        calls++;
        return rpc(...args);
      };
      const directory =
        mode === "direct" ? direct : new CoalescedDirectory(direct);
      const start = performance.now(),
        cpu = process.cpuUsage();
      const results = await Promise.all(
        Array.from({ length: 16 }, () =>
          directory.participant("benchmark", "synthetic-absent-participant"),
        ),
      );
      const elapsed = performance.now() - start,
        used = process.cpuUsage(cpu);
      if (results.some((result) => result !== null))
        throw new Error("Absent participant unexpectedly admitted.");
      rows.push({
        pair,
        mode,
        concurrentRequests: 16,
        actualSfuRpcCalls: calls,
        wallMs: elapsed,
        cpuMs: (used.user + used.system) / 1000,
      });
    }
  const race = [];
  for (const mode of ["direct", "coalesced"]) {
    let active = true,
      calls = 0,
      release;
    const inner = {
      participant() {
        const snapshot = {
          tracks: active
            ? [{ sid: "TR_synthetic", kind: "screen", muted: false }]
            : [],
        };
        calls++;
        return calls === 1
          ? new Promise((resolve) => {
              release = () => resolve(snapshot);
            })
          : Promise.resolve(snapshot);
      },
    };
    const directory = mode === "direct" ? inner : new CoalescedDirectory(inner);
    const first = directory.participant("synthetic-room", "synthetic-owner");
    active = false;
    const second = directory.participant("synthetic-room", "synthetic-owner");
    release();
    const [a, b] = await Promise.all([first, second]);
    race.push({
      mode,
      calls,
      firstTrackPresent: a.tracks.length > 0,
      checkStartedAfterUnpublishReturnedTrack: b.tracks.length > 0,
    });
  }
  if (
    race.find((r) => r.mode === "direct")
      .checkStartedAfterUnpublishReturnedTrack ||
    !race.find((r) => r.mode === "coalesced")
      .checkStartedAfterUnpublishReturnedTrack
  )
    throw new Error("Freshness regression was not reproduced as expected.");
  const output = join(run, "result.json");
  writeFileSync(
    output,
    JSON.stringify(
      {
        schema: 1,
        runs: rows,
        freshnessRace: race,
        decision: "reject/risk",
        limits:
          "RPC performance uses the real pinned local SFU and actual RoomServiceClient/LivekitDirectory for an absent participant. A separately controlled deferred-response race demonstrates why sharing an older pending query changes freshness after unpublish. No application authority was cached, production source changed, or private media used.",
      },
      null,
      2,
    ),
  );
  console.log(JSON.stringify({ output, decision: "reject/risk" }));
} finally {
  if (sfu.exitCode === null) sfu.kill();
}
