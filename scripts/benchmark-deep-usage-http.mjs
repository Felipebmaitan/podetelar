import { fork } from "node:child_process";
import { randomBytes, createHash } from "node:crypto";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { setTimeout as delay } from "node:timers/promises";
import assert from "node:assert/strict";
const root = resolve(import.meta.dirname, ".."),
  out = resolve(root, ".local/experiments/deep-pass/usage");
const fixture = JSON.parse(readFileSync(resolve(out, "queries.json"))).fixture;
const treatment = process.argv[2] ?? "batched";
const steady = process.argv[3] === "steady";
assert.ok(
  ["batched", "cached", "cached-json", "production-json"].includes(treatment),
);
const output = resolve(
  out,
  steady
    ? `${treatment}-steady-http.json`
    : treatment === "batched"
      ? "http.json"
      : `${treatment}-http.json`,
);
assert.ok(
  !existsSync(output),
  "Existing HTTP measurements are immutable; choose a new treatment label.",
);
const runs = [];
let reference;
const median = (a) => [...a].sort((a, b) => a - b)[Math.floor(a.length / 2)];
const percentile = (a, p) =>
  [...a].sort((a, b) => a - b)[Math.floor((a.length - 1) * p)];
for (let pair = 1; pair <= (steady ? 1 : 5); pair++)
  for (const variant of pair % 2
    ? ["baseline", treatment]
    : [treatment, "baseline"]) {
    const password = randomBytes(32).toString("hex"),
      mediaSecret = randomBytes(32).toString("hex");
    const child = fork(
      resolve(root, "scripts/experiments/usage-http-server.mjs"),
      [],
      {
        cwd: root,
        windowsHide: true,
        stdio: ["ignore", "ignore", "ignore", "ipc"],
      },
    );
    child.on("error", () => {});
    const wait = (type) =>
      new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          child.off("message", handler);
          reject(Error("Owned API IPC timed out"));
        }, 30000);
        const handler = (m) => {
          if (m.type === type) {
            clearTimeout(timer);
            child.off("message", handler);
            resolve(m);
          }
        };
        child.on("message", handler);
      });
    try {
      const simulateWrites =
        treatment.startsWith("cached") || treatment === "production-json";
      const ready = wait("ready");
      child.send({
        ...fixture,
        password,
        mediaSecret,
        variant,
        out,
        simulateWrites,
      });
      const { address } = await ready;
      const endpoint =
        address +
        `/api/admin/usage?from=${fixture.now - 366 * 86400000}&to=${fixture.now - (simulateWrites ? 86400000 : 0)}`;
      assert.equal((await fetch(endpoint)).status, 401);
      const login = await fetch(address + "/api/admin/login", {
        method: "POST",
        headers: { Origin: address, "Content-Type": "application/json" },
        body: JSON.stringify({ password }),
      });
      assert.equal(login.status, 200);
      const cookie = login.headers.get("set-cookie").split(";")[0];
      let requests = 0;
      const get = async () => {
        const r = await fetch(endpoint, { headers: { cookie } });
        assert.equal(r.status, 200);
        assert.match(r.headers.get("content-type"), /^application\/json/);
        const body = await r.json();
        requests++;
        if (simulateWrites) {
          assert.equal(body.lastSampleAt, fixture.now + requests * 5000);
          assert.equal(body.available, true);
        }
        return body;
      };
      const same = (report) => {
        const expected = simulateWrites
          ? {
              ...reference,
              lastSampleAt: fixture.now + requests * 5000,
              available: true,
            }
          : reference;
        assert.deepEqual(report, expected);
      };
      const warm = await get();
      reference ??= warm;
      same(warm);
      same(await get());
      const started = wait("started");
      child.send({ type: "start" });
      await started;
      const reportLatencies = [],
        healthLatencies = [];
      const workloadStarted = performance.now();
      await Promise.all([
        (async () => {
          for (let i = 0; i < (steady ? 30 : 10); i++) {
            if (steady)
              await delay(
                Math.max(0, workloadStarted + i * 2000 - performance.now()),
              );
            const start = performance.now();
            const report = await get();
            reportLatencies.push(performance.now() - start);
            same(report);
          }
        })(),
        (async () => {
          for (let i = 0; i < (steady ? 30 : 40); i++) {
            if (steady)
              await delay(
                Math.max(0, workloadStarted + i * 2000 - performance.now()),
              );
            const start = performance.now();
            const response = await fetch(address + "/api/health");
            assert.equal(response.status, 200);
            await response.arrayBuffer();
            healthLatencies.push(performance.now() - start);
            if (!steady) await delay(20);
          }
        })(),
      ]);
      if (steady)
        await delay(Math.max(0, workloadStarted + 60000 - performance.now()));
      const finished = wait("finished");
      child.send({ type: "finish" });
      const metrics = await finished;
      const logout = await fetch(address + "/api/admin/logout", {
        method: "POST",
        headers: { Origin: address, cookie },
      });
      assert.equal(logout.status, 204);
      assert.equal(
        (await fetch(endpoint, { headers: { cookie } })).status,
        401,
      );
      runs.push({
        pair,
        variant,
        ...metrics,
        reports: steady ? 30 : 10,
        healthRequests: steady ? 30 : 40,
        reportLatencyP50Ms: median(reportLatencies),
        reportLatencyP95Ms: percentile(reportLatencies, 0.95),
        healthLatencyP50Ms: median(healthLatencies),
        healthLatencyP95Ms: percentile(healthLatencies, 0.95),
        healthLatencyP99Ms: percentile(healthLatencies, 0.99),
        fullResponseEquivalent: true,
        authenticationAndLogoutPassed: true,
        outputSha256: createHash("sha256")
          .update(JSON.stringify(reference))
          .digest("hex"),
      });
      writeFileSync(
        output,
        JSON.stringify(
          {
            schema: 1,
            treatment,
            steady,
            simulatedCurrentWritePerReport: simulateWrites,
            closedHistoricalRange: simulateWrites,
            fixture: {
              rows: fixture.rows,
              users: fixture.users,
              days: fixture.days,
              synthetic: true,
            },
            runs,
            limits: steady
              ? "One matched 60-second pair, fixed 30 reports and 30 health requests paced every two seconds; memory sampled every five seconds. Actual authenticated API and ongoing outside-range writes; no forced GC or working-set trim. Local diagnostic, not replicated VPS qualification."
              : "Actual local Fastify administrator login/usage/logout and health routes. Separate client/server processes. Fixed 10 reports and 40 successful health requests per trial. Cached experiment refreshes unchanged closed history while current records change and freshness fields remain live; not all report requests or VPS/WAN capacity.",
          },
          null,
          2,
        ),
      );
      console.log(
        JSON.stringify({
          pair,
          variant,
          cpuMs: metrics.cpuMs,
          reportP50Ms: runs.at(-1).reportLatencyP50Ms,
          healthP95Ms: runs.at(-1).healthLatencyP95Ms,
        }),
      );
    } finally {
      if (child.connected) child.send({ type: "close" }, () => {});
      if (child.exitCode === null)
        await Promise.race([
          new Promise((r) => child.once("exit", r)),
          delay(5000),
        ]);
      if (child.exitCode === null) child.kill();
    }
  }
