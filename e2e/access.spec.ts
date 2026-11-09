import { expect, test } from "@playwright/test";
import { createTestRoom, openTestRoom } from "../scripts/test-room.js";

test("browser room access stays private and cannot request native media grants", async ({
  page,
  request,
  baseURL,
}) => {
  const created = await createTestRoom(request, baseURL!, "App-only access");
  const { room, invitationUrl } = await created.json();
  await openTestRoom(page, invitationUrl);
  await page.getByLabel("Your name", { exact: true }).fill("Browser guest");
  await page
    .getByRole("button", { name: "Join private room", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Share and watch in the desktop app" }),
  ).toBeVisible();
  const status = await page.evaluate(async (room) => {
    const response = await fetch(`/api/rooms/${room}/native-media-token`, {
      method: "POST",
    });
    return response.status;
  }, room.id);
  expect(status).toBe(403);
  await expect(
    page.getByRole("button", { name: "Watch", exact: true }),
  ).toHaveCount(0);
});

test("room management fits the viewport and Lock clears operator authority", async ({
  page,
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Management layout");
  const { room } = await created.json();
  await page.goto(`/manage/${room.id}`);
  await page
    .getByLabel("Server access key", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Load access", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Current sessions", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth + 1,
    ),
  ).toBe(true);
  await page.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Current sessions", exact: true }),
  ).toHaveCount(0);
  expect(
    await page.getByLabel("Server access key", { exact: true }).inputValue(),
  ).toBe("");
});
