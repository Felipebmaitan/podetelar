import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";

export function installerInput(directory, commit) {
  const manifest = JSON.parse(readFileSync(join(directory, "publisher.json")));
  if (
    !/^[a-f0-9]{40}$/.test(commit) ||
    manifest.commit !== commit ||
    !/^\d+\.\d+\.\d+$/.test(manifest.version) ||
    typeof manifest.filename !== "string" ||
    basename(manifest.filename) !== manifest.filename ||
    !/^freedomstream-windows-x64-[a-z0-9.-]+\.exe$/.test(manifest.filename)
  )
    throw new Error(
      "Installer input must be a portable release from this commit.",
    );
  const executable = join(directory, manifest.filename);
  const digest = createHash("sha256")
    .update(readFileSync(executable))
    .digest("hex");
  if (manifest.sha256 !== digest || manifest.size !== statSync(executable).size)
    throw new Error("Portable release integrity check failed.");
  return { executable, manifest };
}
