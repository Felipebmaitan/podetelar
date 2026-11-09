import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

test("route edits during admission abandon the old connection and keep the latest choice", async ({
  page,
  request,
  baseURL,
}, info) => {
  test.skip(info.project.name === "mobile", "Shared connection lifecycle.");
  const created = await createTestRoom(request, baseURL!, "Route replacement");
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  let intercepted = false;
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/rooms/*/media-token", async (route) => {
    if (!intercepted) {
      intercepted = true;
      await pending;
    }
    await route.continue().catch(() => {});
  });
  try {
    await page.goto(invitationUrl);
    await page.getByLabel("Your name", { exact: true }).fill("Friend");
    await page
      .getByRole("button", { name: "Join private room", exact: true })
      .click();
    await expect.poll(() => intercepted).toBe(true);
    await page
      .getByLabel("Connection route", { exact: true })
      .selectOption("relay");
    await page
      .getByLabel("Connection route", { exact: true })
      .selectOption("auto");
    release();
    await expect(page.getByTestId("media-connection")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 25_000 },
    );
    await expect(
      page.getByLabel("Connection route", { exact: true }),
    ).toHaveValue("auto");
    await page
      .getByRole("button", { name: "Share camera", exact: true })
      .click();
    await expect(
      page.getByRole("article", { name: "Friend camera", exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Leave room", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "A room for your people." }),
    ).toBeVisible();
  } finally {
    release();
  }
});
