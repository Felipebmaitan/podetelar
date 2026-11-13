import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdtempSync,
  mkdirSync,
  realpathSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { test } from "node:test";

function removeFixture(root) {
  const resolved = realpathSync(root);
  assert.equal(dirname(resolved), realpathSync(tmpdir()));
  assert.ok(basename(resolved).startsWith("FreedomStream native "));
  rmSync(resolved, { recursive: true, force: true });
}

for (const state of ["archive", "source"]) {
  test(`Native SDK setup rejects tampered ${state} before reuse`, () => {
    const root = mkdtempSync(join(tmpdir(), "FreedomStream native SDK "));
    try {
      mkdirSync(join(root, "scripts"));
      mkdirSync(join(root, "deploy"));
      mkdirSync(join(root, ".local/downloads"), { recursive: true });
      copyFileSync(
        new URL("./setup-native-sdk.mjs", import.meta.url),
        join(root, "scripts/setup-native-sdk.mjs"),
      );
      writeFileSync(
        join(root, "deploy/native-sdk.json"),
        JSON.stringify({
          version: "fixture",
          url: "https://example.invalid/must-not-download",
          sha256: "0".repeat(64),
          treeSha256: "0".repeat(64),
        }),
      );
      if (state === "archive")
        writeFileSync(
          join(root, ".local/downloads/livekit-fixture.crate"),
          "Invalid cached package",
        );
      else {
        mkdirSync(join(root, ".local/native-sdk/livekit"), { recursive: true });
        writeFileSync(
          join(root, ".local/native-sdk/livekit/tampered.rs"),
          "Changed source",
        );
      }
      const result = spawnSync(
        process.execPath,
        [join(root, "scripts/setup-native-sdk.mjs")],
        { encoding: "utf8", windowsHide: true, timeout: 15_000 },
      );
      assert.ifError(result.error);
      assert.notEqual(result.status, 0);
      assert.match(
        result.stderr,
        state === "archive"
          ? /archive checksum did not match/
          : /generated native SDK differs/,
      );
    } finally {
      assert.ok(root.startsWith(join(tmpdir(), "FreedomStream native SDK ")));
      removeFixture(root);
    }
  });
}

const decoderHeaders = [
  "freedomstream_decoder.h",
  "deferred_i420.h",
  "bounded_readback.h",
  "native_output_guard.h",
  "native_output_mixer.h",
];

