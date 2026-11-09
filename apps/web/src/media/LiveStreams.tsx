import { useEffect, useRef, useState } from "react";
import type { LiveStream, MediaSnapshot, StreamKind } from "./model";

function StreamPlayer({ stream }: { stream: LiveStream }) {
  const video = useRef<HTMLVideoElement>(null);
  const [blocked, setBlocked] = useState(false);
  const [volume, setVolume] = useState(1);
  const [muted, setMuted] = useState(false);
  useEffect(() => {
    const element = video.current;
    if (!element || !stream.video) return;
    element.srcObject = new MediaStream([
      stream.video,
      ...(stream.audio ? [stream.audio] : []),
    ]);
    void element
      .play()
      .then(() => setBlocked(false))
      .catch(() => setBlocked(true));
    return () => {
      element.pause();
      element.srcObject = null;
    };
  }, [stream.video, stream.audio]);
  useEffect(() => {
    if (video.current) video.current.volume = volume;
  }, [volume]);
  return (
    <div className="stream-playback">
      <video
        ref={video}
        autoPlay
        playsInline
        muted={stream.local || muted}
        aria-label={`${stream.publisherName} ${stream.kind} video`}
      />
      {!stream.video && (
        <div className="stream-waiting" role="status">
          Connecting to stream…
        </div>
      )}
      {blocked && (
        <button
          className="button playback-prompt"
          onClick={() => {
            void video.current
              ?.play()
              .then(() => setBlocked(false))
              .catch(() => setBlocked(true));
          }}
        >
          Play stream
        </button>
      )}
      {!stream.local && stream.audio && (
        <div className="audio-controls">
          <button
            className="text-button"
            aria-pressed={muted}
            onClick={() => setMuted((value) => !value)}
          >
            {muted ? "Unmute" : "Mute"}
          </button>
          <label>
            Volume
            <input
              aria-label={`${stream.publisherName} ${stream.kind} volume`}
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={volume}
              onChange={(event) => setVolume(Number(event.target.value))}
            />
          </label>
        </div>
      )}
    </div>
  );
}

export function LiveStreams({
  snapshot,
  watch,
  toggle,
  reconnect,
}: {
  snapshot: MediaSnapshot;
  watch: (id: string, enabled: boolean) => void;
  toggle: (kind: StreamKind) => Promise<void> | undefined;
  reconnect: () => void;
}) {
  const [focused, setFocused] = useState<string | null>(null);
  useEffect(() => {
    if (
      snapshot.connection === "connected" &&
      focused &&
      !snapshot.streams.some(
        (stream) => stream.id === focused && stream.watching,
      )
    )
      setFocused(null);
  }, [snapshot.streams, snapshot.connection, focused]);
  const active = snapshot.connection === "connected";
  const camera = snapshot.streams.some(
    (stream) => stream.local && stream.kind === "camera",
  );
  const screen = snapshot.streams.some(
    (stream) => stream.local && stream.kind === "screen",
  );
  return (
    <section className="live-room" aria-label="Live sharing">
      <div className="sharing-toolbar">
        <div>
          <div
            className="connection-status"
            role="status"
            data-testid="media-connection"
            data-state={snapshot.connection}
          >
            {snapshot.connection === "connected"
              ? "Connected"
              : snapshot.connection === "reconnecting"
                ? "Reconnecting…"
                : snapshot.connection === "connecting"
                  ? "Connecting…"
                  : "Disconnected"}
          </div>
          <p className="member-count">{snapshot.participants.length} in room</p>
        </div>
        <div className="sharing-actions">
          <button
            className="button secondary"
            disabled={!active || Boolean(snapshot.pending)}
            onClick={() => void toggle("camera")}
          >
            {snapshot.pending === "camera"
              ? "Please wait…"
              : camera
                ? "Stop camera"
                : "Share camera"}
          </button>
          <button
            className="button"
            disabled={!active || Boolean(snapshot.pending)}
            onClick={() => void toggle("screen")}
          >
            {snapshot.pending === "screen"
              ? "Please wait…"
              : screen
                ? "Stop sharing screen"
                : "Share screen"}
          </button>
        </div>
      </div>
      {snapshot.error && (
        <p className="form-error" role="alert">
          {snapshot.error}
        </p>
      )}
      {(snapshot.connection === "failed" ||
        snapshot.connection === "disconnected") && (
        <button className="button secondary" onClick={reconnect}>
          Reconnect
        </button>
      )}
      {snapshot.streams.length ? (
        <div
          className={`stream-grid${focused && snapshot.streams.some((stream) => stream.id === focused) ? " has-focus" : ""}`}
        >
          {snapshot.streams.map((stream) => (
            <article
              key={stream.id}
              className={`stream-card${focused === stream.id ? " focused" : ""}`}
              aria-label={`${stream.publisherName} ${stream.kind}`}
              data-stream-id={stream.id}
            >
              <div className="stream-title">
                <strong>
                  {stream.publisherName}
                  {stream.local ? " (you)" : ""}
                </strong>
                <span>
                  {stream.kind === "screen" ? "Screen" : "Camera"}
                  {stream.muted ? " · Paused" : " · Live"}
                </span>
              </div>
              {stream.watching ? (
                <StreamPlayer stream={stream} />
              ) : (
                <div className="stream-placeholder">
                  <span aria-hidden="true">
                    {stream.publisherName.slice(0, 1).toUpperCase()}
                  </span>
                  <p>
                    {stream.local
                      ? "Your stream is available to friends."
                      : "Choose Watch to receive this stream."}
                  </p>
                </div>
              )}
              <div className="stream-actions">
                <button
                  className="button secondary"
                  onClick={() => {
                    watch(stream.id, !stream.watching);
                    if (stream.watching && focused === stream.id)
                      setFocused(null);
                  }}
                >
                  {stream.watching
                    ? "Stop watching"
                    : stream.local
                      ? "Preview"
                      : "Watch"}
                </button>
                {stream.watching && (
                  <button
                    className="text-button"
                    aria-pressed={focused === stream.id}
                    onClick={() =>
                      setFocused((value) =>
                        value === stream.id ? null : stream.id,
                      )
                    }
                  >
                    {focused === stream.id ? "Back to grid" : "Focus"}
                  </button>
                )}
              </div>
            </article>
          ))}
        </div>
      ) : (
        <div className="room-stage">
          <span className="stage-mark" aria-hidden="true">
            #
          </span>
          <h2>Who's sharing first?</h2>
          <p>
            Share a screen or camera. Your friends choose which streams to
            watch.
          </p>
        </div>
      )}
      <p className="capture-note">
        Browser sharing only captures audio offered by your selected source.
        Microphone capture is off.
      </p>
    </section>
  );
}
