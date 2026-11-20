import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { installerUpdateFeed } from "./installer-feed.mjs";
import { installerRelease } from "./installer-release.mjs";

const config = JSON.parse(
  readFileSync(new URL("../deploy/windows/installer.json", import.meta.url)),
);
test("installer defaults to the public repository even with an empty CI variable", () => {
  for (const value of [undefined, "", "  "])
    assert.equal(
      installerUpdateFeed(config, value),
      "https://github.com/Felipebmaitan/podetelar",
    );
  assert.equal(
    installerUpdateFeed(config, " https://updates.example.test/app/ "),
    "https://updates.example.test/app",
  );
  for (const value of [
    "http://example.test",
    "C:\\feed",
    "https://name:secret@example.test",
    "https://example.test/?token=secret",
    "https://example.test/#secret",
    "https://example.test/?",
    "https://example.test/#",
    "https://github.com/owner/repo/releases",
    "https://github.com:444/owner/repo",
  ])
    assert.throws(() => installerUpdateFeed(config, value));
  assert.throws(() => installerUpdateFeed({}));
});

function fixture(run) {
  const directory = mkdtempSync(join(tmpdir(), "installer-release-"));
  const bytes = Buffer.from("full package fixture");
  const entry = {
    PackageId: config.packageId,
    Version: "0.1.2-preview.10",
    Type: "Full",
    FileName: "PodeTelar-0.1.2-preview.10-preview-full.nupkg",
    SHA256: createHash("sha256").update(bytes).digest("hex").toUpperCase(),
    Size: bytes.length,
  };
  const portable = {
    version: "0.1.2",
    commit: "a".repeat(40),
    filename: "private.exe",
    sha256: "b".repeat(64),
    size: 1,
  };
  const save = (entries = [entry]) =>
    writeFileSync(
      join(directory, "releases.preview.json"),
      JSON.stringify({ Assets: entries }),
    );
  const release = () =>
    installerRelease(
      directory,
      config,
      portable,
      entry.Version,
      config.updateFeed,
    );
  writeFileSync(join(directory, entry.FileName), bytes);
  writeFileSync(
    join(directory, "PodeTelar-preview-Setup.exe"),
    "Setup fixture",
  );
  save();
  try {
    run({ directory, entry, save, release });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("release staging includes Setup, full package, feed and verifiable checksums together", () =>
  fixture(({ entry, release }) => {
    const destination = release();
    assert.deepEqual(
      readdirSync(destination).sort(),
      [
        "PodeTelar-v0.1.2-Setup.exe",
        entry.FileName,
        "releases.preview.json",
        "installer-manifest.json",
        "SHA256SUMS",
      ].sort(),
    );
    for (const line of readFileSync(join(destination, "SHA256SUMS"), "utf8")
      .trim()
      .split("\n")) {
      const [digest, name] = line.split("  ");
      assert.equal(
        createHash("sha256")
          .update(readFileSync(join(destination, name)))
          .digest("hex"),
        digest,
      );
    }
    const manifest = JSON.parse(
      readFileSync(join(destination, "installer-manifest.json")),
    );
    assert.equal(manifest.updateFeed, config.updateFeed);
    assert.equal(manifest.embeddedPortable.publiclyDistributed, false);
    assert.throws(release); // Immutable staging.
  }));

test("release staging refuses missing packages, stale versions, deltas and unsafe feed entries", () =>
  fixture(({ directory, entry, save, release }) => {
    for (const entries of [
      [],
      [entry, entry],
      [{ ...entry, Type: "Delta" }],
      [{ ...entry, PackageId: "OtherApp" }],
      [{ ...entry, Version: "0.1.1-preview.1" }],
      [{ ...entry, FileName: "../escape-full.nupkg" }],
      [{ ...entry, FileName: "C:\\escape-full.nupkg" }],
      [{ ...entry, SHA256: "bad" }],
      [{ ...entry, Size: 1024 ** 3 + 1 }],
      [{ ...entry, FileName: "missing-full.nupkg" }],
    ]) {
      save(entries);
      assert.throws(release);
      assert(!readdirSync(directory).includes("public"));
    }
    save();
    writeFileSync(join(directory, entry.FileName), "corrupted");
    assert.throws(release, /integrity/);
    assert(!readdirSync(directory).includes("public"));
  }));
