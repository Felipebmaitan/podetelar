import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
import { AdminChannels } from "./AdminChannels";
import { AdminAccounts } from "./AdminAccounts";
import { AdminFollowing } from "./AdminFollowing";
type Status = {
  configured: boolean;
  online: boolean;
  error: string | null;
  revision: number;
  guilds: {
    id: string;
    name: string;
    available: boolean;
    channels: { id: string; name: string; members: number }[];
  }[];
};
export function AdminDiscord({ onExpired }: { onExpired: () => void }) {
  const [data, setData] = useState<Status | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [revision, setRevision] = useState(0);
  const mutation = useRef<AbortController | null>(null);
  useEffect(() => () => mutation.current?.abort(), []);
  useEffect(() => {
    const abort = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending || document.hidden) return;
      pending = true;
      try {
        const status = await request<Status>("/api/admin/discord", {
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
        });
        if (!abort.signal.aborted) {
          setData(status);
          setError("");
        }
      } catch (e) {
        if (abort.signal.aborted) return;
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not load Discord connection status.");
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
  const reconnect = async () => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    try {
      await request("/api/admin/discord/reconnect", {
        method: "POST",
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      });
      if (!abort.signal.aborted) setRevision((v) => v + 1);
    } catch (e) {
      if (!abort.signal.aborted) {
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not reconnect Discord.");
      }
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  return (
    <main className="admin-main">
      <div className="admin-heading">
        <div>
          <h1>Discord</h1>
          <p>Bot connection and voice-channel discovery.</p>
        </div>
        <button
          disabled={!data?.configured || busy}
          onClick={() => void reconnect()}
        >
          Reconnect and resynchronize
        </button>
      </div>
      {error && <p role="alert">{error}</p>}
      {!data ? (
        <p role="status">Loading Discord status…</p>
      ) : (
        <>
          <p role="status">
            {!data.configured
              ? "Bot connection not configured"
              : data.online
                ? "Connected to Discord"
                : "Discord voice state unavailable"}
          </p>
          {data.error && <p role="alert">{data.error}</p>}
          {!data.configured && (
            <p>
              Configure the bot token in the server's private environment to
              enable automatic voice discovery.
            </p>
          )}
          {data.guilds.map((g) => (
            <section className="admin-panel" key={g.id}>
              <h2>{g.name}</h2>
              <p>
                {g.available ? "Synchronized" : "Unavailable"} ·{" "}
                {g.channels.length} voice channels
              </p>
              <ul>
                {g.channels.map((c) => (
                  <li key={c.id}>
                    {c.name} ·{" "}
                    {g.available ? `${c.members} in voice` : "Presence unknown"}
                  </li>
                ))}
              </ul>
            </section>
          ))}
          <AdminChannels
            observed={data.guilds.flatMap((g) =>
              g.channels.map((c) => ({
                guildId: g.id,
                channelId: c.id,
                name: `${g.name} / ${c.name}`,
              })),
            )}
            onExpired={onExpired}
          />
          <AdminAccounts onExpired={onExpired} />
          <AdminFollowing onExpired={onExpired} />
        </>
      )}
    </main>
  );
}
