import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { createHash } from "node:crypto";
import { join } from "node:path";
import type { FastifyInstance } from "fastify";
import type { Rooms } from "./rooms.js";

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

export function publisherRoutes(
  app: FastifyInstance,
  artifact: Awaited<ReturnType<typeof loadPublisher>>,
  rooms: Rooms,
) {
  if (!artifact) return;
  const { release, path, size } = artifact;
  const admitted = (id: string, cookie?: string) =>
    Boolean(rooms.session(id, cookie));
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
        downloadUrl: `/api/rooms/${request.params.roomId}/publisher/windows.exe`,
      };
    },
  );
  app.get<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/publisher/windows.exe",
    async (request, reply) => {
      if (!admitted(request.params.roomId, request.cookies.fs_session))
        return reply.code(401).send({ error: "Room access required." });
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
