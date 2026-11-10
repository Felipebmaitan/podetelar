// Real isolated SFU/API/native-UI workload. Every invitation stays in memory/stdin.
import { spawn } from "node:child_process";
import { randomBytes, randomUUID, createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  writeFileSync,
  existsSync,
  copyFileSync,
} from "node:fs";
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
const disconnected = process.env.FREEDOMSTREAM_BENCH_DISCONNECTED === "1";
const audioEnabled = process.env.FREEDOMSTREAM_BENCH_AUDIO === "1";
const contention = process.env.FREEDOMSTREAM_BENCH_CONTENTION === "1";
const resolution = process.env.FREEDOMSTREAM_BENCH_RESOLUTION ?? "p1080";
const rounds = Number(process.env.FREEDOMSTREAM_BENCH_ROUNDS ?? 1);
const steadySeconds = Number(process.env.FREEDOMSTREAM_BENCH_STEADY ?? 0);
const durationSeconds = phaseSeconds * 6 * rounds + steadySeconds;
if (
  !/^[a-z0-9-]{1,50}$/.test(label) ||
  ![0, 1, 4].includes(count) ||
  ![15, 30, 60].includes(fps) ||
  !["p1080", "p2160"].includes(resolution) ||
  !["h264", "h265", "av1", "av1-software"].includes(codec) ||
  !Number.isInteger(phaseSeconds) ||
  phaseSeconds < 5 ||
  phaseSeconds > 20 ||
  !Number.isInteger(rounds) ||
  rounds < 1 ||
  rounds > 7 ||
  !Number.isInteger(steadySeconds) ||
  steadySeconds < 0 ||
  steadySeconds > 360 ||
  (steadySeconds > 0 && (rounds !== 1 || audioEnabled || contention))
)
  throw new Error("Invalid local workload.");
if (disconnected && (count !== 0 || publishFixture || audioEnabled))
  throw new Error("Disconnected workload cannot publish or watch.");
