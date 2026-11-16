import { execFileSync } from "node:child_process";
import { mkdirSync, copyFileSync } from "node:fs";
import { resolve } from "node:path";
import { setupMacosRuntime, macosEnvironment } from "./macos-runtime.mjs";
const root = resolve(import.meta.dirname, "..");
const [action, ...args] = process.argv.slice(2);
const run = (file, args) =>
  execFileSync(file, args, {
    cwd: root,
    stdio: "inherit",
    env: macosEnvironment(),
  });
if (process.arch !== "arm64")
  throw new Error("The macOS build currently targets Apple Silicon (arm64).");
switch (action) {
  case "setup":
    await setupMacosRuntime();
    await import("./setup-native-sdk.mjs");
    await import("./setup-native-decoder.mjs");
    mkdirSync(resolve(root, ".local/macos"), { recursive: true });
    run("xcrun", [
      "swiftc",
      "-swift-version",
      "5",
      "-warnings-as-errors",
      "-O",
      "-target",
      "arm64-apple-macos14.0",
      "apps/macos/ScreenCapture.swift",
      "-o",
      ".local/macos/PodeTelar",
    ]);
    const developmentApp = resolve(root, ".local/macos/PodeTelar.app");
    mkdirSync(resolve(developmentApp, "Contents/MacOS"), { recursive: true });
    mkdirSync(resolve(developmentApp, "Contents/Resources"), {
      recursive: true,
    });
    copyFileSync(
      resolve(root, ".local/macos/PodeTelar"),
      resolve(developmentApp, "Contents/MacOS/PodeTelar"),
    );
    copyFileSync(
      resolve(root, "deploy/macos/Info.plist"),
      resolve(developmentApp, "Contents/Info.plist"),
    );
    run("codesign", ["--force", "--sign", "-", developmentApp]);
    break;
  case "check":
    run("cargo", ["fmt", "--all", "--check"]);
    run("cargo", [
      "clippy",
      "--locked",
      "-p",
      "podetelar-publisher",
      "--",
      "-D",
      "warnings",
    ]);
    break;
  case "test":
    run("cargo", ["test", "--locked", "-p", "podetelar-publisher", ...args]);
    break;
  case "build-fixtures":
    run("cargo", [
      "build",
      "--locked",
      "--release",
      "-p",
      "podetelar-publisher",
      "--features",
      "media-qualification",
    ]);
    mkdirSync(resolve(root, ".local/macos/fixtures"), { recursive: true });
    copyFileSync(
      resolve(root, "target/release/podetelar-publisher"),
      resolve(root, ".local/macos/fixtures/podetelar-publisher"),
    );
    break;
  case "probe":
  case "qualify":
  case "run":
    run("cargo", [
      "run",
      "--locked",
      "-p",
      "podetelar-publisher",
      "--",
      ...(action === "run" ? [] : [`--${action}`]),
      ...args,
    ]);
    break;
  default:
    throw new Error(
      "Use setup, check, test, build-fixtures, probe, qualify or run.",
    );
}
