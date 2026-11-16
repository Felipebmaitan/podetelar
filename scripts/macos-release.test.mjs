import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { macosRelease, releaseVersion } from "./macos-release.mjs";
test("macOS release feed verifies full package bytes and keeps the platform channel separate", () => {
  const root = mkdtempSync(join(tmpdir(), "podetelar-macos-"));
  try {
    const bytes = Buffer.from("synthetic package bytes");
    const full = {
      PackageId: "PodeTelarMac",
      Version: "0.1.2-preview.1",
      Type: "Full",
      FileName: "PodeTelarMac-0.1.2-preview.1-full.nupkg",
      Size: bytes.length,
      SHA256: createHash("sha256").update(bytes).digest("hex"),
    };
    writeFileSync(join(root, full.FileName), bytes);
    const feed = (asset) =>
      writeFileSync(
        join(root, "releases.osx-arm64.json"),
        JSON.stringify({ Assets: [asset] }),
      );
    feed(full);
    assert.equal(
      macosRelease(root, full.PackageId, full.Version).FileName,
      full.FileName,
    );
    assert.throws(() => macosRelease(root, "PodeTelar", full.Version));
    assert.throws(() => macosRelease(root, full.PackageId, "0.1.3-preview.1"));
    for (const change of [
      { FileName: "../escape-full.nupkg" },
      { FileName: "/tmp/escape-full.nupkg" },
      { FileName: "..\\escape-full.nupkg" },
      { Type: "Delta" },
      { Size: 0 },
      { Size: 2 ** 31 },
      { SHA256: "invalid" },
    ]) {
      feed({ ...full, ...change });
      assert.throws(() => macosRelease(root, full.PackageId, full.Version));
    }
    feed(full);
    writeFileSync(
      join(root, full.FileName),
      Buffer.from("corrupted package bytes!"),
    );
    assert.throws(() => macosRelease(root, full.PackageId, full.Version));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
test("release tags cannot attach an older app version to a newer GitHub release", () => {
  assert.equal(releaseVersion("0.1.2", "v0.1.2"), "0.1.2");
  assert.throws(() => releaseVersion("0.1.2", "v0.1.3"));
  assert.throws(() => releaseVersion("0.1.2/../bad"));
});
