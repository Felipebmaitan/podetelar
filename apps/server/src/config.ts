import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isIP } from "node:net";

export type Config = {
  discord?: { applicationId: string; publicKey: string; guildIds: string[] };
  host: string;
  port: number;
  publicUrl: string;
  directUrl: string;
  databasePath: string;
  adminToken: string;
  livekitUrl: string;
  livekitInternalUrl: string;
  livekitApiKey: string;
  livekitApiSecret: string;
  webRoot: string;
  trustedProxy?: string;
  publisherDirectory?: string;
  usageMetricsUrl?: string;
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
  const directUrl = new URL(env.DIRECT_URL || publicUrl.origin);
  const directLocal = ["localhost", "127.0.0.1", "[::1]"].includes(
    directUrl.hostname,
  );
  if (
    (!directLocal && directUrl.protocol !== "https:") ||
    (publicUrl.protocol === "https:" && directUrl.protocol !== "https:") ||
    !["http:", "https:"].includes(directUrl.protocol) ||
    directUrl.pathname !== "/" ||
    directUrl.search ||
    directUrl.hash ||
    directUrl.username ||
    directUrl.password
  )
    throw new Error(
      "DIRECT_URL must be an HTTPS origin, or an HTTP loopback origin for local development.",
    );
  // Both hostnames terminate at the admitted gateway, never at raw SFU HTTP.
  const livekitUrl = new URL("/media", directUrl);
  livekitUrl.protocol = directUrl.protocol === "https:" ? "wss:" : "ws:";
  // The native WebSocket transport does not retry IPv4 after localhost's ::1
  // address, while the development/Docker listeners bind IPv4 loopback.
  if (directUrl.protocol === "http:" && directUrl.hostname === "localhost")
    livekitUrl.hostname = "127.0.0.1";
  if (env.LIVEKIT_URL && env.LIVEKIT_URL !== livekitUrl.toString())
    throw new Error(
      "LIVEKIT_URL is derived from DIRECT_URL (default PUBLIC_URL) + /media. Configure LIVEKIT_INTERNAL_URL for the private SFU.",
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
  if (env.TRUSTED_PROXY && isIP(env.TRUSTED_PROXY) !== 4)
    throw new Error(
      "TRUSTED_PROXY must be the single IPv4 address of the private proxy gateway.",
    );
  const discordValues = [
    env.DISCORD_APPLICATION_ID,
    env.DISCORD_PUBLIC_KEY,
    env.DISCORD_GUILD_IDS,
  ];
  let discord: Config["discord"];
  if (discordValues.some(Boolean)) {
    const guildIds = (env.DISCORD_GUILD_IDS ?? "")
      .split(",")
      .map((id) => id.trim());
    if (
      !/^\d{17,20}$/.test(env.DISCORD_APPLICATION_ID ?? "") ||
      !/^[a-fA-F0-9]{64}$/.test(env.DISCORD_PUBLIC_KEY ?? "") ||
      !guildIds.length ||
      guildIds.length > 100 ||
      !guildIds.every((id) => /^\d{17,20}$/.test(id))
    )
      throw new Error(
        "Set valid DISCORD_APPLICATION_ID, DISCORD_PUBLIC_KEY and DISCORD_GUILD_IDS together.",
      );
    discord = {
      applicationId: env.DISCORD_APPLICATION_ID!,
      publicKey: env.DISCORD_PUBLIC_KEY!,
      guildIds,
    };
  }
  return {
    ...(discord ? { discord } : {}),
    host: env.HOST ?? "127.0.0.1",
    port,
    publicUrl: publicUrl.origin,
    directUrl: directUrl.origin,
    databasePath: resolve(
      repositoryRoot,
      env.DATABASE_PATH ?? "data/freedomstream.db",
    ),
    adminToken: required("ADMIN_TOKEN", 32),
    usageMetricsUrl:
      env.USAGE_METRICS_URL ??
      `${internalUrl.protocol}//${internalUrl.hostname}:6789/metrics`,
    livekitUrl: livekitUrl.toString().replace(/\/$/, ""),
    livekitInternalUrl: internalUrl.origin,
    livekitApiKey: required("LIVEKIT_API_KEY", 8),
    livekitApiSecret: required("LIVEKIT_API_SECRET", 32),
    webRoot: fileURLToPath(new URL("../../web/dist", import.meta.url)),
    ...(env.TRUSTED_PROXY ? { trustedProxy: env.TRUSTED_PROXY } : {}),
    ...(env.PUBLISHER_DIRECTORY
      ? { publisherDirectory: resolve(env.PUBLISHER_DIRECTORY) }
      : {}),
  };
}
