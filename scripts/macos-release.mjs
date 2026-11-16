import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
export function macosRelease(directory, packageId, version) {
  const metadata = JSON.parse(
    readFileSync(join(directory, "releases.osx-arm64.json")),
  );
  if (metadata.Assets?.length !== 1)
    throw new Error("Expected one macOS full update.");
  const full = metadata.Assets[0];
  if (
    full.PackageId !== packageId ||
    full.Version !== version ||
    full.Type !== "Full" ||
    !/^[A-Za-z0-9][\w.-]*-full\.nupkg$/.test(full.FileName ?? "") ||
    full.FileName.includes("..") ||
    !/^[a-f0-9]{64}$/i.test(full.SHA256 ?? "") ||
    !Number.isSafeInteger(full.Size) ||
    full.Size <= 0 ||
    full.Size > 1024 ** 3
  )
    throw new Error("Invalid macOS update metadata.");
  const bytes = readFileSync(join(directory, full.FileName));
  if (
    createHash("sha256").update(bytes).digest("hex") !==
      full.SHA256.toLowerCase() ||
    bytes.length !== full.Size
  )
    throw new Error("macOS update integrity mismatch.");
  return full;
}
export function releaseVersion(version, tag) {
  if (!/^\d+\.\d+\.\d+$/.test(version) || (tag && tag !== `v${version}`))
    throw new Error("Release tag must match the committed app version.");
  return version;
}
