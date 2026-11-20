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
  const load = page.getByRole("button", { name: "Load access", exact: true });
  const revoke = page.getByRole("checkbox", {
    name: "Also end every current session",
  });
  await expect(load).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Lock", exact: true }),
  ).toHaveCount(0);
  await page
    .getByLabel("Server access key", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await load.click();
  await expect(
    page.getByRole("heading", { name: "Current sessions", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth + 1,
    ),
  ).toBe(true);
  await revoke.check();
  const mutationRequests: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().endsWith("/access"))
      mutationRequests.push(request.method());
  });
  await page.getByRole("button", { name: "Disable invitation" }).click();
  const warning = page.getByRole("dialog", { name: "Disable invitation?" });
  await expect(warning).toContainText("all current sessions will end");
  await expect(warning.getByRole("button", { name: "Cancel" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(warning).toHaveCount(0);
  expect(mutationRequests).toHaveLength(0);
  await page.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Current sessions", exact: true }),
  ).toHaveCount(0);
  expect(
    await page.getByLabel("Server access key", { exact: true }).inputValue(),
  ).toBe("");
  await page
    .getByLabel("Server access key", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await load.click();
  await expect(revoke).not.toBeChecked();
});
