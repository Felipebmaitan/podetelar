import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, toneFixtures } from "./fixtures.js";

test("Stop after a full native reconnect removes both regenerated publications", async ({
  page,
  request,
  baseURL,
}, testInfo) => {
  const fixtures = toneFixtures(testInfo.outputPath("recovery-application"));
  let publisher: ReturnType<typeof nativeRequest> | undefined;
  try {
    fixtures.game();
    await delay(1500);
    const created = await createTestRoom(
      request,
      baseURL!,
      `Native recovery ${Date.now()}`,
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    const previews: string[] = [];
    page.on("response", (response) => {
      if (/\/previews\/[^/]+\/screen\//.test(response.url()) && response.ok())
        previews.push(response.url());
    });
    await page.goto(invitationUrl);
    await page.getByLabel("Your name", { exact: true }).fill("Recovery viewer");
    await page
      .getByRole("button", { name: "Join private room", exact: true })
      .click();
    publisher = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Recovering publisher",
      codec: "h265",
      seconds: 26,
      audio: { mode: "application", application: fixtures.application },
      reconnectAfterSeconds: 8,
      lingerSeconds: 6,
    });
    const card = page.getByRole("article", {
      name: "Recovering publisher screen",
      exact: true,
    });
    await expect(card).toBeVisible({ timeout: 30_000 });
    await expect(
      card.getByRole("img", { name: "Recovering publisher screen preview" }),
    ).toBeVisible();
    await card.getByRole("button", { name: "Watch", exact: true }).click();
    await expect(
      card.getByRole("button", { name: "Mute", exact: true }),
    ).toBeVisible();
    await expect
      .poll(() =>
        card
          .locator("video")
          .evaluate((video: HTMLVideoElement) => video.videoWidth),
      )
      .toBe(1920);

    // The publisher's full reconnect creates a new publication. A viewer must
    // choose it explicitly; a genuinely new stream never inherits Watch.
    await expect(
      card.getByRole("button", { name: "Watch", exact: true }),
    ).toBeVisible({ timeout: 18_000 });
    await expect(
      card.getByRole("img", { name: "Recovering publisher screen preview" }),
    ).toBeVisible();
    expect(previews).toHaveLength(2);
    expect(previews[1]).not.toBe(previews[0]);
    expect((await page.request.get(previews[0]!)).status()).toBe(404);
    await card.getByRole("button", { name: "Watch", exact: true }).click();
    await expect(
      card.getByRole("button", { name: "Mute", exact: true }),
    ).toBeVisible();
    await expect
      .poll(
        () =>
          card
            .locator("video")
            .evaluate((video: HTMLVideoElement) => video.videoWidth),
        { timeout: 12_000 },
      )
      .toBe(1920);
    const stopped = (await publisher.reported) as {
      captureStopped: boolean;
      republished: number;
      metrics: { preview_frames: number };
      previewUploads: number;
    };
    expect(stopped.captureStopped).toBe(true);
    expect(stopped.republished).toBe(2);
    expect(stopped.metrics.preview_frames).toBe(1);
    expect(stopped.previewUploads).toBe(2);
    await expect(card).toHaveCount(0, { timeout: 2500 });
    // The media identity intentionally lingers for six seconds. Removal must
    // come from Stop, rather than a later room disconnect hiding stale SIDs.
    expect(publisher.child.exitCode).toBe(null);
    await expect(page.getByText("2 in room", { exact: true })).toBeVisible();
    await publisher.result;
    console.log(
      "Native full reconnect republished audio/video; Stop removed current publications while the participant remained connected.",
    );
  } finally {
    if (publisher?.child.exitCode === null) publisher.child.kill();
    fixtures.stop();
  }
});
