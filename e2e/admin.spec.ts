import { expect, test } from "@playwright/test";

test("administrator login, filters, responsive charts, reload and logout", async ({
  page,
  context,
  baseURL,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Administrator login" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Create a room" }),
  ).toHaveCount(0);
  expect(
    (await page.request.get("/api/admin/usage?from=0&to=60000")).status(),
  ).toBe(401);
  await page
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN!)
    .catch(() => {
      throw new Error("Could not fill administrator credential");
    });
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Usage", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("img", { name: /Server traffic/ })).toBeVisible();
  await expect(
    page.getByText("Speed in Mb/s · Data in MB or GB"),
  ).toBeVisible();
  const cookies = await context.cookies(`${baseURL}/api/admin/session`);
  const admin = cookies.find((c) => c.name === "fs_admin");
  expect(admin?.httpOnly).toBe(true);
  expect(admin?.sameSite).toBe("Strict");
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).toBe("{}");
  for (const period of ["Real time", "7 days", "Month", "Today"]) {
    await page.getByRole("button", { name: period, exact: true }).click();
    await expect(
      page.getByRole("button", { name: period, exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(
      page.getByRole("img", { name: /Server traffic/ }),
    ).toBeVisible();
  }
  await page.getByRole("button", { name: "Custom period" }).click();
  await page.getByRole("button", { name: "Apply period" }).click();
  await expect(page.getByRole("img", { name: /Server traffic/ })).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  ).toBe(true);
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Usage", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(
    page.getByRole("heading", { name: "Administrator login" }),
  ).toBeVisible();
  expect((await page.request.get("/api/admin/session")).status()).toBe(401);
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Administrator login" }),
  ).toBeVisible();
});
