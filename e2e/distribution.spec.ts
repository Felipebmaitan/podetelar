import { createHash } from "node:crypto";
import { expect, test } from "@playwright/test";
import { createTestRoom, openTestRoom } from "../scripts/test-room.js";

test("room members download the verified portable EXE through the room UI", async ({
  page,
  request,
  baseURL,
}, info) => {
  test.skip(
    info.project.name === "mobile" ||
      process.env.FREEDOMSTREAM_PUBLISHER_DOWNLOAD !== "1",
    "Opt-in check for a server with an installed portable publisher.",
  );
  const created = await createTestRoom(
    request,
    baseURL!,
    "Distribution qualification",
  );
  expect(created.status()).toBe(201);
  const { room, invitationUrl } = await created.json();
  const path = `/api/rooms/${room.id}/publisher`;
  const available = await request.get(path);
  expect(available.status()).toBe(200);
  expect((await page.request.get(`${path}/windows.exe`)).status()).toBe(401);
  await openTestRoom(page, invitationUrl);
  await page.getByLabel("Your name", { exact: true }).fill("Windows friend");
  await page
    .getByRole("button", { name: "Join private room", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Leave room", exact: true }),
  ).toBeVisible();
  const metadata = await page.request.get(path);
  expect(metadata.status()).toBe(200);
  const release = await metadata.json();
  const download = page.waitForEvent("download");
  await page
    .getByRole("link", {
      name: `Download portable Windows app ${release.version}`,
      exact: true,
    })
    .click();
  const stream = await (await download).createReadStream();
  const hash = createHash("sha256");
  let size = 0;
  for await (const chunk of stream) {
    hash.update(chunk);
    size += chunk.length;
  }
  expect(hash.digest("hex")).toBe(release.sha256);
  expect(size).toBe(release.size);
});
