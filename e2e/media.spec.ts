import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import type { Page } from "@playwright/test";

async function receivedVideoBytes(
  page: Page,
  kind: "video" | "audio" = "video",
): Promise<number> {
  return page.evaluate(async (kind) => {
    const connections = (
      window as unknown as { testConnections: RTCPeerConnection[] }
    ).testConnections;
    let bytes = 0;
    for (const connection of connections) {
      const stats = await connection.getStats();
      stats.forEach((entry) => {
        if (
          entry.type === "inbound-rtp" &&
          (entry.kind === kind || entry.mediaType === kind)
        )
          bytes += Number(entry.bytesReceived ?? 0);
      });
    }
    return bytes;
  }, kind);
}

test("two publishers, selective A/V subscriptions, late join and cleanup use real SFU media", async ({
  browser,
  request,
  baseURL,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "desktop",
    "Transport scenario runs once; room flows cover both viewport sizes.",
  );
  test.setTimeout(90_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    `Media ${Date.now()}`,
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const contexts = await Promise.all([
    browser.newContext(),
    browser.newContext(),
    browser.newContext(),
  ]);
  try {
    const pages = await Promise.all(
      contexts.map((context) => context.newPage()),
    );
    const [alice, bob, viewer] = pages as [Page, Page, Page];
    await alice.addInitScript(() => {
      const probe = {
        acquisitions: 0,
        permissionQueries: 0,
        tracks: [] as MediaStreamTrack[],
        resumePermissions: () => {},
      };
      (window as unknown as { testCameraProbe: typeof probe }).testCameraProbe =
        probe;
      let grant!: (status: PermissionStatus) => void;
      const permissions = new Promise<PermissionStatus>((resolve) => {
        grant = resolve;
      });
      probe.resumePermissions = () =>
        grant({ state: "granted", onchange: null } as PermissionStatus);
      const query = navigator.permissions.query.bind(navigator.permissions);
      navigator.permissions.query = (descriptor) => {
        if (descriptor.name === "camera") {
          probe.permissionQueries += 1;
          return permissions;
        }
        return query(descriptor);
      };
      const capture = navigator.mediaDevices.getUserMedia.bind(
        navigator.mediaDevices,
      );
      navigator.mediaDevices.getUserMedia = async (constraints) => {
        const stream = await capture(constraints);
        probe.acquisitions += 1;
        probe.tracks.push(...stream.getTracks());
        (
          window as unknown as { testCaptureTracks: MediaStreamTrack[] }
        ).testCaptureTracks = stream.getTracks();
        return stream;
      };
    });
    await viewer.addInitScript(() => {
      const NativeConnection = window.RTCPeerConnection;
      const connections: RTCPeerConnection[] = [];
      (
        window as unknown as { testConnections: RTCPeerConnection[] }
      ).testConnections = connections;
      window.RTCPeerConnection = class extends NativeConnection {
        constructor(configuration?: RTCConfiguration) {
          super(configuration);
          connections.push(this);
        }
      };
    });
    // Synthetic screen/audio capture keeps CI deterministic. Signaling, codecs and RTP are real.
    await bob.addInitScript(() => {
      navigator.mediaDevices.getDisplayMedia = async () => {
        const stream = await navigator.mediaDevices.getUserMedia({
          video: true,
          audio: true,
        });
        (
          window as unknown as { testCaptureTracks: MediaStreamTrack[] }
        ).testCaptureTracks = stream.getTracks();
        return stream;
      };
    });
    const join = async (page: Page, name: string) => {
      await page.goto(invitationUrl);
      await page.getByLabel("Your name", { exact: true }).fill(name);
      await page
        .getByRole("button", { name: "Join private room", exact: true })
        .click();
      await expect(page.getByTestId("media-connection")).toHaveAttribute(
        "data-state",
        "connected",
        { timeout: 20_000 },
      );
    };
    await join(alice, "Alice");
    await join(bob, "Bob");
    await alice
      .getByRole("button", { name: "Share camera", exact: true })
      .click();
    await bob
      .getByRole("button", { name: "Share screen", exact: true })
      .click();
    await expect(
      alice.getByRole("button", { name: "Stop camera", exact: true }),
    ).toBeVisible();
    await expect(
      bob.getByRole("button", { name: "Stop sharing screen", exact: true }),
    ).toBeVisible();
    const ownScreen = bob.getByRole("article", {
      name: "Bob screen",
      exact: true,
    });
    await ownScreen
      .getByRole("button", { name: "Preview", exact: true })
      .click();
    await expect
      .poll(() =>
        ownScreen
          .locator("video")
          .evaluate((video: HTMLVideoElement) => video.muted),
      )
      .toBe(true);
    // Viewer arrives after publication and must learn metadata without receiving video.
    await join(viewer, "Viewer");
    await expect(viewer.getByRole("article")).toHaveCount(2);
    await expect(viewer.getByText("3 in room", { exact: true })).toBeVisible();
    await expect(viewer.locator("video")).toHaveCount(0);
    const idleBefore = await receivedVideoBytes(viewer);
    await viewer.waitForTimeout(1200);
    expect((await receivedVideoBytes(viewer)) - idleBefore).toBeLessThan(1024);
    const aliceCard = viewer.getByRole("article", {
      name: "Alice camera",
      exact: true,
    });
    const bobCard = viewer.getByRole("article", {
      name: "Bob screen",
      exact: true,
    });
    await aliceCard.getByRole("button", { name: "Watch", exact: true }).click();
    await expect
      .poll(
        async () => {
          const play = aliceCard.getByRole("button", {
            name: "Play stream",
            exact: true,
          });
          if (await play.isVisible()) await play.click();
          return aliceCard
            .locator("video")
            .evaluate(
              (video: HTMLVideoElement) =>
                video.videoWidth > 0 && video.currentTime > 0,
            );
        },
        { timeout: 20_000 },
      )
      .toBe(true);
    await expect
      .poll(() => receivedVideoBytes(viewer), { timeout: 10_000 })
      .toBeGreaterThan(idleBefore + 1000);
    await bobCard.getByRole("button", { name: "Watch", exact: true }).click();
    await expect(
      bobCard.getByRole("slider", { name: "Bob screen volume" }),
    ).toBeVisible({ timeout: 15_000 });
    await expect
      .poll(() => receivedVideoBytes(viewer, "audio"))
      .toBeGreaterThan(0);
    await bobCard.getByRole("slider").fill("0.25");
    await expect
      .poll(() =>
        bobCard
          .locator("video")
          .evaluate((video: HTMLVideoElement) => video.volume),
      )
      .toBe(0.25);
    await bobCard.getByRole("button", { name: "Mute", exact: true }).click();
    await expect
      .poll(() =>
        bobCard
          .locator("video")
          .evaluate((video: HTMLVideoElement) => video.muted),
      )
      .toBe(true);
    await aliceCard.getByRole("button", { name: "Focus", exact: true }).click();
    await expect(aliceCard).toHaveClass(/focused/);
    await expect(viewer.locator("video")).toHaveCount(2);
    await aliceCard
      .getByRole("button", { name: "Stop watching", exact: true })
      .click();
    await bobCard
      .getByRole("button", { name: "Stop watching", exact: true })
      .click();
    await expect(viewer.locator("video")).toHaveCount(0);
    await viewer.waitForTimeout(1200); // Drain in-flight RTP before checking stopped subscriptions.
    const stopped = await receivedVideoBytes(viewer);
    const stoppedAudio = await receivedVideoBytes(viewer, "audio");
    await viewer.waitForTimeout(1500);
    expect((await receivedVideoBytes(viewer)) - stopped).toBeLessThan(1024);
    expect(
      (await receivedVideoBytes(viewer, "audio")) - stoppedAudio,
    ).toBeLessThan(1024);
    // Reload establishes a fresh signaling connection; active publishers remain discoverable.
    await viewer.reload();
    await expect(viewer.getByTestId("media-connection")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 20_000 },
    );
    await expect(viewer.getByRole("article")).toHaveCount(2);
    await expect(viewer.locator("video")).toHaveCount(0);
    await alice
      .getByRole("button", { name: "Stop camera", exact: true })
      .click();
    await bob
      .getByRole("button", { name: "Stop sharing screen", exact: true })
      .click();
    for (const publisher of [alice, bob])
      await expect
        .poll(() =>
          publisher.evaluate(() =>
            (
              window as unknown as { testCaptureTracks: MediaStreamTrack[] }
            ).testCaptureTracks.every((track) => track.readyState === "ended"),
          ),
        )
        .toBe(true);
    await expect(viewer.getByRole("article")).toHaveCount(0);
    // Simulate the browser's native Stop sharing event through the real SDK.
    await bob
      .getByRole("button", { name: "Share screen", exact: true })
      .click();
    await expect(viewer.getByRole("article")).toHaveCount(1);
    await bob.evaluate(() => {
      const tracks = (
        window as unknown as { testCaptureTracks: MediaStreamTrack[] }
      ).testCaptureTracks;
      const video = tracks.find((track) => track.kind === "video")!;
      video.stop();
      video.dispatchEvent(new Event("ended"));
    });
    await expect
      .poll(() =>
        bob.evaluate(() =>
          (
            window as unknown as { testCaptureTracks: MediaStreamTrack[] }
          ).testCaptureTracks.every((track) => track.readyState === "ended"),
        ),
      )
      .toBe(true);
    await expect(viewer.getByRole("article")).toHaveCount(0);
    await expect(
      viewer.getByRole("heading", { name: "Who's sharing first?" }),
    ).toBeVisible();
    // A camera-ended callback must never queue hidden reacquisition after Stop.
    // The permission query is held pending if the SDK tries its managed recovery.
    await alice
      .getByRole("button", { name: "Share camera", exact: true })
      .click();
    await expect(viewer.getByRole("article")).toHaveCount(1);
    await alice.evaluate(() => {
      const video = (
        window as unknown as { testCaptureTracks: MediaStreamTrack[] }
      ).testCaptureTracks[0]!;
      video.stop();
      video.dispatchEvent(new Event("ended"));
    });
    const stopCamera = alice.getByRole("button", {
      name: "Stop camera",
      exact: true,
    });
    if (await stopCamera.isVisible()) await stopCamera.click();
    await expect(viewer.getByRole("article")).toHaveCount(0);
    await alice.evaluate(() =>
      (
        window as unknown as {
          testCameraProbe: { resumePermissions: () => void };
        }
      ).testCameraProbe.resumePermissions(),
    );
    await alice.waitForTimeout(600);
    expect(
      await alice.evaluate(() => {
        const probe = (
          window as unknown as {
            testCameraProbe: {
              acquisitions: number;
              permissionQueries: number;
              tracks: MediaStreamTrack[];
            };
          }
        ).testCameraProbe;
        return {
          acquisitions: probe.acquisitions,
          permissionQueries: probe.permissionQueries,
          allEnded: probe.tracks.every((track) => track.readyState === "ended"),
        };
      }),
    ).toEqual({ acquisitions: 2, permissionQueries: 0, allEnded: true });
    await alice
      .getByRole("button", { name: "Leave room", exact: true })
      .click();
    await expect(viewer.getByText("2 in room", { exact: true })).toBeVisible();
    expect((await request.get("/media/rtc/validate")).status()).toBe(401);
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
  }
});
