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

test("administrator can list and create rooms; guests cannot access either API", async ({
  page,
  browser,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("button", { name: "Rooms", exact: true }),
  ).toHaveCount(0);
  await page
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByRole("button", { name: "Rooms", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Rooms", exact: true }),
  ).toBeVisible();
  const name = `Friends ${Date.now()}`;
  await page.getByLabel("Room name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Create room", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: `${name} is ready`, exact: true }),
  ).toBeVisible();
  await expect(
    page
      .getByRole("region", { name: "Room directory" })
      .getByRole("heading", { name, exact: true }),
  ).toBeVisible();
  const invitation = await page
    .getByLabel("Room invitation", { exact: true })
    .inputValue();
  expect(new URL(invitation).hash).toMatch(/^#invite=[A-Za-z0-9_-]{43}$/);
  const guest = await browser.newContext();
  try {
    const friend = await guest.newPage();
    expect((await friend.request.get("/api/admin/rooms")).status()).toBe(401);
    await friend.goto(invitation);
    await friend.getByLabel("Your name", { exact: true }).fill("Friend");
    await friend
      .getByRole("button", { name: "Join private room", exact: true })
      .click();
    await expect(
      friend.getByRole("heading", { name, exact: true }),
    ).toBeVisible();
    expect((await friend.request.get("/api/admin/rooms")).status()).toBe(401);
    expect(
      (
        await friend.request.post("/api/admin/rooms", {
          headers: { Origin: new URL(invitation).origin },
          data: { name: "Forbidden" },
        })
      ).status(),
    ).toBe(401);
    await friend.goto("/");
    await expect(
      friend.getByRole("heading", { name: "Administrator login" }),
    ).toBeVisible();
    await expect(
      friend.getByRole("button", { name: "Rooms", exact: true }),
    ).toHaveCount(0);
  } finally {
    await guest.close();
  }
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).toBe("{}");
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  ).toBe(true);
  await page.getByRole("button", { name: "Usage", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Usage", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Rooms", exact: true }).click();
  await expect(page.getByLabel("Room invitation", { exact: true })).toHaveCount(
    0,
  );
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Usage", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Rooms", exact: true }).click();
  await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(
    page.getByRole("heading", { name: "Administrator login" }),
  ).toBeVisible();
  expect((await page.request.get("/api/admin/rooms")).status()).toBe(401);
});
