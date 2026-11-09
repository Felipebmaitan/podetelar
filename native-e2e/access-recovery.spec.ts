import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

test("native sharing survives an app restart, then revocation ends its actual capture", async ({
  page,
  request,
  baseURL,
}) => {
  test.skip(
    process.env.FREEDOMSTREAM_RESTART_APP !== "1",
    "Explicit local Docker restart qualification.",
  );
  const created = await createTestRoom(
    request,
    baseURL!,
    "Native access/recovery",
  );
  const { room, invitationUrl } = await created.json();
  await page.addInitScript(() => {
    const Native = window.RTCPeerConnection;
    const peers: RTCPeerConnection[] = [];
    (
      window as unknown as { recoveryPeers: RTCPeerConnection[] }
    ).recoveryPeers = peers;
    window.RTCPeerConnection = class extends Native {
      constructor(config?: RTCConfiguration) {
        super(config);
        peers.push(this);
      }
    };
  });
  await page.goto(invitationUrl);
  await page.getByLabel("Your name", { exact: true }).fill("Recovery watcher");
  await page
    .getByRole("button", { name: "Join private room", exact: true })
    .click();
  const child = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Native recovery/access",
    codec: "h264",
    seconds: 45,
  });
  const card = page.getByRole("article", {
    name: "Native recovery/access screen",
    exact: true,
  });
  const decoded = () =>
    page.evaluate(async () => {
      let frames = 0;
      for (const peer of (
        window as unknown as { recoveryPeers: RTCPeerConnection[] }
      ).recoveryPeers) {
        try {
          const stats = await peer.getStats();
          stats.forEach((stat) => {
            if (stat.type === "inbound-rtp" && stat.kind === "video")
              frames += Number(stat.framesDecoded ?? 0);
          });
        } catch {}
      }
      return frames;
    });
  const admin = {
    Origin: baseURL!,
    Authorization: `Bearer ${process.env.ADMIN_TOKEN}`,
  };
  try {
    await expect(card).toBeVisible({ timeout: 25_000 });
    await expect(card.getByRole("img")).toBeVisible();
    await card.getByRole("button", { name: "Watch", exact: true }).click();
    await expect.poll(decoded).toBeGreaterThan(10);
    try {
      await promisify(execFile)("docker", ["compose", "restart", "app"], {
        windowsHide: true,
        timeout: 20_000,
      });
    } catch {
      throw new Error("Local app restart did not complete.");
    }
    await expect
      .poll(
        async () =>
          request
            .get("/api/health")
            .then((r) => r.status())
            .catch(() => 0),
        { timeout: 20_000 },
      )
      .toBe(200);
    await expect(page.getByTestId("media-connection")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 20_000 },
    );
    await expect(card).toBeVisible({ timeout: 20_000 });
    const watch = card.getByRole("button", { name: "Watch", exact: true });
    if (await watch.isVisible()) await watch.click();
    const baseline = await decoded();
    await expect
      .poll(decoded, { timeout: 10_000 })
      .toBeGreaterThan(baseline + 30);
    const access = await request.get(`/api/rooms/${room.id}/access`, {
      headers: admin,
    });
    expect(access.status()).toBe(200);
    const identity = (await access.json()).sessions.find(
      (session: { displayName: string }) =>
        session.displayName === "Native recovery/access",
    )?.id;
    expect(Boolean(identity)).toBe(true);
    expect(
      (
        await request.delete(
          `/api/rooms/${room.id}/access/sessions/${identity}`,
          { headers: admin },
        )
      ).status(),
    ).toBe(200);
    await expect(card).toHaveCount(0, { timeout: 5000 });
    await expect(page.getByTestId("media-connection")).toHaveAttribute(
      "data-state",
      "connected",
    );
    const result = (await child.result) as {
      captureStopped: boolean;
      mode: { fps: number };
      metrics: { frames: number };
    };
    expect(result.captureStopped).toBe(true);
    expect(result.metrics.frames).toBeLessThan(result.mode.fps * 40);
    console.log(
      "Real app restart recovered native publication and selected playback; revoking only the native session removed its stream and stopped GPU capture before the fixture deadline.",
    );
  } finally {
    if (child.child.exitCode === null) child.child.kill();
  }
});
