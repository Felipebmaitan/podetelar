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
});
