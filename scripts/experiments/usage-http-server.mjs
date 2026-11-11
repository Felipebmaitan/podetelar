// MIT. Actual authenticated API, isolated synthetic DB, credentials only over IPC.
import { once } from "node:events";
import { monitorEventLoopDelay, performance } from "node:perf_hooks";
import { resolve } from "node:path";
import { buildApp } from "../../apps/server/dist/app.js";
import { loadConfig } from "../../apps/server/dist/config.js";
import { Usage } from "../../apps/server/dist/usage.js";
import { reportBatched } from "./usage-report.mjs";
import { reportCached, cacheStats } from "./usage-report-cache.mjs";
import {
  reportCachedJson,
  jsonCacheStats,
} from "./usage-report-json-cache.mjs";
const [request] = await once(process, "message");
if (
  !["baseline", "batched", "cached", "cached-json", "production-json"].includes(
    request.variant,
  ) ||
  !request.synthetic
)
  throw Error("Invalid experiment request");
let clock = request.now,
  instance,
  sequence = 0;
const originalReport = Usage.prototype.report;
const originalJson = Usage.prototype.reportJson;
function response(usage, args) {
  instance = usage;
  if (request.simulateWrites) {
    clock += 5000;
    sequence++;
    usage.record(
      [
        {
          room: "room-0",
          participant: "synthetic-00",
          direction: "incoming",
          bytes: sequence * 5000,
        },
      ],
      clock,
      1,
    );
  }
  return request.variant === "production-json"
    ? originalJson.apply(usage, args)
    : request.variant === "cached-json"
      ? reportCachedJson(usage, ...args)
      : request.variant === "cached"
        ? reportCached(usage, ...args)
        : request.variant === "batched"
          ? reportBatched(usage, ...args)
          : originalReport.apply(usage, args);
}
// Older frozen server builds expose report(); the final route exposes reportJson().
// Baseline always runs the unchanged full query/report implementation.
if (originalJson)
  Usage.prototype.reportJson = function (...args) {
    const value = response(this, args);
    return typeof value === "string" ? value : JSON.stringify(value);
  };
else
  Usage.prototype.report = function (...args) {
    return response(this, args);
  };
const config = loadConfig({
  PUBLIC_URL: "http://127.0.0.1",
  ADMIN_TOKEN: request.password,
  LIVEKIT_API_KEY: "synthetic-local-only",
  LIVEKIT_API_SECRET: request.mediaSecret,
  DATABASE_PATH: request.file,
});
config.webRoot = resolve(request.out, "no-web");
config.publisherDirectory = undefined;
const app = await buildApp(config, {
  logger: false,
  usagePolling: false,
  now: () => clock,
});
if (request.variant === "cached-json")
  app.addHook("onRequest", async (request, reply) => {
    if (request.url.split("?")[0] === "/api/admin/usage")
      reply.type("application/json; charset=utf-8");
  });
const address = await app.listen({ host: "127.0.0.1", port: 0 });
config.publicUrl = address;
let start,
  cpu,
  memoryTimer,
  memorySamples = [];
const delay = monitorEventLoopDelay({ resolution: 10 });
delay.enable();
process.on("message", async (message) => {
  if (message.type === "start") {
    delay.reset();
    cpu = process.cpuUsage();
    start = performance.now();
    memorySamples = [];
    memoryTimer = setInterval(
      () =>
        memorySamples.push({
          second: (performance.now() - start) / 1000,
          ...process.memoryUsage(),
        }),
      5000,
    );
    memoryTimer.unref();
    process.send({ type: "started" });
  }
  if (message.type === "finish") {
    clearInterval(memoryTimer);
    const used = process.cpuUsage(cpu);
    process.send({
      type: "finished",
      cpuMs: (used.user + used.system) / 1000,
      wallMs: performance.now() - start,
      eventLoopP50Ms: delay.percentile(50) / 1e6,
      eventLoopP95Ms: delay.percentile(95) / 1e6,
      eventLoopP99Ms: delay.percentile(99) / 1e6,
      eventLoopMaxMs: delay.max / 1e6,
      memory: process.memoryUsage(),
      memorySamples,
      cache: instance
        ? request.variant === "cached-json"
          ? jsonCacheStats(instance)
          : cacheStats(instance)
        : null,
    });
  }
  if (message.type === "close") {
    clearInterval(memoryTimer);
    delay.disable();
    await app.close();
    process.exit(0);
  }
});
process.send({ type: "ready", address });
