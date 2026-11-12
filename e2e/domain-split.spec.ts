import { test, expect } from "@playwright/test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { randomBytes, createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildApp } from "../apps/server/src/app.js";
import { loadConfig } from "../apps/server/src/config.js";

test("a browser downloads authorized bytes directly without sharing cookies or secrets in HTTP URLs", async ({
  page,
}) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-browser-domains-"));
  const bytes = Buffer.from("local browser download fixture, never executed");
  const filename = "freedomstream-windows-x64-0.1.0-aaaaaaaaaaaa.exe";
  writeFileSync(join(dir, filename), bytes);
  writeFileSync(
    join(dir, "publisher.json"),
    JSON.stringify({
      filename,
      version: "0.1.0",
      commit: "a".repeat(40),
      sha256: createHash("sha256").update(bytes).digest("hex"),
      size: bytes.length,
    }),
  );
  const config = loadConfig({
    PUBLIC_URL: "http://localhost:1",
    DIRECT_URL: "http://127.0.0.1:1",
    DATABASE_PATH: join(dir, "rooms.db"),
    PUBLISHER_DIRECTORY: dir,
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
    // One real listener, two different cookie hosts. No production DNS or TLS claim.
    config.directUrl = await app.listen({ host: "127.0.0.1", port: 0 });
    config.publicUrl = config.directUrl.replace("127.0.0.1", "localhost");
    const created = await page.request.post(`${config.publicUrl}/api/rooms`, {
      headers: {
        origin: config.publicUrl,
        authorization: `Bearer ${config.adminToken}`,
      },
      data: { name: "Browser domains", displayName: "Downloader" },
    });
    expect(created.status()).toBe(201);
    const room = (await created.json()).room.id;
    const metadata = await page.request.get(
      `${config.publicUrl}/api/rooms/${room}/publisher`,
    );
    const urls: string[] = [];
    let directCookie: string | undefined;
    page.on("request", (request) => {
      urls.push(request.url());
      if (request.url() === `${config.directUrl}/api/publisher/download`)
        directCookie = request.headers().cookie ?? "";
    });
    await page.goto(config.publicUrl);
    // Exercise the real authenticated old download URL, redirect, fragment-only
    // handoff, SPA fragment erasure and direct POST attachment response.
    const downloaded = page.waitForEvent("download");
    await page.evaluate(
      (url) => {
        window.location.assign(url);
      },
      (await metadata.json()).downloadUrl,
    );
    const download = await downloaded;
    expect(download.suggestedFilename()).toBe(filename);
    expect(readFileSync((await download.path())!)).toEqual(bytes);
    expect(directCookie).toBe("");
    await expect(page).toHaveURL(`${config.directUrl}/download`);
    expect(
      urls.every(
        (url) =>
          !url.includes("ticket=") &&
          !url.includes("invite=") &&
          !url.includes("handoff="),
      ),
    ).toBe(true);
    expect(
      urls.some((url) => url === `${config.directUrl}/api/publisher/download`),
    ).toBe(true);
    expect(
      await page.evaluate(() =>
        JSON.stringify({ ...localStorage, ...sessionStorage }),
      ),
    ).toBe("{}");
  } finally {
    await app.close();
    rmSync(dir, { recursive: true });
  }
});
