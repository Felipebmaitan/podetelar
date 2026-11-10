import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { chromium, request } from "@playwright/test";

// Opt-in, actual browser WebRTC transport test. This never targets production.
const origin = new URL(process.env.E2E_BASE_URL ?? "http://127.0.0.1:13310")
  .origin;
assert.ok(
  ["localhost", "127.0.0.1", "[::1]"].includes(new URL(origin).hostname),
);
assert.ok(
  process.env.ADMIN_TOKEN,
  "Supply a local test administrator credential.",
);
const headers = { Origin: origin };
const contexts = await Promise.all(
  Array.from({ length: 3 }, () =>
    request.newContext({ baseURL: origin, extraHTTPHeaders: headers }),
  ),
);
const [sender, receiver, admin] = contexts;
const browser = await chromium.launch({ headless: true });
const web = await browser.newContext({
  bypassCSP: true,
  viewport: { width: 1440, height: 1080 },
});
const sdk = resolve("node_modules/livekit-client/dist/livekit-client.umd.js");
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const json = async (response, status = 200) => {
  assert.equal(response.status(), status, "Local test API failed");
  return response.json();
};
const start = Date.now() - 60000;
try {
  const room = await json(
    await sender.post("/api/rooms", {
      headers: { Authorization: `Bearer ${process.env.ADMIN_TOKEN}` },
      data: { name: "Usage acceptance", displayName: "Usage sender" },
    }),
    201,
  );
  await json(
    await receiver.post(`/api/rooms/${room.room.id}/join`, {
      data: {
        invitation: new URL(room.invitationUrl).hash.slice(8),
        displayName: "Usage receiver",
      },
    }),
  );
  const access = await Promise.all(
    [sender, receiver].map(async (api) =>
      json(await api.post(`/api/rooms/${room.room.id}/native-media-token`)),
    ),
  );
  const pages = await Promise.all([web.newPage(), web.newPage()]);
  for (let i = 0; i < pages.length; i++) {
    await pages[i].goto(`${origin}/api/health`);
    await pages[i].addScriptTag({ path: sdk });
    await pages[i].evaluate(async (a) => {
      const sdk = window.LivekitClient;
      window.mediaRoom = new sdk.Room({
        adaptiveStream: false,
        dynacast: false,
      });
      await window.mediaRoom.connect(a.serverUrl, a.token, {
        autoSubscribe: false,
      });
    }, access[i]);
  }
  await pages[0].evaluate(async () => {
    const canvas = document.createElement("canvas");
    canvas.width = 640;
    canvas.height = 360;
    const ctx = canvas.getContext("2d");
    let frame = 0;
    window.paint = setInterval(() => {
      for (let x = 0; x < 640; x += 20) {
        ctx.fillStyle = `hsl(${(frame + x) % 360} 70% 50%)`;
        ctx.fillRect(x, 0, 20, 360);
      }
      ctx.fillStyle = "white";
      ctx.fillRect((frame * 5) % 600, 100, 40, 100);
      frame++;
    }, 33);
    window.capture = canvas.captureStream(30);
    const track = new window.LivekitClient.LocalVideoTrack(
      window.capture.getVideoTracks()[0],
    );
    await window.mediaRoom.localParticipant.publishTrack(track, {
      source: window.LivekitClient.Track.Source.ScreenShare,
      videoCodec: "h264",
      videoEncoding: { maxBitrate: 500000, maxFramerate: 30 },
    });
  });
  for (let i = 0; i < 20; i++) {
    const subscribed = await pages[1].evaluate(() => {
      for (const p of window.mediaRoom.remoteParticipants.values())
        for (const pub of p.trackPublications.values()) {
          pub.setSubscribed(true);
          if (pub.isSubscribed) return true;
        }
      return false;
    });
    if (subscribed) break;
    await wait(500);
  }
  await json(
    await admin.post("/api/admin/login", {
      data: { password: process.env.ADMIN_TOKEN },
    }),
  );
  const report = async () =>
    json(
      await admin.get(
        `/api/admin/usage?from=${start}&to=${Date.now() + 60000}`,
      ),
    );
  let measured;
  for (let i = 0; i < 18; i++) {
    measured = await report();
    if (measured.uploadedBytes > 10000 && measured.downloadedBytes > 10000)
      break;
    await wait(2500);
  }
  assert.ok(measured.available, "Collector must be live");
  assert.ok(
    measured.uploadedBytes > 10000,
    "Actual publication bytes required",
  );
  assert.ok(
    measured.downloadedBytes > 10000,
    "Actual subscription bytes required",
  );
  const sent = measured.users.find((u) => u.displayName === "Usage sender"),
    received = measured.users.find((u) => u.displayName === "Usage receiver");
  assert.ok(sent?.uploadedBytes > 10000);
  assert.ok(received?.downloadedBytes > 10000);
  assert.equal(
    measured.uploadedBytes,
    measured.users.reduce((n, u) => n + u.uploadedBytes, 0),
  );
  assert.equal(
    measured.downloadedBytes,
    measured.users.reduce((n, u) => n + u.downloadedBytes, 0),
  );
  assert.ok(sent.points.some((p) => p.uploadedBytes > 0));
  assert.ok(received.points.some((p) => p.downloadedBytes > 0));
  console.log(
    "Actual SFU publication/subscription counters and per-user graphs passed.",
  );
  const dashboard = await web.newPage();
  await dashboard.goto(origin);
  await dashboard
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN)
    .catch(() => {
      throw new Error("Local credential entry failed");
    });
  await dashboard.getByRole("button", { name: "Sign in", exact: true }).click();
  await dashboard
    .getByRole("heading", { name: "Usage sender", exact: true })
    .waitFor();
  await dashboard
    .getByRole("heading", { name: "Usage receiver", exact: true })
    .waitFor();
  mkdirSync(".local/admin-test", { recursive: true });
  await dashboard.screenshot({
    path: ".local/admin-test/dashboard-desktop.png",
    fullPage: true,
  });
  await dashboard.setViewportSize({ width: 390, height: 844 });
  assert.ok(
    await dashboard.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  );
  await dashboard.screenshot({
    path: ".local/admin-test/dashboard-mobile.png",
    fullPage: true,
  });
  await pages[1].evaluate(() => {
    for (const p of window.mediaRoom.remoteParticipants.values())
      for (const pub of p.trackPublications.values()) pub.setSubscribed(false);
  });
  await wait(15000);
  const stopped = await report();
  await wait(10000);
  const settled = await report();
  assert.ok(
    settled.uploadedBytes > stopped.uploadedBytes,
    "Publisher should still be sending",
  );
  assert.ok(
    settled.downloadedBytes - stopped.downloadedBytes < 1000,
    "Unsubscribed viewer must stop downstream media bytes",
  );
  console.log(
    "Explicit unsubscribe stopped download traffic while upload continued. Desktop/mobile actual-data UI passed.",
  );
} finally {
  for (const page of web.pages())
    await page
      .evaluate(() => {
        clearInterval(window.paint);
        window.capture?.getTracks().forEach((t) => t.stop());
        return window.mediaRoom?.disconnect();
      })
      .catch(() => {});
  await browser.close();
  for (const context of contexts) await context.dispose();
}
