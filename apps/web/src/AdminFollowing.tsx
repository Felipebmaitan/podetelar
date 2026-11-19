import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
import { EmptyState, SectionHeader, StatusLabel } from "./discord-admin-ui";
type Data = {
  enabled: boolean;
  blockedUsers: string[];
  sessions: {
    roomId: string;
    participantId: string;
    userId: string;
    loginId: string;
    guildId: string;
    channelId: string;
  }[];
};
export function AdminFollowing({
  guilds,
  onExpired,
}: {
  guilds: {
    id: string;
    name: string;
    channels: { id: string; name: string }[];
  }[];
  onExpired: () => void;
}) {
  const [data, setData] = useState<Data | null>(null),
    [error, setError] = useState(""),
    [user, setUser] = useState(""),
    [busy, setBusy] = useState(false);
  const mutation = useRef<AbortController | null>(null);
  const generation = useRef(0);
  useEffect(() => {
    const abort = new AbortController();
    let loading = false;
    const load = async () => {
      if (loading || document.hidden || mutation.current) return;
      loading = true;
      const version = generation.current;
      try {
        const result = await request<Data>("/api/admin/discord/following", {
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(10000)]),
        });
        if (!abort.signal.aborted && version === generation.current)
          setData(result);
      } catch (e) {
        if (!abort.signal.aborted) {
          if (e instanceof ApiError && e.status === 401) onExpired();
          else setError("Could not load automatic joining.");
        }
      } finally {
        loading = false;
      }
    };
    void load();
    const timer = setInterval(() => void load(), 5000);
    return () => {
      abort.abort();
      mutation.current?.abort();
      clearInterval(timer);
    };
  }, []);
  const act = async (action: string, userId?: string) => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    generation.current++;
    setBusy(true);
    setError("");
    try {
      const result = await request<Data>("/api/admin/discord/following", {
        method: "POST",
        body: JSON.stringify({ action, ...(userId ? { userId } : {}) }),
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      });
      if (!abort.signal.aborted) {
        setData(result);
        if (action === "block") setUser("");
      }
    } catch (e) {
      if (!abort.signal.aborted) {
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not change automatic joining.");
      }
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  const servers = new Map(guilds.map((guild) => [guild.id, guild]));
  return (
    <section className="discord-view">
      <SectionHeader
        title="Automatic joining"
        description="Control how signed-in users follow Discord voice."
      />
      {error && (
        <p className="discord-feedback" role="alert">
          {error}
        </p>
      )}
      {!data && !error && <EmptyState>Loading automatic joining…</EmptyState>}
      {data && (
        <>
          <article className="discord-surface">
            <div className="discord-section-header">
              <div>
                <h3>Server policy</h3>
                <p>
                  Pausing ends current voice sessions. Invitation sessions stay
                  connected.
                </p>
              </div>
              <StatusLabel tone={data.enabled ? "good" : "attention"}>
                {data.enabled ? "Enabled" : "Paused"}
              </StatusLabel>
            </div>
            <button
              disabled={busy}
              onClick={() => void act(data.enabled ? "disable" : "enable")}
            >
              {data.enabled
                ? "Pause automatic joining everywhere"
                : "Enable automatic joining"}
            </button>
          </article>
          <article className="discord-surface">
            <h3>Paused users</h3>
            <form
              className="discord-pause-form"
              onSubmit={(event) => {
                event.preventDefault();
                if (/^\d{17,20}$/.test(user)) void act("block", user);
              }}
            >
              <label>
                Discord user ID
                <input
                  value={user}
                  inputMode="numeric"
                  maxLength={20}
                  onChange={(e) => setUser(e.target.value)}
                />
              </label>
              <button disabled={busy || !/^\d{17,20}$/.test(user)}>
                Pause for user
              </button>
            </form>
            {data.blockedUsers.length ? (
              <ul className="discord-list">
                {data.blockedUsers.map((id) => (
                  <li key={id}>
                    <code>{id}</code>
                    <button
                      disabled={busy}
                      onClick={() => void act("allow", id)}
                    >
                      Allow automatic joining
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="discord-subtext">No paused users.</p>
            )}
          </article>
          <section>
            <SectionHeader
              title={`Voice-controlled sessions (${data.sessions.length})`}
            />
            {data.sessions.length ? (
              <div className="discord-records">
                {data.sessions.map((session) => {
                  const guild = servers.get(session.guildId);
                  const channel = guild?.channels.find(
                    (item) => item.id === session.channelId,
                  );
                  return (
                    <details
                      className="discord-record"
                      key={session.participantId}
                    >
                      <summary>
                        <span className="discord-record-name">
                          <strong>
                            {channel?.name ?? `Channel ${session.channelId}`}
                          </strong>
                          <small>
                            {guild?.name ?? `Server ${session.guildId}`}
                          </small>
                          <small>
                            Discord user <code>{session.userId}</code>
                          </small>
                        </span>
                      </summary>
                      <div className="discord-record-body">
                        <dl className="discord-session-details">
                          <div>
                            <dt>Room ID</dt>
                            <dd>
                              <code>{session.roomId}</code>
                            </dd>
                          </div>
                          <div>
                            <dt>Participant ID</dt>
                            <dd>
                              <code>{session.participantId}</code>
                            </dd>
                          </div>
                        </dl>
                        <button
                          disabled={busy}
                          onClick={() => void act("block", session.userId)}
                        >
                          End and pause for user
                        </button>
                      </div>
                    </details>
                  );
                })}
              </div>
            ) : (
              <EmptyState>No active voice sessions.</EmptyState>
            )}
          </section>
        </>
      )}
    </section>
  );
}
