import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

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
