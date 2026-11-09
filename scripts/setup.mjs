import { randomBytes } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const target = fileURLToPath(new URL("../.env", import.meta.url));
if (existsSync(target)) {
  console.log(".env already exists; kept your configuration unchanged.");
} else {
  const template = readFileSync(
    new URL("../.env.example", import.meta.url),
    "utf8",
  );
  const generated = template.replaceAll("GENERATE_WITH_NPM_RUN_SETUP", () =>
    randomBytes(32).toString("base64url"),
  );
  writeFileSync(target, generated, { flag: "wx", mode: 0o600 });
  console.log(
    "Created .env with unique secrets. Keep this file private; the server owner uses ADMIN_TOKEN to create rooms.",
  );
}
