import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
import { nativeRequest, toneFixtures } from "./fixtures.js";

test("selected application stereo audio reaches a browser and stops with Watch", async ({
  page,
  request,
  baseURL,
}, testInfo) => {
  const fixtures = toneFixtures(testInfo.outputPath("audio-applications"));
  const children: ReturnType<typeof nativeRequest>[] = [];
  try {
    fixtures.game();
    fixtures.discord();
    await delay(1500);
    const created = await request.post("/api/rooms", {
      headers: {
        Origin: baseURL!,
        Authorization: `Bearer ${process.env.ADMIN_TOKEN!}`,
      },
      data: { name: `Native audio ${Date.now()}`, displayName: "Owner" },
    });
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    await page.addInitScript(() => {
      const Original = window.RTCPeerConnection;
      const connections: RTCPeerConnection[] = [];
      (
        window as unknown as { audioTestConnections: RTCPeerConnection[] }
      ).audioTestConnections = connections;
      window.RTCPeerConnection = class extends Original {
        constructor(options?: RTCConfiguration) {
          super(options);
          connections.push(this);
        }
      };
    });
    await page.goto(invitationUrl);
    const publisher = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Native audio",
      codec: "h265",
      seconds: 22,
      audio: { mode: "application", application: fixtures.application },
    });
    children.push(publisher);
    await page.getByLabel("Your name", { exact: true }).fill("Audio viewer");
    await page
      .getByRole("button", { name: "Join private room", exact: true })
      .click();
    const card = page.getByRole("article", {
      name: "Native audio screen",
      exact: true,
    });
    await expect(card).toBeVisible({ timeout: 30_000 });
    const bytes = () =>
      page.evaluate(async () => {
        const result = { audio: 0, video: 0 };
        for (const connection of (
          window as unknown as { audioTestConnections: RTCPeerConnection[] }
        ).audioTestConnections) {
          (await connection.getStats()).forEach((entry) => {
            if (
              entry.type === "inbound-rtp" &&
              (entry.kind === "audio" || entry.kind === "video")
            )
              result[entry.kind as "audio" | "video"] += Number(
                entry.bytesReceived ?? 0,
              );
          });
        }
        return result;
      });
    expect(await bytes()).toEqual({ audio: 0, video: 0 });
    await card.getByRole("button", { name: "Watch", exact: true }).click();
    await expect
      .poll(async () => (await bytes()).audio, { timeout: 15_000 })
      .toBeGreaterThan(5000);
    await expect
      .poll(() =>
        card
          .locator("video")
          .evaluate(
            (element: HTMLVideoElement) =>
              (element.srcObject as MediaStream)?.getAudioTracks().length ?? 0,
          ),
      )
      .toBe(1);
    const spectrum = await card
      .locator("video")
      .evaluate(async (element: HTMLVideoElement) => {
        const stream = element.srcObject as MediaStream;
        const context = new AudioContext({ sampleRate: 48000 });
        const source = context.createMediaStreamSource(stream);
        const splitter = context.createChannelSplitter(2);
        const analysers = [context.createAnalyser(), context.createAnalyser()];
        for (const analyser of analysers) {
          analyser.fftSize = 8192;
          analyser.smoothingTimeConstant = 0;
        }
        const quiet = context.createGain();
        quiet.gain.value = 0;
        source.connect(splitter);
        analysers.forEach((analyser, index) => {
          splitter.connect(analyser, index);
          analyser.connect(quiet);
        });
        quiet.connect(context.destination);
        await context.resume();
        const started = performance.now();
        let levels: number[][] = [];
        let stable = 0;
        while (performance.now() - started < 8000 && stable < 3) {
          await new Promise((resolve) => setTimeout(resolve, 250));
          levels = analysers.map((analyser) => {
            const frequency = new Float32Array(analyser.frequencyBinCount);
            analyser.getFloatFrequencyData(frequency);
            return [440, 660, 880].map((tone) => {
              const bin = Math.round(
                (tone * analyser.fftSize) / context.sampleRate,
              );
              return Math.max(...frequency.slice(bin - 2, bin + 3));
            });
          });
          const separated =
            levels[0]![0]! > -65 &&
            levels[1]![1]! > -65 &&
            levels[1]![0]! < levels[0]![0]! - 25 &&
            levels[0]![1]! < levels[1]![1]! - 25;
          stable = separated ? stable + 1 : 0;
        }
        const channels = stream.getAudioTracks()[0]!.getSettings().channelCount;
        await context.close();
        return {
          levels,
          channels,
          stable,
          startupMs: Math.round(performance.now() - started),
        };
      });
    const negotiation = await page.evaluate(() => ({
      opus: (
        window as unknown as { audioTestConnections: RTCPeerConnection[] }
      ).audioTestConnections.map((pc) => ({
        localStereo: /[ ;]stereo=1(?:;|\r|\n|$)/.test(
          pc.localDescription?.sdp ?? "",
        ),
        remoteStereo: /[ ;]stereo=1(?:;|\r|\n|$)/.test(
          pc.remoteDescription?.sdp ?? "",
        ),
        remoteSenderStereo: /sprop-stereo=1(?:;|\r|\n|$)/.test(
          pc.remoteDescription?.sdp ?? "",
        ),
      })),
    }));
    console.log(
      `Controlled stereo probe: ${JSON.stringify({ spectrum, negotiation })}`,
    );
    expect(spectrum.levels[0]![0]).toBeGreaterThan(-65);
    expect(spectrum.levels[1]![1]).toBeGreaterThan(-65);
    expect(spectrum.levels[1]![0]).toBeLessThan(spectrum.levels[0]![0]! - 25);
    expect(spectrum.levels[0]![1]).toBeLessThan(spectrum.levels[1]![1]! - 25);
    expect(spectrum.levels[0]![2]).toBeLessThan(spectrum.levels[0]![0]! - 30);
    expect(spectrum.channels).toBe(2);
    expect(spectrum.stable).toBe(3);
    await card
      .getByRole("button", { name: "Stop watching", exact: true })
      .click();
    await page.waitForTimeout(1200);
    const stopped = await bytes();
    await page.waitForTimeout(1000);
    const after = await bytes();
    expect(after.audio - stopped.audio).toBeLessThan(1024);
    expect(after.video - stopped.video).toBeLessThan(1024);
    await card.getByRole("button", { name: "Watch", exact: true }).click();
    await expect
      .poll(async () => (await bytes()).audio)
      .toBeGreaterThan(after.audio + 2000);
    const result = (await publisher.result) as {
      captureStopped: boolean;
      audio: { frames: number };
    };
    expect(result.captureStopped).toBe(true);
    expect(result.audio.frames).toBeGreaterThan(48000 * 18);
    await expect(card).toHaveCount(0);
    console.log(
      "Native stereo Opus decoded through SFU: game tone present, Discord tone absent, A/V subscription traffic stopped, native capture released.",
    );
  } finally {
    for (const { child } of children) if (child.exitCode === null) child.kill();
    fixtures.stop();
  }
});
