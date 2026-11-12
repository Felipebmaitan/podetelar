import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { createHash, randomBytes } from "node:crypto";
import { join } from "node:path";
import type { FastifyInstance } from "fastify";
import type { Rooms } from "./rooms.js";
import type { Config } from "./config.js";

/** A single operator-installed, verified release. Never a request-selected path. */
export async function loadPublisher(directory?: string) {
  if (!directory) return;
  const release = JSON.parse(
    await readFile(join(directory, "publisher.json"), "utf8"),
  ) as Record<string, unknown>;
  if (
    typeof release.filename !== "string" ||
    !/^freedomstream-windows-x64-[\d.a-f-]+\.exe$/.test(release.filename) ||
    typeof release.version !== "string" ||
    !/^\d+\.\d+\.\d+$/.test(release.version) ||
    typeof release.commit !== "string" ||
    !/^[a-f0-9]{40}$/.test(release.commit) ||
    typeof release.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(release.sha256) ||
    !Number.isSafeInteger(release.size) ||
    Number(release.size) < 1 ||
    Number(release.size) > 512 * 1024 * 1024
  )
    throw new Error("Invalid publisher release manifest.");
  const path = join(directory, release.filename);
  const size = (await stat(path)).size;
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  if (size !== release.size || hash.digest("hex") !== release.sha256)
    throw new Error("Publisher archive does not match its release manifest.");
  return { release, path, size };
}

export async function publisherRoutes(
  app: FastifyInstance,
  artifact: Awaited<ReturnType<typeof loadPublisher>>,
  rooms: Rooms,
  config: Config,
  now = Date.now,
) {
  if (!artifact) return;
  const { release, path, size } = artifact;
  const admitted = (id: string, cookie?: string) =>
    Boolean(rooms.session(id, cookie));
  const split = config.directUrl !== config.publicUrl;
  const tickets = new Map<
    string,
    { room: string; cookie: string; expires: number }
  >();
  const digest = (ticket: string) =>
    createHash("sha256").update(ticket).digest("hex");
  const sweep = () => {
    for (const [key, ticket] of tickets)
      if (ticket.expires <= now()) tickets.delete(key);
  };
  const timer = setInterval(sweep, 15_000);
  timer.unref();
  app.addHook("onClose", async () => {
    clearInterval(timer);
    tickets.clear();
  });
  const download = (room: string, cookie: string) => {
    if (!split) return `/api/rooms/${room}/publisher/windows.exe`;
    sweep();
    if (tickets.size >= 128) return;
    const token = randomBytes(32).toString("base64url");
    tickets.set(digest(token), { room, cookie, expires: now() + 60_000 });
    // The fragment is never sent in the direct HTTP request URL or Referer.
    return `${config.directUrl}/download#ticket=${token}`;
  };
  await app.register(async (scope) => {
    scope.addContentTypeParser(
      "application/x-www-form-urlencoded",
      { parseAs: "string", bodyLimit: 128 },
      (_request, body, done) => done(null, new URLSearchParams(String(body))),
    );
    scope.post("/api/publisher/download", async (request, reply) => {
      const token =
        request.body instanceof URLSearchParams
          ? request.body.get("ticket")
          : undefined;
      const key =
        typeof token === "string" && /^[A-Za-z0-9_-]{43}$/.test(token)
          ? digest(token)
          : "";
      const ticket = tickets.get(key);
      tickets.delete(key);
      if (
        !ticket ||
        ticket.expires <= now() ||
        !admitted(ticket.room, ticket.cookie)
      )
        return reply
          .code(401)
          .send({ error: "Download expired. Open the room download again." });
      return reply
        .type("application/vnd.microsoft.portable-executable")
        .header(
          "Content-Disposition",
          `attachment; filename="${release.filename}"`,
        )
        .header("Content-Length", size)
        .send(createReadStream(path));
    });
  });
  app.get<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/publisher",
    async (request, reply) => {
      if (!admitted(request.params.roomId, request.cookies.fs_session))
        return reply.code(401).send({ error: "Room access required." });
      return {
        version: release.version,
        commit: release.commit,
        sha256: release.sha256,
        size,
        // Issue the short-lived ticket only when clicked, not when rendered.
        downloadUrl: `/api/rooms/${request.params.roomId}/publisher/windows.exe`,
      };
    },
  );
  app.get<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/publisher/windows.exe",
    async (request, reply) => {
      if (!admitted(request.params.roomId, request.cookies.fs_session))
        return reply.code(401).send({ error: "Room access required." });
      if (
        split &&
        request.headers.host?.toLowerCase() !== new URL(config.directUrl).host
      ) {
        const url = download(
          request.params.roomId,
          request.cookies.fs_session!,
        );
        if (!url)
          return reply
            .code(503)
            .send({ error: "Downloads busy. Try again shortly." });
        return reply.redirect(url, 303);
      }
      return reply
        .type("application/vnd.microsoft.portable-executable")
        .header(
          "Content-Disposition",
          `attachment; filename="${release.filename}"`,
        )
        .header("Content-Length", size)
        .send(createReadStream(path));
    },
  );
  // Existing open room tabs can still follow their old download address.
  app.get<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/publisher/windows.zip",
    async (request, reply) => {
      if (!admitted(request.params.roomId, request.cookies.fs_session))
        return reply.code(401).send({ error: "Room access required." });
      return reply.redirect(
        `/api/rooms/${request.params.roomId}/publisher/windows.exe`,
        307,
      );
    },
  );
}
