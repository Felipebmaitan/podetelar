import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { randomBytes } from "node:crypto";
import { tmpdir } from "node:os";
import { join, resolve, delimiter } from "node:path";
import { spawn } from "node:child_process";
import { buildApp } from "../apps/server/src/app.js";
import { loadConfig } from "../apps/server/src/config.js";

const dir = mkdtempSync(join(tmpdir(), "freedomstream-direct-native-"));
const config = loadConfig({
  PUBLIC_URL: "http://localhost:1",
  DIRECT_URL: "http://127.0.0.1:1",
  DATABASE_PATH: join(dir, "rooms.db"),
  ADMIN_TOKEN: randomBytes(32).toString("hex"),
  LIVEKIT_API_KEY: "test-key",
  LIVEKIT_API_SECRET: randomBytes(32).toString("hex"),
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
try {
  config.directUrl = await app.listen({ host: "127.0.0.1", port: 0 });
  config.publicUrl = config.directUrl.replace("127.0.0.1", "localhost");
  const room = await app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: {
      origin: config.publicUrl,
      authorization: `Bearer ${config.adminToken}`,
    },
    payload: { name: "Native domains", displayName: "Fixture" },
  });
  const path = join(dir, "fixture.json");
  writeFileSync(
    path,
    JSON.stringify({
      invitation: room.json().invitationUrl,
      directUrl: config.directUrl,
    }),
    { mode: 0o600 },
  );
  const runtime = resolve(
    process.env.FREEDOMSTREAM_GSTREAMER ?? ".local/gstreamer",
  );
  const child = spawn(
    "cargo",
    [
      "test",
      "--locked",
      "-p",
      "freedomstream-publisher",
      "admission::tests::joins_discovered_direct_server",
      "--",
      "--ignored",
      "--exact",
    ],
    {
      windowsHide: true,
      stdio: "inherit",
      env: {
        ...process.env,
        FREEDOMSTREAM_DIRECT_FIXTURE: path,
        PATH: `${join(runtime, "bin")}${delimiter}${process.env.PATH ?? ""}`,
        PKG_CONFIG_PATH: join(runtime, "lib/pkgconfig"),
        GSTREAMER_1_0_ROOT_MSVC_X86_64: runtime,
      },
    },
  );
  process.exitCode = await new Promise<number>((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code) => resolve(code ?? 1));
  });
} finally {
  await app.close();
  rmSync(dir, { recursive: true });
}
