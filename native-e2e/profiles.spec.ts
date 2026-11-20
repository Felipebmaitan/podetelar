import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

test("native profile changes update the connected SFU name and animated avatars persist across sessions", async ({
  request,
  baseURL,
}) => {
  const created = await createTestRoom(
    request,
    baseURL!,
    "Guest profile acceptance",
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  for (const upload of [true, false]) {
    const newName = upload ? "Native renamed guest" : "Native returning guest";
    const result = (await nativeRequest("--profile-fixture", {
      invitationUrl,
      displayName: "Initial guest",
      newName,
      upload,
    }).result) as {
      displayName: string;
      mediaName: string;
      decodedFrames: number;
      animated: boolean;
    };
    expect(result.displayName).toBe(newName);
    expect(result.mediaName).toBe(newName);
    expect(result.decodedFrames).toBe(2);
    expect(result.animated).toBe(true);
  }
});
