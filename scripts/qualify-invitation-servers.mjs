// Real HTTP/native admission against independent API stores; no SFU/media claim.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { buildApp } from "../apps/server/dist/app.js";
import { loadConfig } from "../apps/server/dist/config.js";

const root = resolve(import.meta.dirname, "..");
const directory = mkdtempSync(join(root, ".local/invitation-servers-"));
const apps = [];
try {
  const fixtures = [];
  for (const [index, port] of [49121, 49122].entries()) {
    const origin = `http://127.0.0.1:${port}`;
    const config = loadConfig({
      PUBLIC_URL: origin,
      PORT: String(port),
      ADMIN_TOKEN: "local-qualification-only-administrator",
      LIVEKIT_API_KEY: "local-test-key",
      LIVEKIT_API_SECRET: "local-qualification-only-media-secret",
      DATABASE_PATH: join(directory, `backend-${index}.db`),
    });
    const app = await buildApp(config, {
      usagePolling: false,
      mediaDirectory: {
        async members() {
          return [];
        },
        async participant() {
          return null;
        },
        async attribute() {},
        async remove() {},
      },
    });
    apps.push(app);
    await app.listen({ host: "127.0.0.1", port });
    const name = `Independent backend ${index + 1}`;
    const created = await fetch(`${origin}/api/rooms`, {
      method: "POST",
      headers: {
        Origin: origin,
        Authorization: `Bearer ${config.adminToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ name, displayName: "Local fixture" }),
    });
    assert.equal(created.status, 201);
    const room = await created.json();
    const handoff = await fetch(
      `${origin}/api/rooms/${room.room.id}/app-link`,
      {
        method: "POST",
        headers: { Origin: origin, "Content-Type": "application/json" },
        body: JSON.stringify({
          invitation: new URL(room.invitationUrl).hash.slice(8),
        }),
      },
    );
    assert.equal(handoff.status, 200);
    fixtures.push({ link: (await handoff.json()).appUrl, origin, name });
  }
  const fixturePath = join(directory, "fixture.json");
  writeFileSync(fixturePath, JSON.stringify(fixtures));
  const runtime =
    process.env.FREEDOMSTREAM_GSTREAMER ?? join(root, ".local/gstreamer");
  await new Promise((accept, reject) => {
    const child = spawn(
      "cargo",
      [
        "test",
        "--locked",
        "--bin",
        "freedomstream-publisher",
        "admission::tests::invitation_links_select_two_live_backends_without_saved_configuration",
        "--",
        "--ignored",
        "--exact",
      ],
      {
        cwd: root,
        windowsHide: true,
        stdio: "inherit",
        env: {
          ...process.env,
          PATH: `${join(runtime, "bin")};${process.env.PATH}`,
          PKG_CONFIG_PATH: join(runtime, "lib/pkgconfig"),
          GSTREAMER_1_0_ROOT_MSVC_X86_64: runtime,
          FREEDOMSTREAM_MULTI_SERVER_FIXTURE: fixturePath,
        },
      },
    );
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0
        ? accept()
        : reject(new Error(`Native qualification exited ${code}`)),
    );
  });
  console.log(
    "Passed native handoff, admission, replay denial and session isolation on two independent HTTP backends.",
  );
} finally {
  for (const app of apps) await app.close();
  rmSync(directory, { recursive: true });
}
