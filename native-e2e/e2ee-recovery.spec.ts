import { expect, test } from "@playwright/test";
import { RoomServiceClient } from "livekit-server-sdk";
import { createInterface } from "node:readline";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

test("protected controller resumes selected playback with fresh keys, and honors room-session revocation", async ({
  request,
  baseURL,
}) => {
  test.skip(
    process.platform !== "win32",
    "Controller synthetic capture qualification runs on Windows.",
  );
  test.setTimeout(150_000);
  const created = await createTestRoom(request, baseURL!, "Protected recovery");
  expect(created.status()).toBe(201);
  const room = await created.json();
  const { invitationUrl } = (await nativeRequest("--protect-invitation", {
    invitationUrl: room.invitationUrl,
  }).result) as { invitationUrl: string };
  const media = new RoomServiceClient(
    process.env.PODETELAR_TEST_SFU_URL ?? "http://127.0.0.1:17880",
    process.env.LIVEKIT_API_KEY!,
    process.env.LIVEKIT_API_SECRET!,
  );
  const mediaRoom = `podetelar_${room.room.id}`;
  const controllerPeer = async () =>
    (await media.listParticipants(mediaRoom)).find(
      (participant) => participant.name === "Controller recovery check",
    );
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Remote recovery publisher",
    codec: "h264",
    seconds: 130,
    quality: {
      resolution: "p720",
      fps: 30,
      profile: "motion",
      bitrate: { mode: "manual", bps: 4_000_000 },
    },
  });
  const clients = [sender];
  try {
    await sender.reported;
    const actor = nativeRequest("--controller-recovery-fixture", {
      invitationUrl,
    });
    clients.push(actor);
    let resumed = false;
    createInterface({ input: actor.child.stdout }).on("line", (line) => {
      if (line === '{"phase":"resumed-watch"}') resumed = true;
    });
    expect(await actor.reported).toEqual({ phase: "publishing-and-watching" });
    const initial = await controllerPeer();
    expect(Boolean(initial?.tracks.length)).toBe(true);
    const sessionId = (attribute?: string): string | undefined => {
      if (!attribute) return;
      try {
        return JSON.parse(attribute).session;
      } catch {
        return;
      }
    };
    const originalSession = sessionId(
      initial!.attributes["freedomstream.e2ee.v1"],
    );
    expect(
      typeof originalSession === "string" && originalSession.length === 43,
    ).toBe(true);
    // A media-only loss keeps the original application admission valid.
    await media.removeParticipant(mediaRoom, initial!.identity);
    await expect.poll(() => resumed, { timeout: 45_000 }).toBe(true);
    const recovered = await controllerPeer();
    const renewedSession = sessionId(
      recovered?.attributes["freedomstream.e2ee.v1"],
    );
    expect(recovered?.identity === initial!.identity).toBe(true);
    expect(
      typeof renewedSession === "string" &&
        renewedSession.length === 43 &&
        renewedSession !== originalSession,
    ).toBe(true);
    expect(recovered?.tracks.length).toBe(0);

    // The invitation remains valid. The UI controller must respect the authority's
    // denial of the old admission instead of automatically rejoining via invitation.
    const removal = await request
      .delete(
        `/api/rooms/${room.room.id}/access/sessions/${initial!.identity}`,
        {
          headers: {
            Origin: baseURL!,
            Authorization: `Bearer ${process.env.ADMIN_TOKEN!}`,
          },
        },
      )
      .catch(() => {
        throw new Error("Could not revoke the local test session.");
      });
    expect([200, 202]).toContain(removal.status());
    expect(await actor.result).toEqual({ phase: "revoked-and-closed" });
    expect(Boolean(await controllerPeer())).toBe(false);
  } finally {
    for (const { child } of clients) if (child.exitCode === null) child.kill();
    await Promise.allSettled(clients.map((client) => client.result));
  }
});
