import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
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
    const created = await request.post("/api/rooms", {
      headers: {
        Origin: baseURL!,
        Authorization: `Bearer ${process.env.ADMIN_TOKEN!}`,
      },
      data: { name: `Native recovery ${Date.now()}`, displayName: "Owner" },
    });
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
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
    };
    expect(stopped.captureStopped).toBe(true);
    expect(stopped.republished).toBe(2);
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
