import { spawn, type ChildProcess } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve, join } from "node:path";

export function nativeExecutable(): string {
  const executable = process.env.FREEDOMSTREAM_NATIVE_EXE;
  if (!executable)
    throw new Error("Set FREEDOMSTREAM_NATIVE_EXE to the built publisher.");
  return resolve(executable);
}

export function toneFixtures(directory: string) {
  mkdirSync(directory, { recursive: true });
  const game = join(directory, "Game.exe");
  const discord = join(directory, "Discord.exe");
  // A portable launcher keeps its own name separate from the publisher process.
  // Tone fixtures need their real process names for application-audio policy.
  const tone = process.env.FREEDOMSTREAM_NATIVE_TONE_EXE ?? nativeExecutable();
  copyFileSync(tone, game);
  copyFileSync(tone, discord);
  const children: ChildProcess[] = [];
  const start = (executable: string, frequency: number) => {
    const child = spawn(executable, ["--audio-tone", String(frequency)], {
      windowsHide: true,
      stdio: "ignore",
    });
    children.push(child);
    return child;
  };
  return {
    application: game.toLowerCase(),
    game: () => start(game, 440),
    discord: () => start(discord, 880),
    stop: () => {
      for (const child of children) if (child.exitCode === null) child.kill();
    },
  };
}

export function nativeRequest(option: string, request: unknown) {
  const child = spawn(nativeExecutable(), [option], {
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
    output = (output + data.toString()).slice(-64_000);
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
