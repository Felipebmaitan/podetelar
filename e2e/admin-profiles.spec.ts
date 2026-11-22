import { expect, test } from "@playwright/test";
import sharp from "sharp";

test("administrator can inspect an animated guest picture, delete it, and block uploads", async ({
  page,
  request,
  baseURL,
}, testInfo) => {
  const origin = baseURL!;
  const created = await request.post("/api/rooms", {
    headers: {
      Origin: origin,
      Authorization: `Bearer ${process.env.ADMIN_TOKEN!}`,
    },
    data: {
      name: "Profile acceptance",
      displayName: "Animated guest",
      deviceId: `pc1_${"c".repeat(64)}`,
    },
  });
  expect(created.status()).toBe(201);
  const room = (await created.json()).room.id as string;
  const gif = await sharp(Buffer.from([255, 0, 0, 255, 0, 255, 0, 255]), {
    raw: { width: 1, height: 2, channels: 4, pageHeight: 1 },
  })
    .gif({ delay: [200, 200], loop: 0 })
    .toBuffer();
  const uploaded = await request.put(`/api/rooms/${room}/profile/avatar`, {
    headers: { Origin: origin, "Content-Type": "application/octet-stream" },
    data: gif,
  });
  expect(uploaded.status()).toBe(200);
  await page.goto("/");
  await page
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page
    .getByRole("button", { name: "Guest profiles", exact: true })
    .click();
  const card = page.locator("article").filter({
    has: page.getByRole("heading", { name: "Animated guest", exact: true }),
  });
  const picture = card.getByRole("img");
  await expect(picture).toBeVisible();
  await expect(card.getByText(/Animated GIF/)).toBeVisible();
  await expect
    .poll(() =>
      picture.evaluate((image: HTMLImageElement) => image.naturalWidth),
    )
    .toBe(128);
  await page.screenshot({
    path: testInfo.outputPath("guest-profiles.png"),
    fullPage: true,
  });
  await card
    .getByRole("button", { name: "Block uploads", exact: true })
    .click();
  await expect(
    card.getByText("Uploads blocked", { exact: true }),
  ).toBeVisible();
  const denied = await request.put(`/api/rooms/${room}/profile/avatar`, {
    headers: { Origin: origin, "Content-Type": "application/octet-stream" },
    data: gif,
  });
  expect(denied.status()).toBe(403);
  await card
    .getByRole("button", { name: "Delete picture", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog
    .getByRole("button", { name: "Confirm deletion", exact: true })
    .click();
  await expect(card.getByRole("img")).toHaveCount(0);
  const profiles = await request.get(`/api/rooms/${room}/profiles`);
  expect((await profiles.json()).profiles[0].avatarRevision).toBeNull();
  await page.reload();
  await page
    .getByRole("button", { name: "Guest profiles", exact: true })
    .click();
  await expect(
    card.getByText("No stored picture", { exact: true }),
  ).toBeVisible();
  await card
    .getByRole("button", { name: "Allow uploads", exact: true })
    .click();
  await expect(
    card.getByText("Uploads allowed", { exact: true }),
  ).toBeVisible();
  await request.post(`/api/rooms/${room}/leave`, {
    headers: { Origin: origin },
  });
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).toBe("{}");
});
