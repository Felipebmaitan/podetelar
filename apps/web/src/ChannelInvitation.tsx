import { useState } from "react";
import { request } from "./api";
import { OpenInApp } from "./OpenInApp";
export function ChannelInvitation({
  guildId,
  channelId,
  invitation,
}: {
  guildId: string;
  channelId: string;
  invitation?: string | undefined;
}) {
  const [room, setRoom] = useState<{ id: string; secret: string } | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const resolve = async () => {
    if (!invitation || busy) return;
    setBusy(true);
    setError("");
    try {
      const result = await request<{ invitationUrl: string }>(
        `/api/channels/${guildId}/${channelId}/resolve`,
        { method: "POST", body: JSON.stringify({ invitation }) },
      );
      const url = new URL(result.invitationUrl);
      const match = /^\/r\/([A-Za-z0-9_-]{22})$/.exec(url.pathname);
      const secret = new URLSearchParams(url.hash.slice(1)).get("invite");
      if (url.origin !== location.origin || !match || !secret)
        throw Error("Invalid channel invitation response.");
      setRoom({ id: match[1]!, secret });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not open channel room.");
    } finally {
      setBusy(false);
    }
  };
  return (
    <main className="invitation-page">
      <section className="card">
        <h1>PodeTelar</h1>
        <p>
          This invitation opens the current room for a Discord voice channel.
          Joining independently keeps you connected when you leave Discord
          voice.
        </p>
        {room ? (
          <OpenInApp roomId={room.id} invitation={room.secret} />
        ) : (
          <button
            className="button"
            disabled={!invitation || busy}
            onClick={() => void resolve()}
          >
            {busy ? "Preparing room…" : "Join independently"}
          </button>
        )}
        {!invitation && (
          <p>Open the complete invitation shared by your friends.</p>
        )}
        {error && <p role="alert">{error}</p>}
        <p>Sharing and watching start only when you choose them in the app.</p>
      </section>
    </main>
  );
}
