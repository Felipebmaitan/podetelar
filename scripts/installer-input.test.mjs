import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { installerInput } from "./installer-input.mjs";

test("installer rejects a stale manifest, path escape and modified executable", () => {
  const directory = mkdtempSync(join(tmpdir(), "installer-input-"));
  const commit = "a".repeat(40);
  const data = Buffer.from("test binary");
  const manifest = {
    filename: "freedomstream-windows-x64-0.1.0-aaaaaaaaaaaa.exe",
    version: "0.1.0",
    commit,
    sha256: createHash("sha256").update(data).digest("hex"),
    size: data.length,
  };
  const save = (value) =>
    writeFileSync(join(directory, "publisher.json"), JSON.stringify(value));
  try {
    save(manifest);
    writeFileSync(join(directory, manifest.filename), data);
    assert.equal(installerInput(directory, commit).manifest.commit, commit);
    assert.throws(() => installerInput(directory, "b".repeat(40)));
    save({ ...manifest, filename: "../escape.exe" });
    assert.throws(() => installerInput(directory, commit));
    save(manifest);
    writeFileSync(join(directory, manifest.filename), "damaged");
    assert.throws(() => installerInput(directory, commit), /integrity/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
