import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

for (const codec of ["h265", "h264"] as const) {
  test(`native hardware ${codec} reaches a browser through the SFU`, async ({
    page,
    request,
    baseURL,
  }) => {
    const created = await createTestRoom(
      request,
      baseURL!,
      `Native ${codec} ${Date.now()}`,
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    const relay = process.env.FREEDOMSTREAM_NATIVE_ROUTE === "relay";
    const hold = Number(process.env.FREEDOMSTREAM_NATIVE_HOLD_SECONDS ?? 0);
    if (
      !Number.isInteger(hold) ||
      hold < 0 ||
      hold > 900 ||
      (hold > 0 && hold < 18)
    )
      throw new Error("Use a native hold duration from 18 to 900 seconds.");
    const seconds = hold || (relay ? 30 : 18);
    test.setTimeout(150_000 + seconds * 1000);
    const executable = process.env.FREEDOMSTREAM_NATIVE_EXE;
    if (!executable)
      throw new Error("Set FREEDOMSTREAM_NATIVE_EXE to the built publisher.");
    await page.addInitScript(() => {
      const Native = window.RTCPeerConnection;
      const connections: RTCPeerConnection[] = [];
      const errors: number[] = [];
      (
        window as unknown as { nativeTestIceErrors: number[] }
      ).nativeTestIceErrors = errors;
      (
        window as unknown as { nativeTestConnections: RTCPeerConnection[] }
      ).nativeTestConnections = connections;
      window.RTCPeerConnection = class extends Native {
        constructor(options?: RTCConfiguration) {
          super(options);
          connections.push(this);
          this.addEventListener("icecandidateerror", (event) =>
            errors.push(event.errorCode),
          );
        }
      };
    });
    await page.goto(invitationUrl);
    const receiveCodecs = await page.evaluate(() =>
      RTCRtpReceiver.getCapabilities("video")?.codecs.map((codec) =>
        codec.mimeType.toLowerCase(),
      ),
    );
    expect(receiveCodecs).toContain(`video/${codec}`);
    const child = spawn(resolve(executable), ["--publish-fixture"], {
      windowsHide: true,
      stdio: ["pipe", "pipe", "pipe"],
    });
    let output = "";
    let failed = false;
    let nativeTlsRejected = false;
    let nativeHandshakeTimedOut = false;
    child.stdout.on("data", (data: Buffer) => {
      output = (output + data.toString()).slice(-16_384);
    });
    // SDK/native diagnostics must never leak token-bearing messages in a failure report.
    child.stderr.on("data", (data: Buffer) => {
      const message = data.toString();
      nativeTlsRejected ||= message.includes(
        "TLS certificate verification failed",
      );
      nativeHandshakeTimedOut ||= message.includes(
        "Fixture media connection timed out",
      );
    });
    child.on("error", () => {
      failed = true;
    });
    const completion = new Promise<number | null>((resolve) => {
      child.once("exit", resolve);
      child.once("error", () => resolve(null));
    });
    child.stdin.on("error", () => {
      failed = true;
    });
    const startedAt = Date.now();
    child.stdin.end(
      JSON.stringify({
        invitationUrl,
        displayName: `Native ${codec}`,
        codec,
        seconds,
        route: relay ? "relay" : "auto",
      }),
    );
    let verified = false;
    try {
      await page.getByLabel("Your name", { exact: true }).fill("Viewer");
      await page
        .getByRole("button", { name: "Join private room", exact: true })
        .click();
      if (relay) {
        await expect(page.getByTestId("media-connection")).toHaveAttribute(
          "data-state",
          "connected",
          { timeout: 25_000 },
        );
        await page
          .getByLabel("Connection route", { exact: true })
          .selectOption("relay");
        await expect(page.getByTestId("media-connection")).toHaveAttribute(
          "data-state",
          "connected",
          { timeout: 25_000 },
        );
      }
      const card = page.getByRole("article", {
        name: `Native ${codec} screen`,
        exact: true,
      });
      await expect(card).toBeVisible({ timeout: 30_000 });
      await expect(
        card.getByRole("img", { name: `Native ${codec} screen preview` }),
      ).toBeVisible({ timeout: 15_000 });
      expect(
        await card
          .locator("img")
          .evaluate((image: HTMLImageElement) => [
            image.naturalWidth,
            image.naturalHeight,
          ]),
      ).toEqual([320, 180]);
      await expect(page.locator("video")).toHaveCount(0);
      const stats = () =>
        page.evaluate(async () => {
          let bytes = 0;
          let decoded = 0;
          let mime = "";
          let tls = false;
          for (const connection of (
            window as unknown as { nativeTestConnections: RTCPeerConnection[] }
          ).nativeTestConnections) {
            const stats = await connection.getStats();
            stats.forEach((entry) => {
              if (entry.type === "transport" && entry.selectedCandidatePairId) {
                const pair = stats.get(entry.selectedCandidatePairId);
                tls ||=
                  stats.get(pair?.localCandidateId)?.relayProtocol === "tls";
              }
            });
            stats.forEach((entry) => {
              if (entry.type === "inbound-rtp" && entry.kind === "video") {
                bytes += Number(entry.bytesReceived ?? 0);
                decoded += Number(entry.framesDecoded ?? 0);
                mime =
                  stats.get(entry.codecId)?.mimeType?.toLowerCase() ?? mime;
              }
            });
          }
          return { bytes, decoded, mime, tls };
        });
      expect((await stats()).bytes).toBe(0);
      await card.getByRole("button", { name: "Watch", exact: true }).click();
      await expect
        .poll(async () => (await stats()).decoded, { timeout: 20_000 })
        .toBeGreaterThan(10);
      expect((await stats()).mime).toBe(`video/${codec}`);
      if (relay) expect((await stats()).tls).toBe(true);
      await expect
        .poll(() =>
          card
            .locator("video")
            .evaluate((video: HTMLVideoElement) => video.videoWidth),
        )
        .toBe(1920);
      await card
        .getByRole("button", { name: "Stop watching", exact: true })
        .click();
      await page.waitForTimeout(1200);
      const stopped = (await stats()).bytes;
      await page.waitForTimeout(1000);
      expect((await stats()).bytes - stopped).toBeLessThan(1024);
      await card.getByRole("button", { name: "Watch", exact: true }).click();
      await expect
        .poll(
          () =>
            card
              .locator("video")
              .evaluate((video: HTMLVideoElement) => video.videoWidth),
          { timeout: 10_000 },
        )
        .toBe(1920);
      if (hold) {
        const deadline = startedAt + seconds * 1000 - 2000;
        while (Date.now() < deadline) {
          const before = (await stats()).decoded;
          await page.waitForTimeout(Math.min(5000, deadline - Date.now()));
          expect((await stats()).decoded).toBeGreaterThan(before);
        }
      }
      const exitCode = await completion;
      expect(failed).toBe(false);
      expect(exitCode).toBe(0);
      const result = JSON.parse(output.trim().split("\n").at(-1)!);
      expect(result.captureStopped).toBe(true);
      expect(result.metrics.accepted).toBeGreaterThan(100);
      expect(result.metrics.fps).toBeGreaterThan(50);
      expect(result.metrics.preview_frames).toBe(
        1 + Math.floor((seconds - 2) / 60),
      );
      // A capture in the last few seconds may still be retrying an HTTP upload
      // when Stop cancels it. Its bitmap must not hold stream shutdown open.
      expect(result.previewUploads).toBeGreaterThanOrEqual(
        1 + Math.floor(Math.max(0, seconds - 10) / 60),
      );
      expect(result.previewUploads).toBeLessThanOrEqual(
        result.metrics.preview_frames,
      );
      if (hold)
        expect(result.metrics.accepted).toBeGreaterThan((seconds - 2) * 50);
      if (relay) expect(result.delivery.route).toBe("TURN/TLS");
      await expect(card).toHaveCount(0);
      console.log(
        `${codec}: decoded via SFU, subscriptions stopped, native capture released; ${result.metrics.fps.toFixed(1)} encoded fps`,
      );
      console.log(`Delivery: ${JSON.stringify(result.delivery)}`);
      verified = true;
    } finally {
      if (!verified)
        console.log(
          "Browser ICE summary",
          await page
            .evaluate(() => ({
              errors:
                (window as unknown as { nativeTestIceErrors?: number[] })
                  .nativeTestIceErrors ?? [],
              peers: (
                (
                  window as unknown as {
                    nativeTestConnections?: RTCPeerConnection[];
                  }
                ).nativeTestConnections ?? []
              ).map((pc) => ({
                state: pc.connectionState,
                policy: pc.getConfiguration().iceTransportPolicy,
              })),
            }))
            .catch(() => ({ unavailable: true })),
        );
      if (failed || child.exitCode !== 0)
        console.log({
          nativeExitCode: child.exitCode,
          nativeTlsRejected,
          nativeHandshakeTimedOut,
        });
      if (child.exitCode === null) child.kill();
    }
  });
}