for (const state of ["archive", "source"]) {
  test(`Native audio adapter rejects tampered ${state} before reuse`, () => {
    const root = mkdtempSync(join(tmpdir(), "FreedomStream native audio "));
    try {
      mkdirSync(join(root, "scripts"));
      mkdirSync(join(root, "deploy"));
      copyFileSync(
        new URL("./setup-native-audio.mjs", import.meta.url),
        join(root, "scripts/setup-native-audio.mjs"),
      );
      copyFileSync(
        new URL("../deploy/native-audio.json", import.meta.url),
        join(root, "deploy/native-audio.json"),
      );
      if (state === "archive") {
        mkdirSync(join(root, ".local/downloads"), { recursive: true });
        writeFileSync(
          join(root, ".local/downloads/libwebrtc-0.4.0.crate"),
          "Changed archive",
        );
      } else {
        mkdirSync(join(root, ".local/native-sdk/libwebrtc"), {
          recursive: true,
        });
        writeFileSync(
          join(root, ".local/native-sdk/libwebrtc/tampered.rs"),
          "Changed source",
        );
      }
      const result = spawnSync(
        process.execPath,
        [join(root, "scripts/setup-native-audio.mjs")],
        { encoding: "utf8", windowsHide: true, timeout: 15_000 },
      );
      assert.ifError(result.error);
      assert.notEqual(result.status, 0);
      assert.match(
        result.stderr,
        state === "archive"
          ? /Audio SDK archive checksum mismatch/
          : /Generated audio SDK differs from its pin/,
      );
    } finally {
      removeFixture(root);
    }
  });
}
for (const tampered of decoderHeaders) {
  test(`Native decoder setup rejects changed ${tampered} before SDK reuse`, () => {
    const root = mkdtempSync(join(tmpdir(), "FreedomStream native decoder "));
    try {
      mkdirSync(join(root, "scripts"));
      mkdirSync(join(root, "deploy"));
      mkdirSync(join(root, "apps/publisher/decoder"), { recursive: true });
      mkdirSync(join(root, ".local/native-sdk/webrtc-sys"), {
        recursive: true,
      });
      copyFileSync(
        new URL("./setup-native-decoder.mjs", import.meta.url),
        join(root, "scripts/setup-native-decoder.mjs"),
      );
      copyFileSync(
        new URL("./patch-native-output.mjs", import.meta.url),
        join(root, "scripts/patch-native-output.mjs"),
      );
      copyFileSync(
        new URL("../deploy/native-decoder.json", import.meta.url),
        join(root, "deploy/native-decoder.json"),
      );
      for (const name of decoderHeaders) {
        const bytes = readFileSync(
          new URL(`../apps/publisher/decoder/${name}`, import.meta.url),
          "utf8",
        );
        writeFileSync(
          join(root, "apps/publisher/decoder", name),
          bytes + (name === tampered ? "\n// changed\n" : ""),
        );
      }
      const result = spawnSync(
        process.execPath,
        [join(root, "scripts/setup-native-decoder.mjs")],
        { encoding: "utf8", windowsHide: true, timeout: 15_000 },
      );
      assert.ifError(result.error);
      assert.notEqual(result.status, 0);
      assert.match(
        result.stderr,
        tampered === "freedomstream_decoder.h"
          ? /Decoder adapter differs from its pin/
          : tampered === "deferred_i420.h"
            ? /Deferred I420 buffer differs from its pin/
            : tampered === "bounded_readback.h"
              ? /Bounded readback differs from its pin/
              : tampered === "native_output_guard.h"
                ? /Native output guard differs from its pin/
                : /Native output mixer differs from its pin/,
      );
    } finally {
      removeFixture(root);
    }
  });
}

test(
  "Windows setup verifies a cached installer through an inherited module path",
  {
    skip: process.platform !== "win32",
  },
  () => {
    const root = mkdtempSync(join(tmpdir(), "FreedomStream native setup "));
    try {
      mkdirSync(join(root, "scripts"));
      mkdirSync(join(root, "deploy"));
      mkdirSync(join(root, ".local/downloads"), { recursive: true });
      copyFileSync(
        new URL("./setup-native.ps1", import.meta.url),
        join(root, "scripts/setup-native.ps1"),
      );
      writeFileSync(
        join(root, "deploy/native-runtime.json"),
        JSON.stringify({
          version: "fixture",
          url: "https://example.invalid/must-not-download",
          sha256: "0".repeat(64),
        }),
      );
      writeFileSync(
        join(root, ".local/downloads/gstreamer-fixture.exe"),
        "Invalid cached installer",
      );
      // Reproduce npm's intermediate-process launch. Direct pwsh -> powershell
      // rewrites the module path, whereas Node -> powershell preserves it.
      const result = spawnSync(
        "powershell.exe",
        [
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-File",
          join(root, "scripts/setup-native.ps1"),
        ],
        {
          windowsHide: true,
          encoding: "utf8",
          timeout: 15_000,
          env: {
            ...process.env,
            PSModulePath: join(
              process.env.ProgramFiles,
              "PowerShell/7/Modules",
            ),
          },
        },
      );
      assert.ifError(result.error);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /Native runtime checksum did not match/);
      assert.doesNotMatch(result.stderr, /Get-FileHash.*not recognized/s);
    } finally {
      // root is generated by mkdtemp, never a caller-supplied deletion target.
      assert.ok(root.startsWith(join(tmpdir(), "FreedomStream native setup ")));
      removeFixture(root);
    }
  },
);
