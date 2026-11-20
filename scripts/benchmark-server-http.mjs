// Actual local HTTP routes, fresh authority on every chunk. No SFU RPC is needed
// for these admission transfers. Credentials remain inside each child process.
import { spawn } from "node:child_process";
import { randomBytes, randomUUID, createHash } from "node:crypto";
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { monitorEventLoopDelay, performance } from "node:perf_hooks";
import { setTimeout as delay } from "node:timers/promises";

const root = resolve(import.meta.dirname, "..");
if (process.argv[2] === "--case") {
  const directory = process.argv[3];
  const { buildApp } = await import(pathToFileURL(join(directory, "app.js")));
  const { loadConfig } = await import(
    pathToFileURL(join(directory, "config.js"))
  );
  const origin = "http://localhost:5173";
  const admin = randomBytes(32).toString("hex");
  const app = await buildApp(
    loadConfig({
      PUBLIC_URL: origin,
      ADMIN_TOKEN: admin,
      LIVEKIT_API_KEY: "local-benchmark",
      LIVEKIT_API_SECRET: randomBytes(32).toString("hex"),
      DATABASE_PATH: join(directory, `room-${randomUUID()}.db`),
    }),
    { usagePolling: false },
  );
  try {
    const address = await app.listen({ host: "127.0.0.1", port: 0 });
    const create = await fetch(address + "/api/rooms", {
      method: "POST",
      headers: {
        origin,
        authorization: `Bearer ${admin}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        name: "Synthetic HTTP performance",
        displayName: "Owner",
      }),
    });
    if (create.status !== 201) throw new Error("Synthetic HTTP room failed.");
    const created = await create.json();
    const cookie = create.headers.get("set-cookie").split(";")[0];
    const invitation = new URL(created.invitationUrl).hash.slice(8);
    const path = `/api/rooms/${created.room.id}/network-test`;
    const cookies = [cookie];
    for (let n = 0; n < 7; ++n) {
      const joined = await fetch(
        `${address}/api/rooms/${created.room.id}/join`,
        {
          method: "POST",
          headers: { origin, "content-type": "application/json" },
          body: JSON.stringify({
            invitation,
            displayName: `Synthetic join ${n}`,
          }),
        },
      );
      if (joined.status !== 200) throw new Error("Synthetic HTTP join failed.");
      cookies.push(joined.headers.get("set-cookie").split(";")[0]);
      await joined.arrayBuffer();
    }
    const histogram = monitorEventLoopDelay({ resolution: 10 });
    histogram.enable();
    const start = performance.now(),
      cpu = process.cpuUsage();
    const issued = await Promise.all(
      cookies.map((cookie) =>
        fetch(address + path, {
          method: "POST",
          headers: { origin, cookie, "x-network-test-max-bps": "200000000" },
        }),
      ),
    );
    if (
      issued.filter((r) => r.status === 200).length !== 1 ||
      issued.filter((r) => r.status === 409).length !== 7
    )
      throw new Error("Shared HTTP admission lease regressed.");
    const owner = issued.findIndex((r) => r.status === 200);
    const lease = await issued[owner].json();
    for (const r of issued) if (r.status !== 200) await r.arrayBuffer();
    const downloadStarted = performance.now();
    const down = await fetch(address + path + "/download", {
      headers: { cookie: cookies[owner], "x-network-probe": lease.probe },
    });
    if (down.status !== 200) throw new Error("Synthetic HTTP download failed.");
    let downloaded = 0;
    for await (const block of down.body) downloaded += block.length;
    const downloadMs = performance.now() - downloadStarted;
    if (
      downloaded < 65536 ||
      downloaded > lease.byteLimit ||
      downloadMs < (downloaded * 8000) / lease.payloadCapBps - 100
    )
      throw new Error("HTTP transfer bounds or pacing regressed.");
    const uploadBytes = 16 * 1024 * 1024,
      block = Buffer.alloc(65536, 0x5a);
    const uploadStarted = performance.now();
    async function* body() {
      for (let sent = 0; sent < uploadBytes; sent += block.length) {
        await delay(
          Math.max(
            0,
            (sent * 8000) / lease.payloadCapBps -
              (performance.now() - uploadStarted),
          ),
        );
        yield block;
      }
    }
    const upload = await fetch(address + path + "/upload", {
      method: "POST",
      duplex: "half",
      headers: {
        origin,
        cookie: cookies[owner],
        "x-network-probe": lease.probe,
        "content-type": "application/octet-stream",
      },
      body: body(),
    });
    if (upload.status !== 200 || (await upload.json()).bytes !== uploadBytes)
      throw new Error("Synthetic HTTP upload failed.");
    const used = process.cpuUsage(cpu);
    histogram.disable();
    process.send({
      schema: 1,
      completed: true,
      downloadedBytes: downloaded,
      uploadedBytes: uploadBytes,
      downloadMs,
      uploadMs: performance.now() - uploadStarted,
      wallMs: performance.now() - start,
      combinedServerClientCpuMs: (used.user + used.system) / 1000,
      eventLoopP95Ms: histogram.percentile(95) / 1e6,
      eventLoopP99Ms: histogram.percentile(99) / 1e6,
      oneSharedLease: true,
      busyClients: 7,
    });
  } finally {
    await app.close();
  }
} else {
  const run = join(root, ".local/experiments", `server-http-${randomUUID()}`);
  mkdirSync(run);
  const rows = [];
  for (const mode of ["baseline", "prepared"]) {
    const directory = join(run, mode);
    cpSync(join(root, "apps/server/dist"), directory, { recursive: true });
    writeFileSync(
      join(directory, "rooms.js"),
      readFileSync(
        join(
          root,
          `.local/experiments/server/${mode === "baseline" ? "baseline" : "prepared"}-rooms.mjs`,
        ),
      ),
    );
  }
  for (let pair = 1; pair <= 5; ++pair)
    for (const mode of pair % 2
      ? ["baseline", "prepared"]
      : ["prepared", "baseline"]) {
      const child = spawn(
        process.execPath,
        [import.meta.filename, "--case", join(run, mode)],
        { windowsHide: true, stdio: ["ignore", "ignore", "ignore", "ipc"] },
      );
      let report;
      child.on("message", (value) => {
        report = value;
      });
      const timer = setTimeout(() => child.kill(), 20000);
      const code = await new Promise((done, reject) => {
        child.once("error", reject);
        child.once("close", done);
      });
      clearTimeout(timer);
      if (code !== 0 || !report?.completed)
        throw new Error("Local HTTP comparison failed.");
      rows.push({ pair, mode, ...report });
      console.log(
        `Actual HTTP pair ${pair} ${mode}: one lease, seven busy clients, bounded paced transfers.`,
      );
    }
  const hash = (file) =>
    createHash("sha256").update(readFileSync(file)).digest("hex");
  writeFileSync(
    join(run, "summary.json"),
    JSON.stringify(
      {
        schema: 1,
        runs: rows,
        baselineRoomsSha256: hash(join(run, "baseline/rooms.js")),
        preparedRoomsSha256: hash(join(run, "prepared/rooms.js")),
        limits:
          "Actual local API HTTP admission routes and SQLite, eight joining clients, same global lease and fresh per-chunk authority. Process CPU includes server and synthetic fetch client; not isolated server CPU, production SFU capacity, or native-app savings. No tokens or URLs exported.",
      },
      null,
      2,
    ),
  );
  console.log(
    JSON.stringify({ output: join(run, "summary.json"), runs: rows.length }),
  );
}
