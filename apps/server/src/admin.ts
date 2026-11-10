import { createHash, randomBytes } from "node:crypto";
import type { FastifyInstance, FastifyRequest } from "fastify";
import type { Config } from "./config.js";
import { secretsEqual } from "./rooms.js";
import type { Usage } from "./usage.js";

const COOKIE = "fs_admin";
const TTL = 8 * 60 * 60 * 1000;
const hash = (value: string) =>
  createHash("sha256").update(value).digest("hex");

/** One administrator, opaque expiring cookies. Restart and credential rotation revoke access. */
export async function adminRoutes(
  app: FastifyInstance,
  config: Config,
  usage: Usage,
  now = Date.now,
) {
  const sessions = new Map<string, number>();
  const cookie = {
    httpOnly: true,
    secure: config.publicUrl.startsWith("https:"),
    sameSite: "strict" as const,
    path: "/api/admin",
  };
  const authenticated = (request: FastifyRequest) => {
    const token = request.cookies[COOKIE];
    return Boolean(
      token &&
      /^[A-Za-z0-9_-]{43}$/.test(token) &&
      (sessions.get(hash(token)) ?? 0) > now(),
    );
  };
  app.post(
    "/api/admin/login",
    { config: { rateLimit: { max: 5, timeWindow: "1 minute" } } },
    async (request, reply) => {
      const body = request.body as { password?: unknown } | null;
      if (
        !body ||
        typeof body.password !== "string" ||
        body.password.length > 256 ||
        !secretsEqual(body.password, config.adminToken)
      )
        return reply
          .code(401)
          .send({ error: "Incorrect administrator password." });
      for (const [key, expiry] of sessions)
        if (expiry <= now()) sessions.delete(key);
      if (sessions.size >= 16) sessions.delete(sessions.keys().next().value!);
      const previous = request.cookies[COOKIE];
      if (previous) sessions.delete(hash(previous));
      const token = randomBytes(32).toString("base64url");
      sessions.set(hash(token), now() + TTL);
      reply.setCookie(COOKIE, token, { ...cookie, maxAge: TTL / 1000 });
      return { administrator: true };
    },
  );
  app.get("/api/admin/session", async (request, reply) => {
    if (!authenticated(request))
      return reply.code(401).send({ error: "Sign in as administrator." });
    return { administrator: true };
  });
  app.post("/api/admin/logout", async (request, reply) => {
    const token = request.cookies[COOKIE];
    if (token) sessions.delete(hash(token));
    reply.clearCookie(COOKIE, cookie);
    return reply.code(204).send();
  });
  app.get<{ Querystring: { from?: string; to?: string; page?: string } }>(
    "/api/admin/usage",
    async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      const from = Number(request.query.from),
        to = Number(request.query.to),
        page = Number(request.query.page ?? 0);
      if (
        !Number.isSafeInteger(from) ||
        !Number.isSafeInteger(to) ||
        from < 0 ||
        to <= from ||
        to - from > 366 * 86400000 ||
        to > now() + 86400000 ||
        !Number.isSafeInteger(page) ||
        page < 0 ||
        page > 100000
      )
        return reply
          .code(400)
          .send({ error: "Choose a valid period of up to 366 days." });
      return usage.report(from, to, page);
    },
  );
}
