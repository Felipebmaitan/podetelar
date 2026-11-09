import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export type Config = {
  host: string;
  port: number;
  publicUrl: string;
  databasePath: string;
  adminToken: string;
  livekitUrl: string;
  livekitInternalUrl: string;
  livekitApiKey: string;
  livekitApiSecret: string;
  webRoot: string;
};

const repositoryRoot = fileURLToPath(new URL("../../../", import.meta.url));

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  const required = (name: string, minimum: number) => {
    const value = env[name];
    if (!value || value.length < minimum || value.includes("GENERATE_WITH")) {
      throw new Error(`${name} is missing or too short. Run npm run setup.`);
    }
    return value;
  };
  const publicUrl = new URL(env.PUBLIC_URL ?? "http://localhost:5173");
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(
    publicUrl.hostname,
  );
  if (
    (!local && publicUrl.protocol !== "https:") ||
    !["http:", "https:"].includes(publicUrl.protocol) ||
    publicUrl.pathname !== "/" ||
    publicUrl.search ||
    publicUrl.hash ||
    publicUrl.username ||
    publicUrl.password
  ) {
    throw new Error(
      "PUBLIC_URL must be an HTTPS origin, or an HTTP loopback origin for local development.",
    );
  }
  // All public signaling passes session admission, including refreshed JWTs.
  const livekitUrl = new URL("/media", publicUrl);
  livekitUrl.protocol = publicUrl.protocol === "https:" ? "wss:" : "ws:";
  // The native WebSocket transport does not retry IPv4 after localhost's ::1
  // address, while the development/Docker listeners bind IPv4 loopback.
  if (publicUrl.protocol === "http:" && publicUrl.hostname === "localhost")
    livekitUrl.hostname = "127.0.0.1";
  if (env.LIVEKIT_URL && env.LIVEKIT_URL !== livekitUrl.toString())
    throw new Error(
      "LIVEKIT_URL is now derived from PUBLIC_URL + /media. Remove the old direct SFU URL; configure LIVEKIT_INTERNAL_URL for the private SFU.",
    );
  const port = Number(env.PORT ?? 3000);
  const internalUrl = new URL(
    env.LIVEKIT_INTERNAL_URL ?? "http://127.0.0.1:7880",
  );
  if (
    !["http:", "https:"].includes(internalUrl.protocol) ||
    internalUrl.username ||
    internalUrl.password ||
    internalUrl.search ||
    internalUrl.hash ||
    internalUrl.pathname !== "/"
  )
    throw new Error(
      "LIVEKIT_INTERNAL_URL must be an HTTP(S) origin without credentials.",
    );
  if (!Number.isInteger(port) || port < 1 || port > 65535)
    throw new Error("PORT must be between 1 and 65535.");
  return {
    host: env.HOST ?? "127.0.0.1",
    port,
    publicUrl: publicUrl.origin,
    databasePath: resolve(
      repositoryRoot,
      env.DATABASE_PATH ?? "data/freedomstream.db",
    ),
    adminToken: required("ADMIN_TOKEN", 32),
    livekitUrl: livekitUrl.toString().replace(/\/$/, ""),
    livekitInternalUrl: internalUrl.origin,
    livekitApiKey: required("LIVEKIT_API_KEY", 8),
    livekitApiSecret: required("LIVEKIT_API_SECRET", 32),
    webRoot: fileURLToPath(new URL("../../web/dist", import.meta.url)),
  };
}
