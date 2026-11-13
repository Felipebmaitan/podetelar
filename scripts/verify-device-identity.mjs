// Opt-in Windows qualification against an isolated local Docker API. The native
// test binary is built from this checkout by npm run native:test. Never targets
// production, prints identifiers/secrets, or changes the real Windows profile.
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";

assert.equal(process.platform, "win32");
assert.equal(process.versions.node.split(".")[0], "24");
const origin = new URL(process.env.E2E_BASE_URL ?? "http://localhost:13310")
  .origin;
assert.ok(["localhost", "127.0.0.1"].includes(new URL(origin).hostname));
assert.ok(
  process.env.ADMIN_TOKEN,
  "Supply a local test administrator credential.",
);
const executable = process.env.FREEDOMSTREAM_NATIVE_TEST_EXE;
assert.ok(
  executable,
  "Supply the publisher's freshly built Rust test executable.",
);
const container = process.env.FREEDOMSTREAM_DEVICE_DB_CONTAINER;
assert.ok(
  container && /^[a-z0-9][a-z0-9_-]{0,127}$/.test(container),
  "Supply the isolated local API container name.",
);
const root = resolve(".local", `device-identity-${randomUUID()}`);
mkdirSync(root, { recursive: true });
const created = await fetch(`${origin}/api/rooms`, {
  method: "POST",
  headers: {
    Origin: origin,
    "Content-Type": "application/json",
    Authorization: `Bearer ${process.env.ADMIN_TOKEN}`,
  },
  body: JSON.stringify({
    name: "Native device qualification",
    displayName: "Fixture creator",
  }),
});
assert.equal(created.status, 201);
const room = await created.json();
const results = [];
for (let index = 0; index < 2; index++) {
  const directory = join(root, `copy-${index}`);
  mkdirSync(directory);
  const copy = join(directory, "publisher-tests.exe");
  copyFileSync(executable, copy);
  const output = join(directory, "result.json");
  const fixture = join(directory, "fixture.json");
  writeFileSync(
    fixture,
    JSON.stringify({ invitation: room.invitationUrl, output }),
    { flag: "wx", mode: 0o600 },
  );
  try {
    execFileSync(
      copy,
      [
        "--exact",
        "admission::tests::qualifies_device_identity_and_admission_in_an_independent_process",
        "--ignored",
        "--test-threads=1",
      ],
      {
        cwd: directory,
        env: {
          ...process.env,
          LOCALAPPDATA: directory,
          FREEDOMSTREAM_DEVICE_FIXTURE: fixture,
        },
        windowsHide: true,
        timeout: 30000,
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
  } catch (error) {
    writeFileSync(
      join(directory, "failure.log"),
      Buffer.concat([
        error.stdout ?? Buffer.alloc(0),
        error.stderr ?? Buffer.alloc(0),
      ]),
    );
    throw new Error(
      "Native identity qualification failed; inspect the private local failure log.",
    );
  }
  results.push(JSON.parse(readFileSync(output, "utf8")));
}
assert.ok(
  results[0].deviceId === results[1].deviceId,
  "Separate processes, executable locations and empty app profiles must recognize the same PC",
);
assert.ok(
  results[0].participant !== results[1].participant,
  "Admission identities must stay independent",
);
const query = `import { DatabaseSync } from 'node:sqlite';
let input=''; for await (const chunk of process.stdin) input+=chunk;
const ids=JSON.parse(input);
const db=new DatabaseSync('/app/data/freedomstream.db',{readOnly:true});
console.log(JSON.stringify(db.prepare('SELECT id,device_id FROM usage_users WHERE id IN (?,?)').all(...ids)));
db.close();`;
const rows = JSON.parse(
  execFileSync(
    "docker",
    ["exec", "-i", container, "node", "--input-type=module", "-e", query],
    {
      input: JSON.stringify(results.map((result) => result.participant)),
      encoding: "utf8",
      windowsHide: true,
      stdio: ["pipe", "pipe", "pipe"],
    },
  ),
);
assert.equal(rows.length, 2);
for (const row of rows)
  assert.ok(
    row.device_id === results[0].deviceId,
    "The API must bind each session to its native PC identifier",
  );
console.log(
  "PASS: independent Windows processes, executable copies and empty app profiles retain one PC identity; actual HTTP admissions bind it privately to distinct sessions.",
);
