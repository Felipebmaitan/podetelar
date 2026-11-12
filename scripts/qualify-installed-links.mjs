// Windows hooks/registry/real process forwarding with an isolated test scheme.
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
const root = resolve(import.meta.dirname, "..");
const id = `${process.pid}-${Date.now()}`;
const output = join(root, ".local", `installed-links-${id}`);
const current = join(output, "app with spaces/current");
mkdirSync(current, { recursive: true });
const scheme = `Software\\Classes\\freedomstream-qualification-${id}`;
const active = `Software\\FreedomStream\\Qualification-${id}`;
const env = {
  ...process.env,
  FREEDOMSTREAM_LINK_TEST_SCHEME: scheme,
  FREEDOMSTREAM_LINK_TEST_ACTIVE: active,
  LINK_FIXTURE_EVENTS: join(output, "events.txt"),
};
const run = (file, args, options = {}) =>
  execFileSync(file, args, {
    cwd: root,
    env,
    windowsHide: true,
    stdio: "inherit",
    ...options,
  });
run("cargo", [
  "build",
  "--locked",
  "--release",
  "--manifest-path",
  "apps/installed-launcher/Cargo.toml",
  "--target",
  "x86_64-pc-windows-msvc",
  "--features",
  "link-qualification",
]);
const binaries = join(
  root,
  "apps/installed-launcher/target/x86_64-pc-windows-msvc/release",
);
const executable = join(current, "freedomstream-installed.exe");
copyFileSync(join(binaries, "freedomstream-installed.exe"), executable);
copyFileSync(
  join(binaries, "link-fixture.exe"),
  join(current, "FreedomStream.exe"),
);
run(executable, ["--veloapp-install", "0.1.0"]);
const query = () =>
  run("reg.exe", ["query", `HKCU\\${scheme}\\shell\\open\\command`, "/ve"], {
    encoding: "utf8",
    stdio: "pipe",
  });
if (!query().includes(`"${executable}" --browser-link "%1"`))
  throw new Error("Handler does not target the stable installed launcher");
run(executable, ["--veloapp-updated", "0.1.1"]);
if (!query().includes(executable)) throw new Error("Update lost handler");
const first = spawn(executable, [], {
  env,
  windowsHide: true,
  stdio: "ignore",
});
const exited = new Promise((accept, reject) => {
  first.on("error", reject);
  first.on("exit", (code) =>
    code === 0 ? accept() : reject(new Error(`Fixture exited ${code}`)),
  );
});
let timer;
async function waitEvent(text) {
  const deadline = Date.now() + 20000;
  while (
    !existsSync(env.LINK_FIXTURE_EVENTS) ||
    !readFileSync(env.LINK_FIXTURE_EVENTS, "utf8").includes(text)
  ) {
    if (Date.now() > deadline) throw new Error(`Missing ${text} event`);
    await new Promise((next) => setTimeout(next, 50));
  }
}
try {
  await waitEvent("started");
  run(executable, []);
  await waitEvent("focused");
  run(executable, [
    "--browser-link",
    `freedomstream://join?server=https%3A%2F%2Fexample.test#handoff=${"t".repeat(43)}`,
  ]);
  await Promise.race([
    exited,
    new Promise((_, reject) => {
      timer = setTimeout(
        () => reject(new Error("Activation did not exit")),
        25000,
      );
    }),
  ]);
  const events = readFileSync(env.LINK_FIXTURE_EVENTS, "utf8");
  if (events.split("started").length !== 2 || !events.includes("received"))
    throw new Error("Activation opened another publisher");
} finally {
  clearTimeout(timer);
  first.kill();
  run(executable, ["--veloapp-uninstall", "0.1.1"]);
}
if (
  spawnSync("reg.exe", ["query", `HKCU\\${scheme}`], {
    windowsHide: true,
    stdio: "ignore",
  }).status === 0
)
  throw new Error("Uninstall retained handler");
console.log(
  `Passed install/update/uninstall hooks and existing-process link delivery. Fixture evidence: ${output}`,
);
