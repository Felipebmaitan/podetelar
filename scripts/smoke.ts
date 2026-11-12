import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { RoomServiceClient } from "livekit-server-sdk";
import type { CreatedRoom, RoomSession } from "@freedomstream/contracts";

const origin = new URL(process.env.SMOKE_BASE_URL ?? "http://localhost:3000")
  .origin;
const admin = process.env.ADMIN_TOKEN;
if (!admin) throw new Error("Run npm run setup first.");
const headers = { Origin: origin, "Content-Type": "application/json" };
const health = await fetch(`${origin}/api/health`);
assert.equal(health.status, 200);
const home = await fetch(origin);
assert.equal(home.status, 200);
assert.match(await home.text(), /PodeTelar/);

const created = await fetch(`${origin}/api/rooms`, {
  method: "POST",
  headers: { ...headers, Authorization: `Bearer ${admin}` },
  body: JSON.stringify({
    name: `Smoke ${Date.now()}`,
    displayName: "Smoke owner",
  }),
});
assert.equal(created.status, 201);
const room = (await created.json()) as CreatedRoom;
const cookie = created.headers.get("set-cookie")!.split(";")[0]!;
assert.equal(
  (await fetch(`${origin}/api/rooms/${room.room.id}/session`)).status,
  401,
);
assert.equal(
  (
    await fetch(`${origin}/api/rooms/${room.room.id}/media-token`, {
      method: "POST",
      headers: { Origin: origin },
    })
  ).status,
  401,
);

const invitation = new URL(room.invitationUrl).hash.slice("#invite=".length);
const joined = await fetch(`${origin}/api/rooms/${room.room.id}/join`, {
  method: "POST",
  headers,
  body: JSON.stringify({ invitation, displayName: "Smoke friend" }),
});
assert.equal(joined.status, 200);
const friend = (await joined.json()) as RoomSession;
assert.equal(friend.room.id, room.room.id);

if (process.argv.includes("--restart-app")) {
  execFileSync("docker", ["compose", "restart", "app"], {
    stdio: "pipe",
    windowsHide: true,
  });
  let ready = false;
  for (let attempt = 0; attempt < 20 && !ready; attempt++) {
    ready = await fetch(`${origin}/api/health`)
      .then((response) => response.ok)
      .catch(() => false);
    if (!ready) await new Promise((resolve) => setTimeout(resolve, 500));
  }
  assert.ok(ready, "App did not recover after restart");
}
const resumed = await fetch(`${origin}/api/rooms/${room.room.id}/session`, {
  headers: { Cookie: cookie },
});
assert.equal(resumed.status, 200);
assert.deepEqual(
  ((await resumed.json()) as RoomSession).participant,
  room.participant,
);
const media = new RoomServiceClient(
  process.env.SMOKE_LIVEKIT_URL ?? "http://localhost:7880",
  process.env.LIVEKIT_API_KEY,
  process.env.LIVEKIT_API_SECRET,
);
await media.listRooms();
const left = await fetch(`${origin}/api/rooms/${room.room.id}/leave`, {
  method: "POST",
  headers: { Origin: origin, Cookie: cookie },
});
assert.equal(left.status, 204);
assert.equal(
  (
    await fetch(`${origin}/api/rooms/${room.room.id}/session`, {
      headers: { Cookie: cookie },
    })
  ).status,
  401,
);
console.log(
  "PASS: built web assets, room API admission, scoped sessions, persistence, logout and authenticated LiveKit access.",
);
