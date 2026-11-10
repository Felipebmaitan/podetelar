import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

test("invitation page prepares a temporary native handoff without exposing the permanent invitation", async ({
  page,
  request,
  baseURL,
}) => {
  const created = await createTestRoom(
    request,
    baseURL!,
    `Browser handoff ${Date.now()}`,
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const secret = new URL(invitationUrl).hash.slice("#invite=".length);
  const urls: string[] = [];
  page.on("request", (request) => urls.push(request.url()));
  await page.goto(invitationUrl);
  await expect(
    page.getByRole("heading", { name: "Take your place." }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Prepare app invitation", exact: true })
    .click();
  const open = page.getByRole("link", {
    name: "Open in FreedomStream",
    exact: true,
  });
  await expect(open).toBeVisible();
  const appUrl = new URL((await open.getAttribute("href"))!);
  expect(appUrl.protocol).toBe("freedomstream:");
  expect(appUrl.href).not.toContain(secret);
  const ticket = appUrl.hash.slice("#handoff=".length);
  expect(ticket).toMatch(/^[A-Za-z0-9_-]{43}$/);
  expect(
    await page.evaluate(
      async (ticket) =>
        (
          await fetch("/api/app-links/redeem", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ ticket }),
          })
        ).status,
      ticket,
    ),
  ).toBe(403);
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
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  ).toBe(true);
  await page.clock.install();
  await page.clock.fastForward(60000);
  await open.click();
  await expect(page.getByRole("alert")).toContainText("expired");
  await expect(open).toHaveCount(0);
});
