import { expect, test } from "@playwright/test";
import WebSocket from "ws";
import { createTestRoom } from "../scripts/test-room.js";

test.use({ screenshot: "off" });
test("operator rotation and revocation close real media and reject saved tokens", async ({
  browser,
  request,
  baseURL,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "desktop",
    "One real transport run covers both layouts; the separate management check covers viewport sizes.",
  );
  test.setTimeout(120_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Access qualification",
  );
  const { room, invitationUrl } = await created.json();
  const contexts = await Promise.all([
    browser.newContext({ permissions: ["camera", "microphone"] }),
    browser.newContext(),
    browser.newContext(),
  ]);
  const [publisher, viewer, operator] = await Promise.all(
    contexts.map((context) => context.newPage()),
  );
  let savedToken = "";
  let mediaUrl = "";
  viewer!.on("response", (response) => {
    if (response.url().endsWith("/media-token") && response.ok())
      void response.json().then((body) => {
        savedToken = body.token;
        mediaUrl = body.serverUrl;
      });
  });
  const join = async (page: NonNullable<typeof viewer>, name: string) => {
    await page.goto(invitationUrl);
    await page.getByLabel("Your name", { exact: true }).fill(name);
    await page
      .getByRole("button", { name: "Join private room", exact: true })
      .click();
    await expect(page.getByTestId("media-connection")).toHaveAttribute(
      "data-state",
      "connected",
    );
  };
  try {
    await join(publisher!, "Access publisher");
    await join(viewer!, "Access watcher");
    await publisher!
      .getByRole("button", { name: "Share camera", exact: true })
      .click();
    const card = viewer!.getByRole("article", {
      name: "Access publisher camera",
      exact: true,
    });
    await expect(card.getByRole("img")).toBeVisible({ timeout: 15_000 });
    const previewPath = await card.getByRole("img").getAttribute("src");
    // Blob URLs stay private in memory; obtain the publication descriptor through
    // its HTTP request instead of persisting any image or invitation artifact.
    expect(previewPath?.startsWith("blob:")).toBe(true);
    await card.getByRole("button", { name: "Watch", exact: true }).click();
    await expect
      .poll(() =>
        card.locator("video").evaluate((v: HTMLVideoElement) => v.videoWidth),
      )
      .toBeGreaterThan(0);
    const watcher = (
      await (await viewer!.request.get(`/api/rooms/${room.id}/session`)).json()
    ).participant.id;
    const publisherId = (
      await (
        await publisher!.request.get(`/api/rooms/${room.id}/session`)
      ).json()
    ).participant.id;
    await operator!.goto(`/manage/${room.id}`);
    await operator!
      .getByLabel("Server access key", { exact: true })
      .fill(process.env.ADMIN_TOKEN!);
    await operator!
      .getByRole("button", { name: "Load access", exact: true })
      .click();
    await expect(
      operator!.getByRole("button", {
        name: "End session for Access watcher",
        exact: true,
      }),
    ).toBeVisible();
    await operator!
      .getByRole("button", { name: "Rotate invitation", exact: true })
      .click();
    const next = await operator!
      .getByLabel("New room invitation", { exact: true })
      .inputValue();
    const oldInvite = new URL(invitationUrl).hash.slice(8);
    expect(
      (
        await request.post(`/api/rooms/${room.id}/join`, {
          headers: { Origin: baseURL! },
          data: { invitation: oldInvite, displayName: "Old invite" },
        })
      ).status(),
    ).toBe(404);
    await expect(viewer!.getByTestId("media-connection")).toHaveAttribute(
      "data-state",
      "connected",
    );
    await operator!
      .getByRole("button", {
        name: "End session for Access watcher",
        exact: true,
      })
      .click();
    await expect(
      viewer!.getByRole("button", { name: "Join private room", exact: true }),
    ).toBeVisible({ timeout: 12_000 });
    for (const suffix of [
      "session",
      `previews/${publisherId}/camera/${"a".repeat(22)}`,
    ])
      expect(
        (await viewer!.request.get(`/api/rooms/${room.id}/${suffix}`)).status(),
      ).toBe(401);
    expect(
      (
        await viewer!.request.post(`/api/rooms/${room.id}/media-token`, {
          headers: { Origin: baseURL! },
        })
      ).status(),
    ).toBe(401);
    expect(savedToken.length > 0).toBe(true);
    expect(
      (
        await request.get(
          `${mediaUrl.replace(/^ws/, "http")}/rtc/v1/validate`,
          { params: { access_token: savedToken } },
        )
      ).status(),
    ).toBe(401);
    const replay = await new Promise<number>((resolve) => {
      const socket = new WebSocket(
        `${mediaUrl}/rtc/v1?access_token=${encodeURIComponent(savedToken)}`,
        { handshakeTimeout: 5000 },
      );
      socket.on("error", () => resolve(0));
      socket.on("unexpected-response", (_request, response) => {
        response.resume();
        resolve(response.statusCode ?? 0);
        socket.terminate();
      });
      socket.on("open", () => {
        socket.close();
        resolve(101);
      });
    });
    expect(replay).toBe(401);
    // Ending one session leaves the publisher connected and sharing.
    await expect(
      publisher!.getByRole("button", { name: "Stop camera", exact: true }),
    ).toBeVisible();
    expect(watcher !== publisherId).toBe(true);
    await operator!
      .getByLabel("Also end every current session, including yours")
      .check();
    await operator!
      .getByRole("button", { name: "Disable invitation", exact: true })
      .click();
    await expect(
      publisher!.getByRole("button", {
        name: "Join private room",
        exact: true,
      }),
    ).toBeVisible({ timeout: 12_000 });
    expect(
      (
        await request.post(`/api/rooms/${room.id}/join`, {
          headers: { Origin: baseURL! },
          data: {
            invitation: new URL(next).hash.slice(8),
            displayName: "Disabled invite",
          },
        })
      ).status(),
    ).toBe(404);
    await expect(
      operator!.getByText("No current sessions.", { exact: true }),
    ).toBeVisible();
    await operator!.getByRole("button", { name: "Lock", exact: true }).click();
    await expect(
      operator!.getByLabel("New room invitation", { exact: true }),
    ).toHaveCount(0);
    expect(
      await operator!
        .getByLabel("Server access key", { exact: true })
        .inputValue(),
    ).toBe("");
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
  }
});

test("room management fits the viewport and Lock clears operator authority", async ({
  page,
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Management layout");
  const { room } = await created.json();
  await page.goto(`/manage/${room.id}`);
  await page
    .getByLabel("Server access key", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Load access", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Current sessions", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth + 1,
    ),
  ).toBe(true);
  await page.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Current sessions", exact: true }),
  ).toHaveCount(0);
  expect(
    await page.getByLabel("Server access key", { exact: true }).inputValue(),
  ).toBe("");
});
