import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

test("five synthetic publishers serve twenty selected subscriptions", async ({
  browser,
  request,
  baseURL,
}) => {
  test.skip(
    process.env.FREEDOMSTREAM_GROUP_LOAD !== "1",
    "Opt-in GPU/network qualification on the production host.",
  );
  test.setTimeout(180_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Five-stream qualification",
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const contexts = await Promise.all(
    Array.from({ length: 5 }, () => browser.newContext()),
  );
  const pages = await Promise.all(contexts.map((context) => context.newPage()));
  const fixtures = Array.from({ length: 5 }, (_, i) =>
    nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: `Load ${i + 1}`,
      codec: "h265",
      seconds: 70,
      quality: {
        resolution: "p720",
        fps: 30,
        profile: "motion",
        bitrate: { mode: "manual", bps: 3_000_000 },
      },
    }),
  );
  try {
    // Stagger admission so a burst doesn't turn this media check into a rate-limit test.
    for (const [i, page] of pages.entries()) {
      await page.goto(invitationUrl);
      await page
        .getByLabel("Your name", { exact: true })
        .fill(`Load viewer ${i + 1}`);
      await page
        .getByRole("button", { name: "Join private room", exact: true })
        .click();
      await expect(page.getByTestId("media-connection")).toHaveAttribute(
        "data-state",
        "connected",
        { timeout: 25_000 },
      );
      await expect(page.getByRole("article")).toHaveCount(5, {
        timeout: 20_000,
      });
      for (let publisher = 1; publisher <= 5; publisher++) {
        if (publisher === i + 1) continue;
        await page
          .getByRole("article", {
            name: `Load ${publisher} screen`,
            exact: true,
          })
          .getByRole("button", { name: "Watch", exact: true })
          .click();
      }
      await expect(page.locator("video")).toHaveCount(4);
    }
    const decoded = async () =>
      Promise.all(
        pages.map((page) =>
          page
            .locator("video")
            .evaluateAll((videos) =>
              videos.map(
                (video) =>
                  (video as HTMLVideoElement).getVideoPlaybackQuality()
                    .totalVideoFrames,
              ),
            ),
        ),
      );
    await expect
      .poll(
        async () =>
          (await decoded()).flat().filter((frames) => frames > 10).length,
        { timeout: 20_000 },
      )
      .toBe(20);
    const before = await decoded();
    await pages[0]!.waitForTimeout(10_000);
    const after = await decoded();
    for (let viewer = 0; viewer < 5; viewer++)
      for (let stream = 0; stream < 4; stream++)
        expect(
          after[viewer]![stream]! - before[viewer]![stream]!,
        ).toBeGreaterThan(150);
    const reports = (await Promise.all(
      fixtures.map((fixture) => fixture.result),
    )) as { metrics: { fps: number }; delivery: unknown }[];
    for (const report of reports)
      expect(report.metrics.fps).toBeGreaterThan(28);
    console.log(
      "Five 720p30 HEVC/3 Mbps sources and twenty selected browser subscriptions decoded continuously; publisher aggregates:",
      JSON.stringify(
        reports.map((report) => ({
          fps: report.metrics.fps,
          delivery: report.delivery,
        })),
      ),
    );
  } finally {
    for (const fixture of fixtures)
      if (fixture.child.exitCode === null) fixture.child.kill();
    await Promise.all(contexts.map((context) => context.close()));
  }
});
