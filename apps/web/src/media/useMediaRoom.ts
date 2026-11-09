import { useCallback, useEffect, useRef, useState } from "react";
import type { RoomSession } from "@freedomstream/contracts";
import type { MediaRoom } from "./MediaRoom";
import { emptyMediaSnapshot } from "./model";
import type { StreamKind, PlaybackBuffer, ConnectionRoute } from "./model";

export function useMediaRoom(session: RoomSession | null) {
  const controller = useRef<MediaRoom | null>(null);
  const stop = useRef<(() => Promise<void>) | null>(null);
  const shutdown = useRef<Promise<void>>(Promise.resolve());
  const [snapshot, setSnapshot] = useState(emptyMediaSnapshot);
  const [attempt, setAttempt] = useState(0);
  const [route, setRoute] = useState<ConnectionRoute>("auto");
  const playbackBuffer = useRef<PlaybackBuffer>("smooth");
  const roomId = session?.room.id;
  const identity = session?.participant.id;
  useEffect(() => {
    if (!roomId || !identity) return;
    setSnapshot(emptyMediaSnapshot);
    let cancelled = false;
    let media: MediaRoom | null = null;
    let unsubscribe = () => {};
    let closing: Promise<void> | null = null;
    const previous = shutdown.current;
    const close = () => {
      cancelled = true;
      unsubscribe();
      if (controller.current === media) controller.current = null;
      closing ??= previous.then(() => media?.disconnect()).then(() => {});
      shutdown.current = closing.catch(() => {});
      return closing;
    };
    stop.current = close;
    // Keep capture/signaling code off the unauthenticated landing page.
    void import("./MediaRoom")
      .then(async ({ MediaRoom }) => {
        // A new route uses the same session identity. Finish the old SDK's
        // Leave/disconnect before admitting its replacement, including while
        // the previous connection is still being established.
        await previous;
        if (cancelled) return;
        media = new MediaRoom(roomId, route);
        controller.current = media;
        unsubscribe = media.subscribe(setSnapshot);
        media.setPlaybackBuffer(playbackBuffer.current);
        void media.connect();
      })
      .catch(() => {
        if (!cancelled)
          setSnapshot({
            ...emptyMediaSnapshot,
            connection: "failed",
            error: "Live sharing could not load. Reconnect to try again.",
          });
      });
    return () => {
      if (stop.current === close) stop.current = null;
      void close().catch(() => {});
    };
  }, [roomId, identity, attempt, route]);
  return {
    snapshot,
    route,
    setRoute,
    setPlaybackBuffer: useCallback((value: PlaybackBuffer) => {
      playbackBuffer.current = value;
      controller.current?.setPlaybackBuffer(value);
    }, []),
    watch: useCallback(
      (id: string, enabled: boolean) => controller.current?.watch(id, enabled),
      [],
    ),
    toggle: useCallback(
      (kind: StreamKind) => controller.current?.toggle(kind),
      [],
    ),
    disconnect: useCallback(async () => {
      await stop.current?.();
      setSnapshot({ ...emptyMediaSnapshot, connection: "disconnected" });
    }, []),
    reconnect: useCallback(() => setAttempt((value) => value + 1), []),
  };
}
