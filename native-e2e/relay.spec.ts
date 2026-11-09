import { chromium, expect, test, type Page } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

// The qualification hostname maps to the supplied VPS only in this test browser.
// Certificate verification stays enabled; no changes to Windows DNS/hosts.
test("TLS relay carries selected media with non-proxied UDP disabled and rejects invalid relay credentials", async ({
  request,
  baseURL,
}) => {
  const holdSeconds = Number(process.env.FREEDOMSTREAM_RELAY_HOLD_SECONDS ?? 0);
  if (!Number.isInteger(holdSeconds) || holdSeconds < 0 || holdSeconds > 900)
    throw new Error("Choose a relay hold duration from 0 to 900 seconds.");
  test.setTimeout(150_000 + holdSeconds * 1000);
  test.skip(
    !process.env.FREEDOMSTREAM_RELAY_HOST,
    "Requires the documented TLS relay deployment.",
  );
  const host = process.env.FREEDOMSTREAM_RELAY_HOST!;
  const ip = process.env.FREEDOMSTREAM_RELAY_IP!;
  if (!/^[a-zA-Z0-9.-]+$/.test(host) || !/^\d{1,3}(?:\.\d{1,3}){3}$/.test(ip))
    throw new Error("Use a relay hostname and IPv4 address for qualification.");
  const created = await createTestRoom(
    request,
    baseURL!,
    "UDP-disabled relay qualification",
  );
  const { invitationUrl } = await created.json();
  const browser = await chromium.launch({
    channel: "chrome",
    headless: true,
    args: [
      `--host-resolver-rules=MAP ${host} ${ip}`,
      "--force-webrtc-ip-handling-policy=disable_non_proxied_udp",
      "--use-fake-device-for-media-stream",
      "--use-fake-ui-for-media-stream",
    ],
  });
  const contexts = await Promise.all([
    browser.newContext({ permissions: ["camera", "microphone"] }),
    browser.newContext(),
  ]);
  const pages = await Promise.all(contexts.map((context) => context.newPage()));
  try {
    for (const [index, page] of pages.entries()) {
      await page.addInitScript(() => {
        const Native = window.RTCPeerConnection;
        const connections: RTCPeerConnection[] = [];
        const errors: number[] = [];
        (window as unknown as { relayErrors: number[] }).relayErrors = errors;
        (
          window as unknown as { relayConnections: RTCPeerConnection[] }
        ).relayConnections = connections;
        window.RTCPeerConnection = class extends Native {
          constructor(config?: RTCConfiguration) {
            super(config);
            connections.push(this);
            this.addEventListener("icecandidateerror", (event) =>
              errors.push(event.errorCode),
            );
          }
        };
      });
      await page.goto(invitationUrl);
      await page
        .getByLabel("Your name", { exact: true })
        .fill(index === 0 ? "Relay publisher" : "Relay watcher");
      await page
        .getByRole("button", { name: "Join private room", exact: true })
        .click();
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
    const publisher = pages[0]!,
      viewer = pages[1]!;
    await publisher
      .getByRole("button", { name: "Share camera", exact: true })
      .click();
    const card = viewer.getByRole("article", {
      name: "Relay publisher camera",
      exact: true,
    });
    await expect(card).toBeVisible();
    await card.getByRole("button", { name: "Watch", exact: true }).click();
    await expect
      .poll(
        () =>
          card
            .locator("video")
            .evaluate((video: HTMLVideoElement) => video.videoWidth),
        { timeout: 20_000 },
      )
      .toBeGreaterThan(0);
    const routes = async (page: Page) =>
      page.evaluate(async () => {
        const routes: {
          type: string;
          protocol: string;
          urlTls: boolean;
          bytes: number;
        }[] = [];
        for (const pc of (
          window as unknown as { relayConnections: RTCPeerConnection[] }
        ).relayConnections) {
          if (
            pc.connectionState !== "connected" ||
            pc.getConfiguration().iceTransportPolicy !== "relay"
          )
            continue;
          const stats = await pc.getStats();
          stats.forEach((stat) => {
            if (stat.type !== "transport" || !stat.selectedCandidatePairId)
              return;
            const pair = stats.get(stat.selectedCandidatePairId);
            const candidate = pair && stats.get(pair.localCandidateId);
            if (candidate)
              routes.push({
                type: candidate.candidateType,
                protocol: candidate.relayProtocol,
                urlTls: String(candidate.url ?? "").startsWith("turns:"),
                bytes: Number(pair.bytesReceived) + Number(pair.bytesSent),
              });
          });
        }
        return routes;
      });
    for (const page of pages)
      console.log(
        "Selected relay aggregate",
        JSON.stringify(await routes(page)),
      );
    for (const page of pages)
      await expect
        .poll(
          async () => {
            const selected = await routes(page);
            return selected.some(
              (route) =>
                (route.type === "relay" || route.type === "prflx") &&
                route.protocol === "tls" &&
                route.urlTls &&
                route.bytes > 0,
            );
          },
          { timeout: 15_000 },
        )
        .toBe(true);
    await expect(
      card.getByLabel("Relay publisher camera received quality"),
    ).toContainText("TURN/TLS");
    const until = Date.now() + holdSeconds * 1000;
    let frames = await card
      .locator("video")
      .evaluate(
        (video: HTMLVideoElement) =>
          video.getVideoPlaybackQuality().totalVideoFrames,
      );
    while (Date.now() < until) {
      await viewer.waitForTimeout(Math.min(10_000, until - Date.now()));
      const next = await card
        .locator("video")
        .evaluate(
          (video: HTMLVideoElement) =>
            video.getVideoPlaybackQuality().totalVideoFrames,
        );
      expect(next).toBeGreaterThan(frames);
      frames = next;
    }
    const rejected = await viewer.evaluate(async () => {
      const peers = (
        window as unknown as { relayConnections: RTCPeerConnection[] }
      ).relayConnections;
      const valid = peers
        .filter(
          (pc) =>
            pc.connectionState === "connected" &&
            pc.getConfiguration().iceTransportPolicy === "relay",
        )
        .flatMap((pc) => pc.getConfiguration().iceServers ?? [])
        .filter((server) =>
          [server.urls].flat().some((url) => url.startsWith("turns:")),
        );
      if (!valid.length) return false;
      const pc = new RTCPeerConnection({
        iceTransportPolicy: "relay",
        iceServers: valid.map((server) => ({
          ...server,
          username: "invalid",
          credential: "invalid",
        })),
      });
      let candidates = 0;
      pc.addEventListener("icecandidate", (event) => {
        if (event.candidate) candidates++;
      });
      try {
        pc.createDataChannel("unauthorized-relay-probe");
        await pc.setLocalDescription(await pc.createOffer());
        await new Promise<void>((resolve) => {
          const timer = setTimeout(resolve, 7000);
          pc.addEventListener("icegatheringstatechange", () => {
            if (pc.iceGatheringState === "complete") {
              clearTimeout(timer);
              resolve();
            }
          });
        });
        return candidates === 0;
      } finally {
        pc.close();
      }
    });
    expect(rejected).toBe(true);
    console.log(
      "Selected publisher/viewer media used certificate-verified TURN/TLS with relay-only ICE and non-proxied UDP disabled; invalid relay credentials yielded no candidate.",
    );
  } catch (error) {
    for (const page of pages)
      console.log(
        "Relay aggregate diagnostic",
        await page.evaluate(async () => {
          const connections =
            (window as unknown as { relayConnections: RTCPeerConnection[] })
              .relayConnections ?? [];
          const summaries = [];
          for (const pc of connections) {
            const candidates: {
              type: string;
              protocol: string;
              relayProtocol: string;
            }[] = [];
            try {
              const stats = await pc.getStats();
              stats.forEach((stat) => {
                if (stat.type === "local-candidate")
                  candidates.push({
                    type: stat.candidateType,
                    protocol: stat.protocol,
                    relayProtocol: stat.relayProtocol ?? "unknown",
                  });
              });
            } catch {}
            summaries.push({
              state: pc.connectionState,
              policy: pc.getConfiguration().iceTransportPolicy,
              tlsPorts: (pc.getConfiguration().iceServers ?? [])
                .flatMap((server) => [server.urls].flat())
                .filter((url) => url.startsWith("turns:"))
                .map((url) => Number(/:(\d+)\?/.exec(url)?.[1] ?? 443)),
              candidates,
            });
          }
          return {
            peers: summaries,
            errors:
              (window as unknown as { relayErrors: number[] }).relayErrors ??
              [],
          };
        }),
      );
    throw error;
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
    await browser.close();
  }
});
