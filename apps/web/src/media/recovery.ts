import type { ReconnectContext } from "livekit-client";

/** One reconnect episode; an explicit user retry gets a new room/controller. */
export const reconnectPolicy = {
  nextRetryDelayInMs({
    retryCount,
    elapsedMs,
  }: ReconnectContext): number | null {
    if (retryCount >= 10 || elapsedMs >= 60_000) return null;
    return (
      Math.min(7000, 300 * retryCount * retryCount) +
      (retryCount > 1 ? Math.floor(Math.random() * 1000) : 0)
    );
  },
};
