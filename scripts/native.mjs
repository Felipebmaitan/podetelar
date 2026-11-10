import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

const [action, ...args] = process.argv.slice(2);
const root = resolve(import.meta.dirname, "..");
if (action === "setup") {
  await import("./setup-native-sdk.mjs");
  await import("./setup-native-decoder.mjs");
}
try {
  execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-WindowStyle",
      "Hidden",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      resolve(
        root,
        action === "setup" ? "scripts/setup-native.ps1" : "scripts/native.ps1",
      ),
      ...(action === "setup" ? [] : [action]),
      ...args,
    ],
    { cwd: root, stdio: "inherit", windowsHide: true },
  );
} catch (error) {
  process.exitCode = Number.isInteger(error.status) ? error.status : 1;
}
