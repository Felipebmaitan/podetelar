import { expect, test, type Page } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

test("browser preview capture has a minute cadence and unwatched tiles carry only private JPEGs", async ({
  browser,
  request,
  baseURL,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "desktop",
    "Preview transport runs once; room flows cover viewport sizes.",
  );
  test.setTimeout(100_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    `Browser previews ${Date.now()}`,
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const publisherContext = await browser.newContext();
  const viewerContext = await browser.newContext();
  const lateContext = await browser.newContext();
  try {
    const publisher = await publisherContext.newPage();
    const viewer = await viewerContext.newPage();
    let uploads = 0;
    await publisher.route("**/previews/camera", async (route) => {
      if (route.request().method() === "POST" && ++uploads === 1)
        await route.fulfill({
          status: 503,
          contentType: "application/json",
          headers: { "Retry-After": "1" },
          body: JSON.stringify({ error: "Temporary preview test outage." }),
        });
      else await route.continue();
    });
    await publisher.addInitScript(() => {
      const probe = {
        captures: [] as number[],
        acquisitions: 0,
        tracks: [] as MediaStreamTrack[],
      };
      (window as unknown as { previewProbe: typeof probe }).previewProbe =
        probe;
      const toBlob = HTMLCanvasElement.prototype.toBlob;
      HTMLCanvasElement.prototype.toBlob = function (callback, type, quality) {
        if (type === "image/jpeg") probe.captures.push(performance.now());
        return toBlob.call(this, callback, type, quality);
      };
      const capture = navigator.mediaDevices.getUserMedia.bind(
        navigator.mediaDevices,
      );
      navigator.mediaDevices.getUserMedia = async (constraints) => {
        const stream = await capture(constraints);
        probe.acquisitions++;
        probe.tracks.push(...stream.getTracks());
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
    const urls: string[] = [];
    viewer.on("response", (response) => {
      if (/\/previews\/[^/]+\/camera\//.test(response.url()) && response.ok())
        urls.push(response.url());
    });
    await join(publisher, "Preview publisher");
    await join(viewer, "Preview viewer");
    await publisher
      .getByRole("button", { name: "Share camera", exact: true })
      .click();
    const card = viewer.getByRole("article", {
      name: "Preview publisher camera",
      exact: true,
    });
    await expect(
      card.getByRole("img", { name: "Preview publisher camera preview" }),
    ).toBeVisible({ timeout: 15_000 });
    expect(urls).toHaveLength(1);
    expect(
      await card
        .locator("img")
        .evaluate((image: HTMLImageElement) => [
          image.naturalWidth,
          image.naturalHeight,
        ]),
    ).toEqual([320, 180]);
    await expect(viewer.locator("video")).toHaveCount(0);
    const late = await lateContext.newPage();
    await join(late, "Late friend");
    await expect(
      late.getByRole("img", { name: "Preview publisher camera preview" }),
    ).toBeVisible({ timeout: 15_000 });
    const captures = () =>
      publisher.evaluate(
        () =>
          (
            window as unknown as {
              previewProbe: { captures: number[]; acquisitions: number };
            }
          ).previewProbe,
      );
    expect((await captures()).captures).toHaveLength(1);
    await expect
      .poll(() => urls.length, { timeout: 68_000, intervals: [1000] })
      .toBe(2);
    const probe = await captures();
    expect(probe.captures).toHaveLength(2);
    expect(probe.captures[1]! - probe.captures[0]!).toBeGreaterThanOrEqual(
      60_000,
    );
    expect(probe.acquisitions).toBe(1);
    expect(uploads).toBeGreaterThanOrEqual(3);
    await expect(viewer.locator("video")).toHaveCount(0);
    await publisher
      .getByRole("button", { name: "Stop camera", exact: true })
      .click();
    await expect(card).toHaveCount(0);
    expect((await viewer.request.get(urls[1]!)).status()).toBe(404);
    expect(
      await publisher.evaluate(() =>
        (
          window as unknown as { previewProbe: { tracks: MediaStreamTrack[] } }
        ).previewProbe.tracks.every((track) => track.readyState === "ended"),
      ),
    ).toBe(true);
  } finally {
    await Promise.all([
      publisherContext.close(),
      viewerContext.close(),
      lateContext.close(),
    ]);
  }
});
