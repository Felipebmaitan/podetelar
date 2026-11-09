import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";

test("create, share, join, resume and leave a private room", async ({
  page,
  browser,
  baseURL,
}) => {
  const name = `Evening room ${Date.now()}`;
  await page.goto("/");
  await page.getByLabel("Room name", { exact: true }).fill(name);
  await page.getByLabel("Your name", { exact: true }).fill("Felipe");
  await page.getByLabel("Server access key").fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Create private room" }).click();
  await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
  const invitation = await page
    .getByLabel("Room invitation", { exact: true })
    .inputValue();
  const roomId = new URL(invitation).pathname.split("/")[2]!;
  await expect(page).toHaveURL(`${baseURL}/r/${roomId}`);
  await expect(
    page.getByText("This invitation stays valid when you leave the room."),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);

  const guest = await browser.newContext();
  try {
    const friend = await guest.newPage();
    const requestedUrls: string[] = [];
    const mediaTokens: string[] = [];
    friend.on("request", (request) => requestedUrls.push(request.url()));
    friend.on("response", async (response) => {
      if (!response.url().endsWith("/media-token") || !response.ok()) return;
      const body = await response.json().catch(() => null);
      if (typeof body?.token === "string") mediaTokens.push(body.token);
    });
    await friend.goto(invitation);
    await expect(
      friend.getByRole("heading", { name: "Take your place." }),
    ).toBeVisible();
    await expect(friend).toHaveURL(`${baseURL}/r/${roomId}`);
    await friend.getByLabel("Your name", { exact: true }).fill("Friend");
    await friend.getByRole("button", { name: "Join private room" }).click();
    await expect(
      friend.getByRole("heading", { name, exact: true }),
    ).toBeVisible();
    await friend.reload();
    await expect(
      friend.getByRole("heading", { name, exact: true }),
    ).toBeVisible();
    await expect(friend.getByText("Friend", { exact: true })).toBeVisible();
    await expect(friend.getByTestId("media-connection")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 20_000 },
    );
    await expect.poll(() => mediaTokens.length).toBeGreaterThan(0);
    const secret = new URL(invitation).hash.slice("#invite=".length);
    const stored = await friend.evaluate(() =>
      JSON.stringify({
        local: { ...localStorage },
        session: { ...sessionStorage },
      }),
    );
    const sessionCookies = await guest.cookies();
    expect(stored.includes(secret)).toBe(false);
    for (const cookie of sessionCookies)
      expect(stored.includes(cookie.value)).toBe(false);
    for (const token of mediaTokens) expect(stored.includes(token)).toBe(false);
    expect(requestedUrls.every((url) => !url.includes(secret))).toBe(true);
    await friend
      .getByRole("button", { name: "Leave room", exact: true })
      .click();
    await expect(
      friend.getByRole("heading", { name: "A room for your people." }),
    ).toBeVisible();
    expect(
      (
        await guest.request.get(`${baseURL}/api/rooms/${roomId}/session`)
      ).status(),
    ).toBe(401);
  } finally {
    await guest.close();
  }
});

test("a room URL without its invitation cannot reveal the room or grant media access", async ({
  page,
  request,
  baseURL,
}) => {
  const name = `Hidden room ${Date.now()}`;
  const created = await createTestRoom(request, baseURL!, name);
  expect(created.status()).toBe(201);
  const { room } = await created.json();
  await page.goto(`/r/${room.id}`);
  await expect(
    page.getByText(
      "Open the complete invitation pinned in your group chat to join this room.",
    ),
  ).toBeVisible();
  await expect(page.getByText(name, { exact: true })).toHaveCount(0);
  expect(
    (
      await page.request.post(`/api/rooms/${room.id}/media-token`, {
        headers: { Origin: baseURL! },
      })
    ).status(),
  ).toBe(401);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});