const root = resolve(import.meta.dirname, "..");
process.chdir(root);
if (existsSync(".local/experiments/stop-after-current-case")) {
  console.log("Stopped at a completed local workload boundary.");
  process.exit(20);
}
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
  const diagnostic = {
    panic: false,
    allocationFailure: false,
    runtimeError: false,
  };
  child.stdout.on("data", (bytes) => {
    output = (output + bytes.toString()).slice(-128000);
  });
  child.stderr.on("data", (bytes) => {
    const text = bytes.toString();
    diagnostic.panic ||= text.includes("panicked at");
    diagnostic.allocationFailure ||=
      /memory allocation.*failed|out of memory/i.test(text);
    diagnostic.runtimeError ||= text.startsWith("Error:");
  });
  child.stdin.on("error", () => {});
  child.on("error", () => {});
  if (input !== undefined) child.stdin.end(JSON.stringify(input));
  else child.stdin.end();
  const closed = new Promise((resolve) => {
    child.once("exit", (code) => resolve(code));
    child.once("error", () => resolve(-1));
  });
  children.push(child);
  return { child, closed, output: () => output, diagnostic };
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
  PATH: `${join(process.env.FREEDOMSTREAM_GSTREAMER ?? join(root, ".local/gstreamer"), "bin")};${process.env.PATH}`,
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
async function executeWorkload() {
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
    let load = null,
      loadSampler = null;
    const loadReport = join(run, "contention.json"),
      loadOsReport = join(run, "contention-process.json");
    if (contention) {
      const request = join(run, "contention-request.json");
      const passes = Array.from(
        { length: 3 },
        () =>
          "d3d11convert ! video/x-raw(memory:D3D11Memory),format=NV12 ! d3d11convert ! video/x-raw(memory:D3D11Memory),format=RGBA",
      ).join(" ! ");
      writeFileSync(
        request,
        JSON.stringify({
          description: `d3d11testsrc is-live=false pattern=ball ! video/x-raw(memory:D3D11Memory),width=3840,height=2160,framerate=60/1 ! ${passes} ! identity name=count ! fakesink sync=false`,
          seconds: 90,
        }),
      );
      load = start(
        resolve("target/release/examples/pipeline-resource.exe"),
        [request, loadReport],
        nativeEnv,
      );
      loadSampler = start("pwsh.exe", [
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-File",
        resolve("scripts/sample-native-process.ps1"),
        "-ProcessId",
        String(load.child.pid),
        "-Output",
        loadOsReport,
        "-Seconds",
        "110",
      ]);
      await delay(1500);
    }
    let audioPath = null,
      audioSource = null;
    if (audioEnabled && count > 0) {
      audioPath = join(run, "SyntheticGameAudio.exe");
      copyFileSync(
        resolve("target/release/examples/pipeline-resource.exe"),
        audioPath,
      );
      const request = join(run, "audio-source.json"),
        report = join(run, "audio-source-result.json");
      const tone = (frequency, pan) =>
        `audiotestsrc is-live=true samplesperbuffer=480 freq=${frequency} volume=0.005 ! audio/x-raw,format=F32LE,rate=48000,channels=1 ! audiopanorama panorama=${pan} ! mix.`;
      writeFileSync(
        request,
        JSON.stringify({
          description: `audiomixer name=mix ! audioconvert ! audio/x-raw,rate=48000,channels=2 ! wasapi2sink low-latency=true ${tone(440, -1)} ${tone(660, 1)}`,
          seconds: 90,
        }),
      );
      audioSource = start(audioPath, [request, report], nativeEnv);
      await delay(1000);
    }
    const publishers = [];
    for (let i = 0; i < count; i++) {
      const publisher = start(exe, ["--publish-fixture"], nativeEnv, {
        invitationUrl,
        displayName: `Synthetic publisher ${i + 1}`,
        codec: codec.replace("-software", ""),
        seconds: durationSeconds + 35,
        fixture4k: resolution === "p2160",
        audio: audioPath
          ? { mode: "application", application: audioPath.toLowerCase() }
          : { mode: "off" },
        quality: {
          resolution,
          fps,
          profile: "motion",
          bitrate: {
            mode: "manual",
            bps: resolution === "p2160" ? 18_000_000 : 6_000_000,
          },
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
    if (process.env.FREEDOMSTREAM_BENCH_CLI === "1") {
      const receiver = resolve(uiPath),
        osReport = join(run, "receiver-process.json");
      const env = { ...nativeEnv };
      if (codec === "av1-software") env.FREEDOMSTREAM_AV1_DECODER = "software";
      const child = start(receiver, ["--watch-fixture"], env, {
        invitationUrl,
        seconds: 30,
        consumePictures: true,
        downloadLimitMbps: 200,
      });
      const sampler = start("pwsh.exe", [
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-File",
        resolve("scripts/sample-native-process.ps1"),
        "-ProcessId",
        String(child.child.pid),
        "-Output",
        osReport,
        "-Seconds",
        "60",
      ]);
      const timer = setTimeout(() => child.child.kill(), 60000);
      const code = await child.closed;
      clearTimeout(timer);
      await sampler.closed;
      if (code !== 0) throw new Error("Native diagnostic receiver failed.");
      const report = JSON.parse(child.output().trim());
      if (!report.closed)
        throw new Error("Native diagnostic receiver did not close.");
      writeFileSync(
        join(run, "diagnostic.json"),
        JSON.stringify(
          {
            schema: 1,
            label,
            count,
            codec,
            resolution,
            appSha256: hash(receiver),
            publisherSha256: hash(exe),
            samples: report.samples.map((s) => ({
              second: s.second,
              watching: s.watching,
              connected: s.connected,
              decoders: s.decoders,
              streams: s.streams.map((t) => ({
                codec: t.codec,
                frames: t.frames,
                watching: t.watching,
                pictureSize: t.pictureSize,
              })),
            })),
            process: JSON.parse(
              readFileSync(osReport, "utf8").replace(/^\uFEFF/, ""),
            ),
            limits:
              "Actual SDK/SFU receiver and CPU conversion, no egui presentation. Only structured decoder, frame and process counters exported.",
          },
          null,
          2,
        ),
      );
      console.log(
        JSON.stringify({
          completed: true,
          diagnostic: true,
          output: join(run, "diagnostic.json"),
        }),
      );
      return;
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
      ...(publishFixture ? { publishFixture: true } : {}),
      ...(disconnected ? { disconnected: true } : {}),
      ...(rounds > 1 ? { rounds } : {}),
      ...(steadySeconds > 0 ? { steadySeconds } : {}),
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
      String(durationSeconds + 60),
      "-RestoreAfterSeconds",
      String(phaseSeconds),
    ]);
    let timedOut = false;
    const timer = setTimeout(
      () => {
        timedOut = true;
        app.child.kill();
      },
      (durationSeconds + 60) * 1000,
    );
    const code = await app.closed;
    clearTimeout(timer);
    await sampler.closed;
    if (load) {
      const loadCode = await load.closed;
      await loadSampler.closed;
      if (
        loadCode !== 0 ||
        !existsSync(loadReport) ||
        JSON.parse(readFileSync(loadReport)).deliveredBuffers < 100
      ) {
        writeFileSync(
          join(run, "failure.json"),
          JSON.stringify(
            {
              schema: 1,
              stage: "synthetic-contention",
              code: loadCode,
              appReportExists: existsSync(appReport),
              helperReportExists: existsSync(loadReport),
            },
            null,
            2,
          ),
        );
        throw new Error("Synthetic contention workload did not run correctly.");
      }
    }
    const audioSourceAliveAtAppExit = audioSource
      ? audioSource.child.exitCode === null
      : null;
    if (audioSource && !audioSourceAliveAtAppExit) {
      writeFileSync(
        join(run, "failure.json"),
        JSON.stringify(
          {
            schema: 1,
            stage: "synthetic-audio",
            code: audioSource.child.exitCode,
            appReportExists: existsSync(appReport),
          },
          null,
          2,
        ),
      );
      throw new Error("Synthetic audio source ended before the workload.");
    }
    if (timedOut || code !== 0 || !existsSync(appReport)) {
      writeFileSync(
        join(run, "failure.json"),
        JSON.stringify(
          {
            schema: 1,
            code,
            timedOut,
            appReportExists: existsSync(appReport),
            diagnostic: app.diagnostic,
            appSha256: hash(uiExe),
            publisherSha256: hash(exe),
          },
          null,
          2,
        ),
      );
      throw new Error("Native UI benchmark failed to finish.");
    }
    const appResult = JSON.parse(readFileSync(appReport));
    if (!appResult.completed) {
      writeFileSync(
        join(run, "failure.json"),
        JSON.stringify(
          {
            schema: 1,
            code,
            timedOut,
            appReportExists: true,
            completed: false,
            fault: appResult.fault,
            appSha256: hash(uiExe),
            publisherSha256: hash(exe),
          },
          null,
          2,
        ),
      );
      throw new Error(
        `Native UI benchmark did not complete: ${appResult.fault ?? "incomplete"}`,
      );
    }
    const observed = appResult.samples.filter(
      (s) => s.phase !== "stopped" && s.activeWatches > 0,
    );
    const expectedCodec =
      `video/${codec.replace("-software", "")}`.toLowerCase();
    const codecStable =
      count === 0
        ? true
        : !observed.length ||
            observed.some((s) => !Array.isArray(s.videoCodecs))
          ? null
          : observed.every((s) =>
              s.videoCodecs.every((c) => c.toLowerCase() === expectedCodec),
            );
    const aggregate = {
      schema: 1,
      label,
      count,
      codec,
      codecStable,
      phaseSeconds,
      fps,
      resolution,
      publishFixture,
      disconnected,
      audioEnabled,
      audioSourceAliveAtAppExit,
      contention,
      rounds,
      steadySeconds,
      appStartedAt,
      appSha256: hash(uiExe),
      publisherSha256: hash(exe),
      fixtureHelperSha256: hash(
        resolve("target/release/examples/pipeline-resource.exe"),
      ),
      experimentSettings: {
        imagePool: appEnv.FREEDOMSTREAM_IMAGE_POOL === "1",
        imagePoolLimit: Number(appEnv.FREEDOMSTREAM_IMAGE_POOL_LIMIT ?? 2),
        temporaryImage: appEnv.FREEDOMSTREAM_IMAGE_FALLBACK === "1",
        frameDrivenRepaint: appEnv.FREEDOMSTREAM_FRAME_REPAINT === "1",
        conversionWorkers: Number(appEnv.FREEDOMSTREAM_CONVERSION_WORKERS ?? 0),
        tokioWorkers: Number(appEnv.TOKIO_WORKER_THREADS ?? 0),
        countSdkLogs: appEnv.FREEDOMSTREAM_COUNT_SDK_LOGS === "1",
      },
      source:
        process.env.FREEDOMSTREAM_PERFORMANCE_SOURCE ??
        "see git source manifest",
      app: appResult,
      process: existsSync(osReport)
        ? JSON.parse(readFileSync(osReport, "utf8").replace(/^\uFEFF/, ""))
        : null,
      contentionProcess:
        contention && existsSync(loadOsReport)
          ? JSON.parse(
              readFileSync(loadOsReport, "utf8").replace(/^\uFEFF/, ""),
            )
          : null,
      contentionPipeline:
        contention && existsSync(loadReport)
          ? JSON.parse(readFileSync(loadReport, "utf8").replace(/^\uFEFF/, ""))
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
}
await executeWorkload();
