import { expect, test } from "@playwright/test";

test("administrator login, filters, charts, reload and logout", async ({
  page,
  context,
  baseURL,
}) => {
  await page.goto("/");
  await expect(page).toHaveTitle("PodeTelar");
  await expect(page.getByText("PodeTelar", { exact: true })).toBeVisible();
  await expect
    .poll(() =>
      page
        .locator(".admin-brand-mark")
        .evaluate((image: HTMLImageElement) => image.naturalWidth),
    )
    .toBeGreaterThan(0);
  await expect(
    page.getByRole("heading", { name: "Administrator", exact: true }),
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
  await page.getByRole("button", { name: "Usage", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Usage", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("img", { name: /Server traffic/ })).toBeVisible();
  await expect(
    page
      .getByRole("img", { name: "Server traffic in megabits per second" })
      .getByText("Mb/s", { exact: true }),
  ).toBeVisible();
  const cookies = await context.cookies(`${baseURL}/api/admin/session`);
  const admin = cookies.find((c) => c.name === "podetelar_admin");
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
  await page.setViewportSize({ width: 320, height: 740 });
  const picker = page.getByRole("combobox", { name: "Period", exact: true });
  const assertTickSpacing = async () => {
    const ticks = page.locator(".admin-server-chart svg > text[text-anchor]");
    await expect
      .poll(async () => {
        const boxes = await ticks.evaluateAll((nodes) =>
          nodes.map((node) => node.getBoundingClientRect().toJSON()),
        );
        if (boxes.length < 2) return 0;
        return Math.min(
          ...boxes.slice(1).map((box, index) => box.left - boxes[index]!.right),
        );
      })
      .toBeGreaterThanOrEqual(4);
  };
  await picker.selectOption("7 days");
  await expect(picker).toHaveValue("7 days");
  await expect(page.getByRole("img", { name: /Server traffic/ })).toBeVisible();
  await assertTickSpacing();
  await page
    .getByRole("button", { name: "Guest profiles", exact: true })
    .click();
  await page.goBack();
  await expect(picker).toHaveValue("7 days");
  await picker.selectOption("Month");
  await page.reload();
  await expect(picker).toHaveValue("Month");
  await assertTickSpacing();
  await picker.selectOption("Custom period");
  await expect(
    page.getByRole("button", { name: "Apply period" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  const yesterday = await page.evaluate(() => {
    const date = new Date();
    date.setDate(date.getDate() - 1);
    return [
      date.getFullYear(),
      String(date.getMonth() + 1).padStart(2, "0"),
      String(date.getDate()).padStart(2, "0"),
    ].join("-");
  });
  await page.getByLabel("From", { exact: true }).fill(yesterday);
  await page.getByRole("button", { name: "Apply period" }).click();
  await page.reload();
  await expect(picker).toHaveValue("Custom period");
  await expect(page.getByLabel("From", { exact: true })).toHaveValue(yesterday);
  await expect(page.getByRole("img", { name: /Server traffic/ })).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: "Usage", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Usage", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(
    page.getByRole("heading", { name: "Administrator", exact: true }),
  ).toBeVisible();
  expect((await page.request.get("/api/admin/session")).status()).toBe(401);
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Administrator", exact: true }),
  ).toBeVisible();
});
