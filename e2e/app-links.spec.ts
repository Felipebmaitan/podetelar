import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

for (const width of [1280, 390])
  test(`invitation stays compact and prepares direct app navigation at ${width}px`, async ({
    page,
    request,
    baseURL,
  }) => {
    await page.setViewportSize({ width, height: 800 });
    const created = await createTestRoom(
      request,
      baseURL!,
      `Browser handoff ${Date.now()}`,
    );
    expect(created.status()).toBe(201);
    const { invitationUrl, room } = await created.json();
    const secret = new URL(invitationUrl).hash.slice("#invite=".length);
    const urls: string[] = [];
    const paths: string[] = [];
    page.on("request", (request) => {
      urls.push(request.url());
      paths.push(new URL(request.url()).pathname);
    });
    await page.goto(invitationUrl);
    const open = page.getByRole("button", {
      name: "Join room in PodeTelar",
      exact: true,
    });
    await expect(open).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "People in this room" }),
    ).toBeVisible();
    await expect(page.getByRole("button")).toHaveCount(1);
    await expect(page.getByRole("textbox")).toHaveCount(0);
    await expect(page.locator("aside, details")).toHaveCount(0);
    const appUrl = new URL((await open.getAttribute("href"))!);
    expect(appUrl.protocol).toBe("freedomstream:");
    expect(appUrl.href).not.toContain(secret);
    const ticket = appUrl.hash.slice("#handoff=".length);
    expect(ticket).toMatch(/^[A-Za-z0-9_-]{43}$/);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Page.enable");
    const navigation = new Promise<string>((resolve) =>
      cdp.on("Page.frameRequestedNavigation", (event) => {
        if (event.url.startsWith("freedomstream:")) resolve(event.url);
      }),
    );
    const before = await page.locator("main").innerText();
    if (width === 390) await open.press("Space");
    else await open.click();
    expect(await navigation).toBe(appUrl.href);
    await expect(open).toBeVisible();
    expect(await page.locator("main").innerText()).toBe(before);
    const redeemed = await request.post("/api/app-links/redeem", {
      headers: { Origin: baseURL! },
      data: { ticket },
    });
    expect(redeemed.status()).toBe(200);
    expect((await redeemed.json()).invitationUrl).toBe(invitationUrl);
    expect(
      (
        await request.post("/api/app-links/redeem", {
          headers: { Origin: baseURL! },
          data: { ticket },
        })
      ).status(),
    ).toBe(404);
    expect(
      await page.evaluate(() =>
        JSON.stringify({ ...localStorage, ...sessionStorage }),
      ),
    ).toBe("{}");
    expect(
      urls.every((url) => !url.includes(secret) && !url.includes(ticket)),
    ).toBe(true);
    expect(
      paths.some((path) =>
        /\/(join|media-token|native-media-token)$/.test(path),
      ),
    ).toBe(false);
    expect(
      (await page.request.get(`/api/rooms/${room.id}/session`)).status(),
    ).toBe(401);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth + 1,
      ),
    ).toBe(true);
    await page.clock.install();
    await page.clock.fastForward(46000);
    await expect(open).toBeVisible();
    expect(await open.getAttribute("href")).not.toBe(appUrl.href);
  });

test("a bare room URL does not reveal people or offer a handoff", async ({
  page,
}) => {
  await page.goto(`/r/${"a".repeat(22)}`);
  await expect(
    page.getByText("Open a complete, current room invitation to join."),
  ).toBeVisible();
  await expect(page.getByRole("button")).toHaveCount(0);
});
