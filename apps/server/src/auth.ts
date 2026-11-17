import type { FastifyInstance } from "fastify";
import { z } from "zod";
import type { Config } from "./config.js";
import { Accounts, accountBearer, discordUserSchema } from "./accounts.js";

async function json(response: Response) {
  if (
    !response.ok ||
    Number(response.headers.get("content-length") ?? 0) > 65536
  )
    throw Error("Discord authentication unavailable");
  const reader = response.body?.getReader();
  if (!reader) throw Error("No Discord response");
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      const result = await reader.read();
      if (result.done) break;
      size += result.value.length;
      if (size > 65536) throw Error("Discord response too large");
      chunks.push(result.value);
    }
  } finally {
    await reader.cancel();
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8")) as unknown;
}
export async function discordAuthRoutes(
  app: FastifyInstance,
  config: Config,
  accounts: Accounts,
  fetcher: typeof fetch = fetch,
) {
  const loginRate = {
    max: 120,
    timeWindow: "1 minute",
    keyGenerator: (request: import("fastify").FastifyRequest) =>
      accounts.authenticate(accountBearer(request.headers.authorization))
        ?.sessionId ?? request.ip,
  };
  app.get("/api/auth/config", async () => ({
    discord: Boolean(config.discord?.clientSecret),
  }));
  app.post(
    "/api/auth/discord/start",
    { config: { rateLimit: { max: 20, timeWindow: "1 minute" } } },
    async (request, reply) => {
      if (!config.discord?.clientSecret)
        return reply
          .code(503)
          .send({ error: "Discord sign-in is not configured on this server." });
      const input = z
        .object({ challenge: z.string().regex(/^[a-f0-9]{64}$/) })
        .strict()
        .safeParse(request.body);
      if (!input.success)
        return reply.code(400).send({ error: "Invalid login request." });
      const flow = accounts.start(input.data.challenge);
      if (!flow)
        return reply
          .code(503)
          .send({ error: "Sign-in is busy. Try again shortly." });
      return {
        requestId: flow.id,
        expiresAt: flow.expiresAt,
        authorizationUrl: `${config.publicUrl}/api/auth/discord/authorize?request=${flow.id}`,
      };
    },
  );
  const cookie = {
    httpOnly: true,
    secure: config.publicUrl.startsWith("https:"),
    sameSite: "lax" as const,
    path: "/api/auth/discord",
    maxAge: 300,
  };
  app.get<{ Querystring: { request?: string } }>(
    "/api/auth/discord/authorize",
    async (request, reply) => {
      if (
        !config.discord?.clientSecret ||
        !/^[-\w]{22}$/.test(request.query.request ?? "")
      )
        return reply
          .code(400)
          .type("text/plain")
          .send("Start Discord sign-in from PodeTelar.");
      const flow = accounts.authorize(request.query.request!);
      if (!flow)
        return reply
          .code(410)
          .type("text/plain")
          .send("This sign-in request expired. Start again in PodeTelar.");
      reply.setCookie("podetelar_oauth_browser", flow.browser, cookie);
      const url = new URL("https://discord.com/oauth2/authorize");
      url.search = new URLSearchParams({
        client_id: config.discord.applicationId,
        response_type: "code",
        redirect_uri: `${config.publicUrl}/api/auth/discord/callback`,
        scope: "identify",
        state: flow.state,
      }).toString();
      return reply.redirect(url.toString());
    },
  );
  app.get("/api/auth/discord/callback", async (request, reply) => {
    const input = z
      .object({
        state: z.string().regex(/^[-\w]{43}$/),
        code: z.string().min(1).max(2048).optional(),
        error: z.string().max(128).optional(),
      })
      .safeParse(request.query);
    if (!config.discord?.clientSecret || !input.success)
      return reply
        .code(400)
        .type("text/plain")
        .send("Invalid Discord sign-in response.");
    const flow = accounts.consume(
      input.data.state,
      request.cookies.podetelar_oauth_browser,
    );
    reply.clearCookie("podetelar_oauth_browser", cookie);
    if (!flow)
      return reply
        .code(400)
        .type("text/plain")
        .send(
          "Sign-in expired or belongs to a different browser. Start again in PodeTelar.",
        );
    if (!input.data.code || input.data.error) {
      accounts.complete(flow, null);
      return reply.redirect("/auth/discord/failed");
    }
    try {
      const tokenResponse = await json(
        await fetcher("https://discord.com/api/oauth2/token", {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body: new URLSearchParams({
            client_id: config.discord.applicationId,
            client_secret: config.discord.clientSecret,
            grant_type: "authorization_code",
            code: input.data.code,
            redirect_uri: `${config.publicUrl}/api/auth/discord/callback`,
          }),
          signal: AbortSignal.timeout(8000),
          redirect: "error",
        }),
      );
      const token = z
        .object({
          access_token: z.string().min(1).max(4096),
          token_type: z.literal("Bearer"),
          scope: z.string().max(1024),
        })
        .parse(tokenResponse);
      if (!token.scope.split(" ").includes("identify"))
        throw Error("Missing identity scope");
      const user = discordUserSchema.parse(
        await json(
          await fetcher("https://discord.com/api/v10/users/@me", {
            headers: { Authorization: `Bearer ${token.access_token}` },
            signal: AbortSignal.timeout(8000),
            redirect: "error",
          }),
        ),
      );
      accounts.complete(flow, user);
      return reply.redirect("/auth/discord/complete");
    } catch {
      accounts.complete(flow, null);
      return reply.redirect("/auth/discord/failed");
    }
  });
  app.get("/auth/discord/complete", async (_request, reply) =>
    reply
      .type("text/html")
      .send(
        "<!doctype html><html lang=en><meta charset=utf-8><title>Discord connected</title><main><h1>Discord connected</h1><p>Return to PodeTelar. You can close this tab.</p></main></html>",
      ),
  );
  app.get("/auth/discord/failed", async (_request, reply) =>
    reply
      .type("text/html")
      .send(
        "<!doctype html><html lang=en><meta charset=utf-8><title>Discord sign-in failed</title><main><h1>Discord sign-in was not completed</h1><p>Return to PodeTelar to try again.</p></main></html>",
      ),
  );
  app.post(
    "/api/auth/discord/poll",
    {
      config: {
        rateLimit: {
          max: 120,
          timeWindow: "1 minute",
          hook: "preHandler",
          keyGenerator: (request) => {
            const body = request.body as { requestId?: unknown } | null;
            const proof = (request.body as { proof?: unknown } | null)?.proof;
            return typeof body?.requestId === "string" &&
              typeof proof === "string"
              ? (accounts.flowKey(body.requestId, proof) ?? request.ip)
              : request.ip;
          },
        },
      },
    },
    async (request, reply) => {
      const input = z
        .object({
          requestId: z.string().regex(/^[-\w]{22}$/),
          proof: z.string().regex(/^[-\w]{43}$/),
        })
        .strict()
        .safeParse(request.body);
      if (!input.success)
        return reply.code(400).send({ error: "Invalid login request." });
      const result = accounts.poll(input.data.requestId, input.data.proof);
      if (result.status === "denied")
        return reply
          .code(401)
          .send({ error: "Sign-in expired or was denied. Start again." });
      return reply.code(result.status === "pending" ? 202 : 200).send(result);
    },
  );
  app.get(
    "/api/auth/session",
    { config: { rateLimit: loginRate } },
    async (request, reply) => {
      const account = accounts.authenticate(
        accountBearer(request.headers.authorization),
      );
      return (
        account ?? reply.code(401).send({ error: "Sign in with Discord." })
      );
    },
  );
  app.post("/api/auth/logout", async (request, reply) => {
    const token = accountBearer(request.headers.authorization);
    if (token) accounts.logout(token);
    return reply.code(204).send();
  });
}
