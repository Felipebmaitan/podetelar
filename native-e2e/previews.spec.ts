import { expect, test, type Page } from "@playwright/test";
import { nativeRequest } from "./fixtures.js";
import { createTestRoom } from "../scripts/test-room.js";

test("native JPEG previews update once a minute without any full media subscription", async ({
  page,
  browser,
  request,
  baseURL,
}) => {
  test.setTimeout(105_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    `Native previews ${Date.now()}`,
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const fixture = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Native preview",
    codec: "h265",
    seconds: 68,
    lingerSeconds: 5,
  });
  const late = await browser.newContext();
  const join = async (target: Page, name: string) => {
    await target.addInitScript(() => {
      const Native = window.RTCPeerConnection;
      const connections: RTCPeerConnection[] = [];
      (
        window as unknown as { previewConnections: RTCPeerConnection[] }
      ).previewConnections = connections;
      window.RTCPeerConnection = class extends Native {
        constructor(options?: RTCConfiguration) {
          super(options);
          connections.push(this);
        }
      };
    });
    await target.goto(invitationUrl);
    await target.getByLabel("Your name", { exact: true }).fill(name);
    await target
      .getByRole("button", { name: "Join private room", exact: true })
      .click();
  };
  const received = (target: Page) =>
    target.evaluate(async () => {
      let bytes = 0;
      for (const connection of (
        window as unknown as { previewConnections: RTCPeerConnection[] }
      ).previewConnections)
        (await connection.getStats()).forEach((entry) => {
          if (entry.type === "inbound-rtp")
            bytes += Number(entry.bytesReceived ?? 0);
        });
      return bytes;
    });
  try {
    const urls: string[] = [];
    page.on("response", (response) => {
      if (/\/previews\/[^/]+\/screen\//.test(response.url()) && response.ok())
        urls.push(response.url());
    });
    await join(page, "First viewer");
    const card = page.getByRole("article", {
      name: "Native preview screen",
      exact: true,
    });
    const image = card.getByRole("img", {
      name: "Native preview screen preview",
    });
    await expect(image).toBeVisible({ timeout: 25_000 });
    expect(
      await image.evaluate((element: HTMLImageElement) => [
        element.naturalWidth,
        element.naturalHeight,
      ]),
    ).toEqual([320, 180]);
    expect(await received(page)).toBe(0);
    expect(urls).toHaveLength(1);
    const original = urls[0]!;
    const friend = await late.newPage();
    await join(friend, "Late viewer");
    await expect(
      friend.getByRole("img", { name: "Native preview screen preview" }),
    ).toBeVisible({ timeout: 15_000 });
    expect(await received(friend)).toBe(0);
    // Wait for the actual one-minute publisher gate, not an accelerated clock.
    await expect
      .poll(() => urls.length, { timeout: 68_000, intervals: [1000] })
      .toBe(2);
    expect(urls[1]).not.toBe(original);
    expect(await received(page)).toBe(0);
    expect(await received(friend)).toBe(0);
    await expect(page.locator("video")).toHaveCount(0);
    await expect(friend.locator("video")).toHaveCount(0);
    const report = (await fixture.reported) as {
      metrics: { preview_frames: number };
      previewUploads: number;
      captureStopped: boolean;
    };
    expect(report.metrics.preview_frames).toBe(2);
    expect(report.previewUploads).toBe(2);
    expect(report.captureStopped).toBe(true);
    await expect(card).toHaveCount(0);
    expect((await page.request.get(urls[1]!)).status()).toBe(404);
    await fixture.result;
    console.log(
      "Native: two 320×180 JPEGs in 68 seconds; late join reused preview; both viewers received zero RTP bytes; Stop denied image access.",
    );
  } finally {
    if (fixture.child.exitCode === null) fixture.child.kill();
    await late.close();
  }
});
