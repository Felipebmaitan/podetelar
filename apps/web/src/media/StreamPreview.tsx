import { useEffect, useState } from "react";
import { PREVIEW_MAX_BYTES, PREVIEW_TTL_MS } from "@freedomstream/contracts";
import type { LiveStream } from "./model";

/** Fetches only the latest announced still; no media subscription or polling. */
export function StreamPreview({
  roomId,
  stream,
}: {
  roomId: string;
  stream: LiveStream;
}) {
  const [image, setImage] = useState("");
  const preview = stream.muted ? null : stream.preview;
  const revision = preview?.revision;
  const updatedAt = preview?.updatedAt;
  useEffect(() => {
    setImage("");
    if (!revision || updatedAt === undefined) return;
    const abort = new AbortController();
    let objectUrl = "";
    let retry: ReturnType<typeof setTimeout> | undefined;
    const expiry = setTimeout(
      () => {
        abort.abort();
        if (objectUrl) {
          URL.revokeObjectURL(objectUrl);
          objectUrl = "";
        }
        setImage("");
      },
      Math.max(0, updatedAt + PREVIEW_TTL_MS - Date.now()),
    );
    const load = async (attempt: number) => {
      try {
        const response = await fetch(
          `/api/rooms/${roomId}/previews/${stream.publisherId}/${stream.kind}/${revision}`,
          {
            credentials: "same-origin",
            signal: abort.signal,
            cache: "no-store",
          },
        );
        if (!response.ok) {
          // An attribute can arrive just before its upload commits. Brief,
          // bounded retries also cover a temporary administrative RPC failure.
          if (attempt < 2 && [404, 503].includes(response.status))
            retry = setTimeout(
              () => void load(attempt + 1),
              1000 * (attempt + 1),
            );
          return;
        }
        if (response.headers.get("content-type") !== "image/jpeg") return;
        const blob = await response.blob();
        if (abort.signal.aborted || blob.size > PREVIEW_MAX_BYTES) return;
        objectUrl = URL.createObjectURL(blob);
        setImage(objectUrl);
      } catch {
        /* Placeholder remains available offline. */
      }
    };
    void load(0);
    return () => {
      abort.abort();
      clearTimeout(expiry);
      if (retry !== undefined) clearTimeout(retry);
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [roomId, stream.publisherId, stream.kind, revision, updatedAt]);
  return (
    <div className={`stream-placeholder${image ? " has-preview" : ""}`}>
      {image ? (
        <img
          src={image}
          width="320"
          height="180"
          alt={`${stream.publisherName} ${stream.kind} preview`}
        />
      ) : (
        <span aria-hidden="true">
          {stream.publisherName.slice(0, 1).toUpperCase()}
        </span>
      )}
      <p>
        {!stream.compatible
          ? "This browser cannot receive HEVC. Ask the publisher to choose H.264."
          : stream.local
            ? "Your stream is available to friends."
            : "Choose Watch to receive this stream."}
      </p>
    </div>
  );
}
