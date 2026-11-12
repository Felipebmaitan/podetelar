import { copyFileSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";

export function installerNotices(metadata, stage) {
  const included = new Set(metadata.resolve.nodes.map((node) => node.id));
  const index = [];
  for (const pkg of metadata.packages.filter((pkg) => included.has(pkg.id))) {
    if (pkg.name === "freedomstream-installed") continue;
    // The pinned Rust crate omits its upstream LICENSE. The caller ships the
    // matching repository license separately at the package root.
    if (pkg.name === "velopack" && pkg.version === "1.2.161") {
      index.push("velopack 1.2.161: MIT; see Velopack-LICENSE.txt");
      continue;
    }
    const directory = dirname(pkg.manifest_path);
    const files = readdirSync(directory, {
      recursive: true,
      withFileTypes: true,
    }).filter(
      (entry) =>
        entry.isFile() &&
        /^(?:licen[cs]e|copying|notice)(?:[._-]|$)/i.test(entry.name),
    );
    if (!files.length)
      throw new Error(`Missing installed bootstrap notices: ${pkg.name}`);
    for (const entry of files) {
      const source = join(entry.parentPath, entry.name);
      const target = join(
        stage,
        "bootstrap-notices",
        `${pkg.name}-${pkg.version}`,
        relative(directory, source),
      );
      mkdirSync(dirname(target), { recursive: true });
      copyFileSync(source, target);
    }
    index.push(
      `${pkg.name} ${pkg.version}: ${pkg.license ?? "see included license"}`,
    );
  }
  writeFileSync(
    join(stage, "bootstrap-dependencies.txt"),
    index.join("\n") + "\n",
  );
}
