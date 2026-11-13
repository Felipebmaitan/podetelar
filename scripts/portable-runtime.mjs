import { execFileSync as runFile } from "node:child_process";
const execFileSync = (file, args, options = {}) =>
  runFile(file, args, { ...options, windowsHide: true });
import { createHash } from "node:crypto";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";

const hash = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

/** Build-only downloads: every source archive also accompanies the offline EXE. */
export async function preparePortableRuntime(root, output, notices) {
  const manifest = JSON.parse(
    readFileSync(join(root, "deploy/portable-runtime.json")),
  );
  const runtimeOverride =
    process.env.FREEDOMSTREAM_PORTABLE_RUNTIME ??
    process.env.FREEDOMSTREAM_GSTREAMER;
  const runtime = runtimeOverride
    ? resolve(runtimeOverride)
    : join(root, ".local/gstreamer");
  mkdirSync(join(output, "bin"), { recursive: true });
  copyFileSync(
    join(
      resolve(root, process.env.CARGO_TARGET_DIR ?? "target"),
      "release/freedomstream-publisher.exe",
    ),
    join(output, "bin/freedomstream-publisher.exe"),
  );
  const inventory = readFileSync(join(runtime, "share/versions.txt"), "utf8");
  if (
    !inventory.includes(`gstreamer-1.0 1.28.8+git~${manifest.gstreamerCommit}`)
  )
    throw new Error("The qualified GStreamer source revision does not match.");
  for (const file of manifest.files) {
    const source = join(runtime, file.path);
    if (hash(source) !== file.sha256)
      throw new Error(`Unqualified runtime file: ${file.path}`);
    const destination = join(output, file.path);
    mkdirSync(resolve(destination, ".."), { recursive: true });
    copyFileSync(source, destination);
  }
  for (const name of manifest.licenseDirectories)
    cpSync(
      join(runtime, "share/licenses", name),
      join(notices, "gstreamer", name),
      { recursive: true },
    );
  copyFileSync(
    join(runtime, "share/versions.txt"),
    join(notices, "gstreamer-versions.txt"),
  );
  copyFileSync(
    join(root, "deploy/portable-runtime.json"),
    join(notices, "runtime-inputs.json"),
  );
  const microsoft = join(notices, "microsoft");
  mkdirSync(microsoft, { recursive: true });
  for (const document of manifest.licenseDocuments) {
    const path = join(root, document.file);
    if (hash(path) !== document.sha256)
      throw new Error("Microsoft license text checksum mismatch.");
    copyFileSync(path, join(microsoft, basename(path)));
  }
  const sourceCache = join(root, ".local/downloads/portable-sources");
  const sources = join(notices, "sources");
  mkdirSync(sourceCache, { recursive: true });
  mkdirSync(sources, { recursive: true });
  // Keep downloads bounded; a failed/mismatched cache entry is never used.
  for (const source of manifest.sources) {
    const path = join(sourceCache, source.filename);
    if (!existsSync(path)) {
      const response = await fetch(source.url, {
        signal: AbortSignal.timeout(120_000),
      });
      if (!response.ok)
        throw new Error(
          `Source archive HTTP ${response.status}: ${source.filename}`,
        );
      const bytes = Buffer.from(await response.arrayBuffer());
      if (
        bytes.length > 128 * 1024 * 1024 ||
        createHash("sha256").update(bytes).digest("hex") !== source.sha256
      )
        throw new Error(`Source archive checksum mismatch: ${source.filename}`);
      writeFileSync(path, bytes);
    }
    if (hash(path) !== source.sha256)
      throw new Error(`Source archive checksum mismatch: ${source.filename}`);
    copyFileSync(path, join(sources, source.filename));
    // Make upstream copyright and license texts readable without opening an archive.
    const regular = new Set(
      execFileSync("tar", ["-tvf", path], {
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
      })
        .split(/\r?\n/)
        .filter((line) => line.startsWith("-"))
        .map((line) => line.trim().split(/\s+/).at(-1)),
    );
    const entries = execFileSync("tar", ["-tf", path], {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    })
      .split(/\r?\n/)
      .filter(
        (entry) =>
          regular.has(entry) &&
          (/^(?:copying|licen[cs]e|notice)(?:[._-]|$)/i.test(basename(entry)) ||
            entry.includes("/LICENSES/")),
      );
    if (
      entries.some(
        (entry) =>
          entry.startsWith("/") ||
          entry.includes("\\") ||
          entry.split("/").includes(".."),
      )
    )
      throw new Error("Unsafe source archive notice path.");
    const destination = join(
      notices,
      "source-licenses",
      source.filename.replace(/\.tar\..+$/, ""),
    );
    mkdirSync(destination, { recursive: true });
    const list = join(sourceCache, `${source.filename}.notices.txt`);
    writeFileSync(list, entries.join("\n") + "\n");
    if (entries.length)
      execFileSync("tar", [
        "-xf",
        path,
        "-C",
        destination,
        "--strip-components",
        "1",
        "-T",
        list,
      ]);
  }
  copyFileSync(
    join(root, "deploy/windows/portable-sources.md"),
    join(sources, "README.md"),
  );
  copyFileSync(
    join(root, "deploy/windows/portable-runtime.md"),
    join(notices, "RUNTIME.md"),
  );

  const vswhere = join(
    process.env["ProgramFiles(x86)"],
    "Microsoft Visual Studio/Installer/vswhere.exe",
  );
  const visualStudio = execFileSync(
    vswhere,
    [
      "-latest",
      "-products",
      "*",
      "-requires",
      "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
      "-property",
      "installationPath",
    ],
    { encoding: "utf8" },
  ).trim();
  const redist = join(visualStudio, "VC/Redist/MSVC");
  const versions = readdirSync(redist)
    .filter((name) => /^14\.\d+\.\d+$/.test(name))
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  const platform = join(redist, versions.at(-1), "x64");
  const crtName = readdirSync(platform).find((name) =>
    /^Microsoft\.VC\d+\.CRT$/.test(name),
  );
  if (!crtName)
    throw new Error(
      "No redistributable x64 release CRT found in Visual Studio.",
    );
  const crt = join(platform, crtName);
  mkdirSync(join(output, "crt"));
  const crtFiles = readdirSync(crt).filter((name) => name.endsWith(".dll"));
  if (
    !["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"].every((name) =>
      crtFiles.includes(name),
    )
  )
    throw new Error("The app-local Visual C++ runtime is incomplete.");
  const crtHashes = {};
  for (const name of crtFiles) {
    crtHashes[name] = hash(join(crt, name));
    copyFileSync(join(crt, name), join(output, "crt", name));
  }
  writeFileSync(
    join(notices, "Microsoft-CRT.json"),
    JSON.stringify(
      {
        copyright: "Copyright Microsoft Corporation. All rights reserved.",
        license:
          "Microsoft Visual Studio license terms, Distributable Code. Unmodified app-local release CRT files; not MIT licensed.",
        redistribution:
          "https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution#visual-c-runtime-files",
        deployment:
          "https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files",
        directoryVersion: versions.at(-1),
        files: crtHashes,
      },
      null,
      2,
    ) + "\n",
  );

  const tools = join(visualStudio, "VC/Tools/MSVC");
  const toolVersion = readdirSync(tools)
    .filter((name) => /^14\.\d+\.\d+$/.test(name))
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
    .at(-1);
  const dumpbin = join(tools, toolVersion, "bin/Hostx64/x64/dumpbin.exe");
  const files = readdirSync(output, {
    recursive: true,
    withFileTypes: true,
  }).filter((entry) => entry.isFile() && /\.(?:exe|dll)$/i.test(entry.name));
  const shipped = new Set(files.map((entry) => entry.name.toLowerCase()));
  for (const entry of files) {
    const imports = execFileSync(
      dumpbin,
      ["/DEPENDENTS", join(entry.parentPath, entry.name)],
      { encoding: "utf8" },
    );
    for (const match of imports.matchAll(/^\s+([a-z0-9_.+-]+\.dll)\s*$/gim)) {
      const name = match[1].toLowerCase();
      if (
        !shipped.has(name) &&
        !manifest.windowsImports.includes(name) &&
        !/^(?:api|ext)-ms-win-/.test(name)
      )
        throw new Error(`Unqualified DLL import ${name} in ${entry.name}`);
    }
  }
  const env = {
    ...process.env,
    PATH: `${join(output, "bin")};${join(output, "crt")};${process.env.WINDIR}/System32;${process.env.WINDIR}`,
    GST_PLUGIN_PATH_1_0: "",
    GST_PLUGIN_SYSTEM_PATH_1_0: join(output, "lib/gstreamer-1.0"),
    GST_PLUGIN_SCANNER_1_0: join(
      output,
      "libexec/gstreamer-1.0/gst-plugin-scanner.exe",
    ),
    GST_REGISTRY_1_0: join(sourceCache, "qualification-registry.bin"),
  };
  // Window/WGC capture uses this cadence element even though the synthetic
  // encoder probe does not. Reject incomplete bundles before creating an EXE.
  execFileSync(
    join(output, "bin/gst-inspect-1.0.exe"),
    ["--exists", "imagefreeze"],
    { env },
  );
  for (const file of manifest.files.filter((file) =>
    file.path.startsWith("lib/gstreamer-1.0/"),
  )) {
    const info = execFileSync(
      join(output, "bin/gst-inspect-1.0.exe"),
      [join(output, file.path)],
      { env, encoding: "utf8" },
    );
    if (!/^\s+License\s+LGPL\s*$/m.test(info))
      throw new Error(`Unqualified plugin license: ${file.path}`);
  }
  return { dumpbin };
}
