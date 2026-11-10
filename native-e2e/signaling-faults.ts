// Local-only adversarial signaling harness. Never logs signaling or tokens.
import { createServer } from "node:http";
import { once } from "node:events";
import { WebSocket, WebSocketServer } from "ws";
import {
  SignalResponse,
  ParticipantInfo,
  ParticipantUpdate,
} from "@livekit/protocol";

export async function signalingFaults(
  origin: string,
  change: (response: SignalResponse) => void = () => {},
) {
  const targetOrigin = new URL(origin);
  if (
    targetOrigin.protocol !== "http:" ||
    !["localhost", "127.0.0.1"].includes(targetOrigin.hostname)
  )
    throw new Error("Fault tests require a local server.");
  const server = createServer((_request, response) => {
    response.writeHead(200);
    response.end("ok");
  });
  const sockets = new Set<WebSocket>();
  const downlinks = new Set<WebSocket>();
  let receiverIdentity = "";
  const webSockets = new WebSocketServer({
    noServer: true,
    maxPayload: 1_048_576,
  });
  server.on("upgrade", (request, socket, head) => {
    webSockets.handleUpgrade(request, socket, head, (client) => {
      const incoming = new URL(request.url!, "http://localhost");
      const target = new URL(targetOrigin);
      target.protocol = "ws:";
      target.pathname = "/media" + incoming.pathname;
      target.search = incoming.search;
      const upstream = new WebSocket(target, {
        maxPayload: 1_048_576,
        handshakeTimeout: 8000,
        followRedirects: false,
        headers: request.headers.authorization
          ? { Authorization: request.headers.authorization }
          : {},
      });
      sockets.add(client);
      sockets.add(upstream);
      downlinks.add(client);
      const queue: { data: Buffer; binary: boolean }[] = [];
      let queuedBytes = 0;
      client.on("message", (data, binary) => {
        const bytes = Buffer.from(data as Buffer);
        if (upstream.readyState === WebSocket.OPEN)
          upstream.send(bytes, { binary });
        else if (queue.length < 32 && queuedBytes + bytes.length <= 1_048_576) {
          queue.push({ data: bytes, binary });
          queuedBytes += bytes.length;
        } else client.close();
      });
      upstream.on("open", () => {
        for (const item of queue)
          upstream.send(item.data, { binary: item.binary });
        queue.length = 0;
      });
      upstream.on("message", (data, binary) => {
        try {
          const response = binary
            ? SignalResponse.fromBinary(new Uint8Array(data as Buffer))
            : SignalResponse.fromJsonString(data.toString());
          if (response.message.case === "join")
            receiverIdentity =
              response.message.value.participant?.identity ?? "";
          change(response);
          if (client.readyState === WebSocket.OPEN)
            client.send(
              binary ? response.toBinary() : response.toJsonString(),
              { binary },
            );
        } catch {
          client.close();
        }
      });
      client.on("error", () => {});
      upstream.on("error", () => client.close());
      client.on("close", () => {
        upstream.terminate();
        downlinks.delete(client);
      });
      upstream.on("close", () => client.close());
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("Fault listener unavailable.");
  return {
    url: `ws://127.0.0.1:${address.port}`,
    identity: () => receiverIdentity,
    participant: (participant: ParticipantInfo) => {
      const response = new SignalResponse({
        message: {
          case: "update",
          value: new ParticipantUpdate({ participants: [participant] }),
        },
      });
      for (const client of downlinks)
        if (client.readyState === WebSocket.OPEN)
          client.send(response.toBinary(), { binary: true });
    },
    close: async () => {
      for (const socket of sockets) socket.terminate();
      webSockets.close();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
}
