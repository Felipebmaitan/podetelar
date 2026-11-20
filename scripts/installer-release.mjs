import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

function asset(path, filename) {
  const bytes = readFileSync(path);
  return {
    filename,
    size: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

// Produce the complete public asset set together: a Setup alone cannot serve
// the built-in updater. Keep portable EXEs/ZIPs out of this directory.
export function installerRelease(
  output,
  config,
  portable,
  internalVersion,
  feed,
) {
  const feedName = "releases.preview.json";
  const metadata = JSON.parse(readFileSync(join(output, feedName), "utf8"));
  const entries = metadata.Assets;
  if (!Array.isArray(entries) || entries.length !== 1)
    throw new Error("Expected exactly one full update package.");
  const entry = entries[0];
  if (
    entry.PackageId !== config.packageId ||
    entry.Version !== internalVersion ||
    entry.Type !== "Full" ||
    !/^[A-Za-z0-9][A-Za-z0-9._-]*-full\.nupkg$/.test(entry.FileName ?? "") ||
    entry.FileName.includes("..") ||
    !/^[a-f0-9]{64}$/i.test(entry.SHA256 ?? "") ||
    !Number.isSafeInteger(entry.Size) ||
    entry.Size <= 0 ||
    entry.Size > 1024 ** 3
  )
    throw new Error("Invalid full update package metadata.");
  const packageAsset = asset(join(output, entry.FileName), entry.FileName);
  if (
    packageAsset.sha256 !== entry.SHA256.toLowerCase() ||
    packageAsset.size !== entry.Size
  )
    throw new Error("Update package integrity does not match its feed.");
  const setup = `${config.packageId}-preview-Setup.exe`;
  const setupName = `${config.packageId}-v${portable.version}-Setup.exe`;
  const setupAsset = asset(join(output, setup), setupName);
  const feedAsset = asset(join(output, feedName), feedName);
  const destination = join(output, "public");
  mkdirSync(destination); // Never overwrite a previously prepared release.
  for (const [source, name] of [
    [setup, setupName],
    [entry.FileName, entry.FileName],
    [feedName, feedName],
  ])
    copyFileSync(join(output, source), join(destination, name));
  const manifestName = "installer-manifest.json";
  const manifestPath = join(destination, manifestName);
  const assets = [setupAsset, packageAsset, feedAsset];
  writeFileSync(
    manifestPath,
    JSON.stringify(
      {
        name: config.title,
        version: portable.version,
        internalVersion,
        builtFromCommit: portable.commit,
        platform: "windows-x64",
        packageId: config.packageId,
        signed: false,
        updateFeed: feed,
        updateChannel: "preview",
        embeddedPortable: { ...portable, publiclyDistributed: false },
        assets,
      },
      null,
      2,
    ) + "\n",
  );
  writeFileSync(
    join(destination, "SHA256SUMS"),
    [...assets, asset(manifestPath, manifestName)]
      .map((value) => `${value.sha256}  ${value.filename}\n`)
      .join(""),
  );
  return destination;
}
