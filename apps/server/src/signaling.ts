import websocket from "@fastify/websocket";
import WebSocket, { type RawData } from "ws";
import { TokenVerifier } from "livekit-server-sdk";
import type { FastifyInstance } from "fastify";
import type { Config } from "./config.js";
import type { Rooms } from "./rooms.js";

const MAX_BYTES = 1_048_576;
const PATHS = [
  "/media/rtc",
  "/media/rtc/v1",
  "/media/rtc/validate",
  "/media/rtc/v1/validate",
];
type Peer = { room: string; identity: string; close: () => void };

/** Admission for every SFU handshake, including SDK-refreshed tokens. Carries
 * bounded signaling messages only; media stays on the SFU's ICE transports. */
export class Signaling {
  private readonly counts = {
    admissions: 0,
    upstreamOpens: 0,
    upstreamFailures: 0,
    incomingMessages: 0,
    outgoingMessages: 0,
  };
  diagnostics() {
    return { ...this.counts, connections: this.peers.size };
  }
  private readonly peers = new Set<Peer>();
  constructor(private readonly rooms: Rooms) {}
  closeRevoked(): void {
    for (const peer of this.peers)
      if (!this.rooms.active(peer.room, peer.identity)) peer.close();
  }
  close(): void {
    for (const peer of this.peers) peer.close();
  }
  async register(app: FastifyInstance, config: Config): Promise<void> {
    const verifier = new TokenVerifier(
      config.livekitApiKey,
      config.livekitApiSecret,
    );
    await app.register(websocket, {
      options: { maxPayload: MAX_BYTES },
      errorHandler: (_error, socket) => socket.terminate(),
    });
    await app.register(
      async (scope) => {
        // Neither SDK URLs nor signaling payloads belong in application logs.
        scope.addHook("preValidation", async (request, reply) => {
          const url = new URL(request.url, config.publicUrl);
          const queryToken = url.searchParams.get("access_token");
          const header = request.headers.authorization;
          const token =
            queryToken ??
            (header?.startsWith("Bearer ") ? header.slice(7) : null);
          if (
            queryToken &&
            request.headers.origin &&
            request.headers.origin !== config.publicUrl
          )
            return reply
              .code(403)
              .send({ error: "Request origin is not allowed." });
          if (
            !token ||
            token.length > 16_384 ||
            url.searchParams.getAll("access_token").length > 1 ||
            (queryToken && header)
          )
            return reply.code(401).send({ error: "Room access required." });
          try {
            const claims = await verifier.verify(token, 0);
            const room = claims.video?.room?.match(
              /^freedomstream_([A-Za-z0-9_-]{22})$/,
            )?.[1];
            if (
              !room ||
              !claims.sub ||
              !claims.video?.roomJoin ||
              !this.rooms.active(room, claims.sub)
            )
              throw new Error();
            // Keep authority out of arbitrary request properties/log serialization.
            admitted.set(request, { room, identity: claims.sub, token });
            this.counts.admissions++;
          } catch {
            return reply.code(401).send({
              error: "Room access ended. Open a current invitation to join.",
            });
          }
        });
        const admitted = new WeakMap<
          object,
          { room: string; identity: string; token: string }
        >();
        for (const path of PATHS) {
          if (path.endsWith("/validate")) {
            scope.get(path, async () => ({ status: "ok" }));
            continue;
          }
          scope.get(path, { websocket: true }, (source, request) => {
            const member = admitted.get(request);
            if (
              !member ||
              !this.rooms.active(member.room, member.identity) ||
              this.peers.size >= 128 ||
              [...this.peers].filter((p) => p.identity === member.identity)
                .length >= 4
            ) {
              source.close(1008, "Room connection unavailable.");
              return;
            }
            const url = new URL(
              request.url.replace(/^\/media/, ""),
              config.livekitInternalUrl,
            );
            url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
            url.searchParams.delete("access_token");
            const target = new WebSocket(url, {
              maxPayload: MAX_BYTES,
              handshakeTimeout: 8000,
              followRedirects: false,
              perMessageDeflate: false,
              headers: { Authorization: `Bearer ${member.token}` },
            });
            let queuedBytes = 0;
            let queue: { data: RawData; binary: boolean }[] = [];
            let closed = false;
            const peer: Peer = {
              room: member.room,
              identity: member.identity,
              close: () => {
                if (closed) return;
                closed = true;
                queue = [];
                queuedBytes = 0;
                source.terminate();
                target.terminate();
                this.peers.delete(peer);
              },
            };
            this.peers.add(peer);
            const active = () =>
              !closed && this.rooms.active(member.room, member.identity);
            const size = (data: RawData) =>
              Array.isArray(data)
                ? data.reduce((n, b) => n + b.length, 0)
                : data.byteLength;
            const send = (
              destination: WebSocket,
              data: RawData,
              binary: boolean,
            ) => {
              if (
                !active() ||
                destination.readyState !== WebSocket.OPEN ||
                destination.bufferedAmount + size(data) > MAX_BYTES
              ) {
                peer.close();
                return;
              }
              destination.send(data, { binary }, (error) => {
                if (error) peer.close();
              });
            };
            source.on("message", (data, binary) => {
              this.counts.incomingMessages++;
              if (!active()) {
                peer.close();
                return;
              }
              if (target.readyState === WebSocket.CONNECTING) {
                queuedBytes += size(data);
                if (queuedBytes > MAX_BYTES || queue.length >= 128)
                  peer.close();
                else queue.push({ data, binary });
              } else send(target, data, binary);
            });
            target.on("open", () => {
              this.counts.upstreamOpens++;
              if (!active()) {
                peer.close();
                return;
              }
              for (const { data, binary } of queue) send(target, data, binary);
              queue = [];
              queuedBytes = 0;
            });
            target.on("message", (data, binary) => {
              this.counts.outgoingMessages++;
              send(source, data, binary);
            });
            source.on("error", peer.close);
            target.on("error", () => {
              this.counts.upstreamFailures++;
              peer.close();
            });
            source.on("close", peer.close);
            target.on("close", peer.close);
            target.on("unexpected-response", (_request, response) => {
              response.destroy();
              peer.close();
            });
          });
        }
      },
      { logLevel: "silent" },
    );
  }
}
