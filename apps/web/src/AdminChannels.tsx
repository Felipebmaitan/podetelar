import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
type Channel = {
  guildId: string;
  channelId: string;
  name: string;
  roomId: string | null;
  guestEnabled: number;
  autoEnabled: number;
  announcementError: number;
};
export function AdminChannels({
  observed,
  onExpired,
}: {
  observed: { guildId: string; channelId: string; name: string }[];
  onExpired: () => void;
}) {
  const [rows, setRows] = useState<Channel[]>([]),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [revision, setRevision] = useState(0),
    [invitation, setInvitation] = useState("");
  const mutation = useRef<AbortController | null>(null);
  useEffect(() => () => mutation.current?.abort(), []);
  useEffect(() => {
    const abort = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending || document.hidden) return;
      pending = true;
      try {
        const result = await request<{ channels: Channel[] }>(
          "/api/admin/discord/channels",
          {
            signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
          },
        );
        if (!abort.signal.aborted) setRows(result.channels);
      } catch (e) {
        if (!abort.signal.aborted) {
          if (e instanceof ApiError && e.status === 401) onExpired();
          else setError("Could not load channel rooms.");
        }
      } finally {
        pending = false;
      }
    };
    void load();
    const timer = setInterval(() => void load(), 5000);
    return () => {
      abort.abort();
      clearInterval(timer);
    };
  }, [revision]);
  const act = async (
    c: { guildId: string; channelId: string },
    action: string,
  ) => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    setError("");
    setInvitation("");
    try {
      const result = await request<{ invitationUrl?: string }>(
        `/api/admin/discord/channels/${c.guildId}/${c.channelId}`,
        {
          method: "POST",
          body: JSON.stringify({ action }),
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
        },
      );
      if (abort.signal.aborted) return;
      if (result?.invitationUrl) {
        try {
          await navigator.clipboard.writeText(result.invitationUrl);
        } catch {
          setInvitation(result.invitationUrl);
        }
      }
      setRevision((v) => v + 1);
    } catch (e) {
      if (!abort.signal.aborted) {
        if (e instanceof ApiError && e.status === 401) onExpired();
        else
          setError(
            e instanceof Error ? e.message : "Could not manage channel.",
          );
      }
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  const combined = [
    ...observed.map(
      (c) =>
        rows.find(
          (r) => r.guildId === c.guildId && r.channelId === c.channelId,
        ) ?? {
          ...c,
          roomId: null,
          guestEnabled: 1,
          autoEnabled: 1,
          announcementError: 0,
        },
    ),
    ...rows.filter(
      (r) =>
        !observed.some(
          (c) => c.guildId === r.guildId && c.channelId === r.channelId,
        ),
    ),
  ];
  return (
    <section>
      <h2>Channel rooms and invitations</h2>
      {error && <p role="alert">{error}</p>}
      {invitation && (
        <label>
          Copy channel invitation
          <input
            readOnly
            value={invitation}
            onFocus={(e) => e.currentTarget.select()}
          />
        </label>
      )}
      {combined.map((c) => (
        <article className="admin-panel" key={`${c.guildId}:${c.channelId}`}>
          <h3>{c.name}</h3>
          <p>
            {c.roomId ? "Room open" : "No current room"} ·{" "}
            {c.autoEnabled
              ? "Automatic joins enabled"
              : "Automatic joins paused"}{" "}
            ·{" "}
            {c.guestEnabled
              ? "Guest invitation enabled"
              : "Guest invitation disabled"}
          </p>
          <div className="admin-profile-actions">
            <button disabled={busy} onClick={() => void act(c, "invitation")}>
              Copy permanent invitation
            </button>
            <button disabled={busy} onClick={() => void act(c, "rotate")}>
              Replace invitation
            </button>
            <button
              disabled={busy}
              onClick={() => void act(c, c.guestEnabled ? "disable" : "enable")}
            >
              {c.guestEnabled
                ? "Disable guest invitation"
                : "Enable guest invitation"}
            </button>
            <button
              disabled={busy}
              onClick={() => void act(c, c.autoEnabled ? "pause" : "resume")}
            >
              {c.autoEnabled
                ? "Pause automatic joins"
                : "Resume automatic joins"}
            </button>
            <button disabled={busy} onClick={() => void act(c, "repair")}>
              Repair announcement
            </button>
          </div>
          {Boolean(c.announcementError) && (
            <p role="alert">
              Announcement unavailable. Check bot message permissions.
            </p>
          )}
        </article>
      ))}
    </section>
  );
}
