import { expect, test } from "@playwright/test";
test("Discord administration shows configuration status without exposing credentials", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByRole("button", { name: "Discord", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Discord", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Bot connection not configured", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Reconnect and resynchronize" }),
  ).toBeDisabled();
  const response = await page.request.get("/api/admin/discord");
  expect(response.status()).toBe(200);
  expect(JSON.stringify(await response.json())).not.toMatch(
    /botToken|publicKey|adminToken/,
  );
  await page.setViewportSize({ width: 390, height: 844 });
  const views = page.getByRole("navigation", {
    name: "Discord administration",
  });
  await views.getByRole("button", { name: "Channels", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Channel rooms", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("No voice channels discovered yet.", { exact: true }),
  ).toBeVisible();
  await views.getByRole("button", { name: "Accounts", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Discord accounts", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("No Discord accounts yet.", { exact: true }),
  ).toBeVisible();
  await views.getByRole("button", { name: "Auto-join", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Automatic joining", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", {
      name: "Pause automatic joining everywhere",
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("button", { name: "Enable automatic joining", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Enable automatic joining", exact: true })
    .click();
  await page
    .getByLabel("Discord user ID", { exact: true })
    .fill("323456789012345678");
  await page
    .getByRole("button", { name: "Pause for user", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Allow automatic joining", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Allow automatic joining", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Allow automatic joining", exact: true }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBeLessThanOrEqual(391);
});
