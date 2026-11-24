import { expect, type APIRequestContext } from "@playwright/test";
import { defaultStreamPolicy, type StreamPolicy } from "@podetelar/contracts";
import { spawn, type ChildProcess } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve, join } from "node:path";

export function nativeExecutable(): string {
  const executable = process.env.PODETELAR_NATIVE_EXE;
  if (!executable)
    throw new Error("Set PODETELAR_NATIVE_EXE to the built publisher.");
  return resolve(executable);
}

/** Isolate the tone fixture without muting or changing other applications. */
export async function backgroundAudioExclusions(): Promise<string[]> {
  const applications = await nativeRequest("--audio-apps", {}).result;
  if (!Array.isArray(applications))
    throw new Error("Audio application inventory was unavailable.");
  return applications.flatMap((application: unknown) => {
    if (
      typeof application !== "object" ||
      application === null ||
      !("id" in application) ||
      typeof application.id !== "string"
    )
      return [];
    // The game is our child; excluding this harness image would exclude it too.
    return resolve(application.id).toLowerCase() ===
      process.execPath.toLowerCase()
      ? []
      : [application.id];
  });
}

export function toneFixtures(directory: string) {
  mkdirSync(directory, { recursive: true });
  const game = join(directory, "Game.exe");
  const discord = join(directory, "Discord.exe");
  // A portable launcher keeps its own name separate from the publisher process.
  // Tone fixtures need their real process names for application-audio policy.
  const tone = process.env.PODETELAR_NATIVE_TONE_EXE ?? nativeExecutable();
  copyFileSync(tone, game);
  copyFileSync(tone, discord);
  const children: ChildProcess[] = [];
  const start = (executable: string, frequency: number, rate = 48_000) => {
    const child = spawn(
      executable,
      ["--audio-tone", String(frequency), String(rate)],
      {
        windowsHide: true,
        stdio: "ignore",
      },
    );
    children.push(child);
    return child;
  };
  return {
    application: game.toLowerCase(),
    game: (frequency = 440, rate = 48_000) => start(game, frequency, rate),
    discord: () => start(discord, 880),
    stop: () => {
      for (const child of children) if (child.exitCode === null) child.kill();
    },
  };
}

export function nativeRequest(
  option: string,
  request: unknown,
  args: string[] = [],
) {
  const child = spawn(nativeExecutable(), [option, ...args], {
    windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"],
  });
  let output = "";
  let stage = "unknown";
  const failureTags = new Set<string>();
  let reportReady!: (value: unknown) => void;
  let reportFailed!: (error: Error) => void;
  const reported = new Promise<unknown>((resolve, reject) => {
    reportReady = resolve;
    reportFailed = reject;
  });
  child.stdout.on("data", (data: Buffer) => {
    // Two-stream timing/gain qualification exceeds 64 KB. Retain one bounded
    // numeric report without truncating its JSON during the real-media check.
    output = (output + data.toString()).slice(-512_000);
    const line = output.trim().split("\n").at(-1);
    if (line?.startsWith("{")) {
      try {
        reportReady(JSON.parse(line));
      } catch {
        /* A later chunk may finish the diagnostic line. */
      }
    }
  });
  // Never include raw native/SDK stderr in test reports.
  child.stderr.on("data", (data: Buffer) => {
    const message = data.toString();
    for (const [hint, label] of [
      ["panicked at", "rust-panic"],
      ["cannot unwind", "unwind-abort"],
      ["Check failed", "native-check"],
      ["Assertion failed", "native-assertion"],
      ["terminate", "native-terminate"],
      ["deadlock", "deadlock"],
    ]) {
      if (message.includes(hint!)) failureTags.add(label!);
    }
    for (const [hint, label] of [
      ["Audio transport stalled", "audio-transport"],
      ["Audio publication failed", "audio-publication"],
      ["Fixture media connection", "media-connection"],
      ["Could not connect to the media server", "media-connection"],
      ["Media connection timed out", "media-timeout"],
      [
        "The media server is synchronizing stream limits",
        "policy-synchronization",
      ],
      ["Dual-sharing operation did not finish", "dual-sharing-timeout"],
      ["Dual-sharing qualification requires", "dual-sharing-locality"],
      ["No codec can be hardware encoded", "codec-negotiation"],
      ["This camera does not offer the selected", "camera-mode"],
      ["Sharing stopped because the media server ended", "sfu-ended-video"],
      ["Sharing stopped because the stream could not meet", "policy-update"],
      ["No H.264 hardware encoder", "hardware-encoder"],
      ["Application audio capture failed", "audio-capture"],
      ["Room access was denied", "admission"],
      ["Too many room requests", "rate-limit"],
      ["capture source or GPU encoder stopped", "gpu-pipeline"],
      ["Capture stopped producing frames", "video-stall"],
      ["Source is smaller", "source-dimensions"],
      ["Capture worker stopped", "capture-worker"],
    ]) {
      if (message.includes(hint!)) stage = label!;
    }
  });
  child.stdin.on("error", () => {});
  const result = new Promise<unknown>((resolve, reject) => {
    child.once("error", () =>
      reject(new Error("Native fixture could not start.")),
    );
    child.once("close", (code, signal) => {
      if (code !== 0) {
        reject(
          new Error(
            `Native fixture failed at ${stage} (exit ${code ?? signal ?? "unknown"}; tags ${[...failureTags].join(",") || "none"}); run its bounded local diagnostic separately.`,
          ),
        );
        return;
      }
      try {
        resolve(JSON.parse(output.trim().split("\n").at(-1)!));
      } catch {
        reject(new Error("Native fixture returned invalid diagnostic JSON."));
      }
    });
  });
  // Callers may interact with the UI before awaiting completion. Attach an
  // immediate handler so early process failure cannot become an unhandled rejection.
  void result.catch(() => {});
  void result.catch((error: Error) => reportFailed(error));
  void reported.catch(() => {});
  child.stdin.end(JSON.stringify(request));
  return { child, result, reported };
}

export async function policyAdministrator(
  request: APIRequestContext,
  origin: string,
) {
  expect(new URL(origin).hostname).toMatch(
    /^(localhost|127\.0\.0\.1|\[::1\])$/,
  );
  const headers = { Origin: origin };
  const login = await request.post("/api/admin/login", {
    headers,
    data: { password: process.env.ADMIN_TOKEN },
  });
  expect(login.status()).toBe(200);
  const endpoint = "/api/admin/stream-policy";
  const read = async () => {
    const response = await request.get(endpoint);
    expect(response.status()).toBe(200);
    return (await response.json()).policy as StreamPolicy;
  };
  const write = async (policy: StreamPolicy) => {
    const response = await request.put(endpoint, { headers, data: policy });
    expect(response.status()).toBe(200);
  };
  const initial = await read();
  const emptyCamera = { maxCameraBitrateBps: null, cameraAutomaticTargets: [] };
  await write({
    ...defaultStreamPolicy,
    ...emptyCamera,
    revision: initial.revision,
  });
  const restore = async () =>
    write({ ...emptyCamera, ...initial, revision: (await read()).revision });
  return { read, write, restore };
}
