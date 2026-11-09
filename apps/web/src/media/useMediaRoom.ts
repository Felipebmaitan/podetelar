import { useCallback, useEffect, useRef, useState } from "react";
import type { RoomSession } from "@freedomstream/contracts";
import type { MediaRoom } from "./MediaRoom";
import { emptyMediaSnapshot } from "./model";
import type { StreamKind } from "./model";

export function useMediaRoom(session: RoomSession | null) {
  const controller = useRef<MediaRoom | null>(null);
  const stop = useRef<(() => Promise<void>) | null>(null);
  const [snapshot, setSnapshot] = useState(emptyMediaSnapshot);
  const [attempt, setAttempt] = useState(0);
  const roomId = session?.room.id;
  const identity = session?.participant.id;
  useEffect(() => {
    if (!roomId || !identity) return;
    setSnapshot(emptyMediaSnapshot);
    let cancelled = false;
    let media: MediaRoom | null = null;
    let unsubscribe = () => {};
    let closing: Promise<void> | null = null;
    const close = () => {
      cancelled = true;
      unsubscribe();
      if (controller.current === media) controller.current = null;
      closing ??= media?.disconnect() ?? Promise.resolve();
      return closing;
    };
    stop.current = close;
    // Keep capture/signaling code off the unauthenticated landing page.
    void import("./MediaRoom")
      .then(({ MediaRoom }) => {
        if (cancelled) return;
        media = new MediaRoom(roomId);
        controller.current = media;
        unsubscribe = media.subscribe(setSnapshot);
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
  }, [roomId, identity, attempt]);
  return {
    snapshot,
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
