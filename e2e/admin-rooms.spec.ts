import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import type { Room as LiveRoom } from "livekit-client";

async function signIn(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN!)
    .catch(() => {
      throw new Error("Could not enter test administrator credential");
    });
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Rooms", exact: true }),
  ).toBeVisible();
}

test("room creation, inline expansion, actual clipboard, search, reload and confirmed closure", async ({
  page,
  context,
  baseURL,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"], {
    origin: baseURL!,
  });
  await signIn(page);
  const name = `Friday games ${Date.now()}`;
  await expect(page.getByLabel("Room name", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "+ New room", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "New room", exact: true });
  await expect(dialog).toBeVisible();
  await expect(page.getByLabel("Room name", { exact: true })).toBeFocused();
  await page.getByLabel("Room name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Create room", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  const row = page.getByRole("button", { name: new RegExp(name) });
  await expect(row).toHaveAttribute("aria-expanded", "true");
  await expect(
    page.getByText("Ready for the first friend to join.", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Copy invite", exact: true }).click();
  await expect(
    page.getByText("Invitation copied.", { exact: true }),
  ).toBeVisible();
  const invitation = await page.evaluate(() => navigator.clipboard.readText());
  const id = new URL(invitation).pathname.split("/").pop()!;
  expect(new URL(invitation).hash).toMatch(/^#invite=[A-Za-z0-9_-]{43}$/);
  await row.click();
  await expect(
    page.getByRole("button", { name: "Copy invite", exact: true }),
  ).toHaveCount(0);
  await row.press("Enter");
  await expect(row).toHaveAttribute("aria-expanded", "true");
  await page.getByRole("searchbox").fill("nothing matches this room");
  await expect(
    page.getByRole("heading", { name: "No matching rooms", exact: true }),
  ).toBeVisible();
  await page.getByRole("searchbox").fill("");
  await page.reload();
  await expect(row).toHaveAttribute("aria-expanded", "false");
  await row.click();
  await page.getByRole("button", { name: "Copy invite", exact: true }).click();
  await expect(
    page.getByText("Invitation copied.", { exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    invitation,
  );
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
  await page.screenshot({
    path: test.info().outputPath("rooms-expanded.png"),
    fullPage: true,
  });
  await page.getByRole("button", { name: "Close room", exact: true }).click();
  const confirm = page.getByRole("dialog", {
    name: `Close ${name}?`,
    exact: true,
  });
  await confirm.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(row).toBeVisible();
  await page.getByRole("button", { name: "Close room", exact: true }).click();
  await confirm
    .getByRole("button", { name: "Close room", exact: true })
    .click();
  await expect(row).toHaveCount(0);
  expect(
    (
      await page.request.post(`/api/admin/rooms/${id}/invitation`, {
        headers: { Origin: baseURL! },
      })
    ).status(),
  ).toBe(404);
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  expect((await page.request.get("/api/admin/rooms")).status()).toBe(401);
});

test("real SFU members survive a reconnect, then an empty room expires and disappears", async ({
  page,
  browser,
  request,
  baseURL,
}) => {
  test.setTimeout(150000);
  await signIn(page);
  const origin = baseURL!;
  const name = `Live friends ${Date.now()}`;
  const created = await page.request.post("/api/admin/rooms", {
    headers: { Origin: origin },
    data: { name },
  });
  expect(created.status()).toBe(201);
  const { room, invitationUrl } = await created.json();
  const joined = await request.post(`/api/rooms/${room.id}/join`, {
    headers: { Origin: origin },
    data: {
      invitation: new URL(invitationUrl).hash.slice(8),
      displayName: "Ana",
    },
  });
  expect(joined.status()).toBe(200);
  const tokenResponse = await request.post(
    `/api/rooms/${room.id}/native-media-token`,
    { headers: { Origin: origin } },
  );
  expect(tokenResponse.status()).toBe(200);
  const credentials = await tokenResponse.json();
  const participantContext = await browser.newContext({ bypassCSP: true });
  try {
    const participant = await participantContext.newPage();
    await participant.goto(origin);
    await participant.addScriptTag({
      content: readFileSync(
        new URL(
          "../node_modules/livekit-client/dist/livekit-client.umd.js",
          import.meta.url,
        ),
        "utf8",
      ),
    });
    const connect = () =>
      participant.evaluate(async ({ serverUrl, token }) => {
        const scope = window as unknown as {
          LivekitClient: { Room: new (options: object) => LiveRoom };
          testRoom: LiveRoom;
        };
        scope.testRoom = new scope.LivekitClient.Room({ autoSubscribe: false });
        try {
          await scope.testRoom.connect(serverUrl, token);
        } catch {
          throw new Error("Test participant could not connect to local SFU");
        }
      }, credentials);
    const disconnect = () =>
      participant.evaluate(async () => {
        await (
          window as unknown as { testRoom: LiveRoom }
        ).testRoom.disconnect();
      });
    await connect();
    const row = page.getByRole("button", { name: new RegExp(name) });
    await expect(row).toContainText("1 member", { timeout: 15000 });
    await row.click();
    await expect(
      page.locator(".admin-member-list").getByText("Ana", { exact: true }),
    ).toBeVisible();
    await page.screenshot({
      path: test.info().outputPath("rooms-live-member.png"),
      fullPage: true,
    });
    await disconnect();
    await expect(row).toContainText("Closing soon", { timeout: 15000 });
    await connect();
    await expect(row).toContainText("Active", { timeout: 15000 });
    await expect(
      page.locator(".admin-member-list").getByText("Ana", { exact: true }),
    ).toBeVisible();
    await disconnect();
    await expect(row).toContainText("Closing soon", { timeout: 15000 });
    // Real server time and SFU absence, no clock fast-forward or mocked HTTP.
    await expect(row).toHaveCount(0, { timeout: 80000 });
    expect(
      (
        await request.post(`/api/rooms/${room.id}/native-media-token`, {
          headers: { Origin: origin },
        })
      ).status(),
    ).toBe(401);
    expect(
      (
        await request.post(`/api/rooms/${room.id}/join`, {
          headers: { Origin: origin },
          data: {
            invitation: new URL(invitationUrl).hash.slice(8),
            displayName: "Returning friend",
          },
        })
      ).status(),
    ).toBe(404);
  } finally {
    await participantContext.close();
  }
});
test("room accordion and creation dialog fit a mobile viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await signIn(page);
  const name = `Mobile friends ${Date.now()}`;
  await page.getByRole("button", { name: "+ New room", exact: true }).click();
  await page.getByLabel("Room name", { exact: true }).fill(name);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  ).toBe(true);
  await page.getByRole("button", { name: "Create room", exact: true }).click();
  const row = page.getByRole("button", { name: new RegExp(name) });
  await expect(row).toHaveAttribute("aria-expanded", "true");
  await expect(
    page.getByRole("button", { name: "Copy invite", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Close room", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  ).toBe(true);
  await page.screenshot({
    path: test.info().outputPath("rooms-mobile.png"),
    fullPage: true,
  });
  await page.getByRole("button", { name: "Close room", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Close room", exact: true })
    .click();
  await expect(row).toHaveCount(0);
});
