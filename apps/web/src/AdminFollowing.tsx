import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
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
export function AdminFollowing({ onExpired }: { onExpired: () => void }) {
  const [data, setData] = useState<Data | null>(null),
    [error, setError] = useState(""),
    [user, setUser] = useState(""),
    [busy, setBusy] = useState(false);
  const mutation = useRef<AbortController | null>(null);
  useEffect(() => {
    const abort = new AbortController();
    let loading = false;
    const load = async () => {
      if (loading || document.hidden) return;
      loading = true;
      try {
        const result = await request<Data>("/api/admin/discord/following", {
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(10000)]),
        });
        if (!abort.signal.aborted) setData(result);
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
    setBusy(true);
    setError("");
    try {
      const result = await request<Data>("/api/admin/discord/following", {
        method: "POST",
        body: JSON.stringify({ action, ...(userId ? { userId } : {}) }),
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      });
      if (!abort.signal.aborted) setData(result);
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
  return (
    <section className="admin-card">
      <h2>Automatic joining</h2>
      <p>
        Voice departures end only voice-controlled sessions. Invitation sessions
        remain independent. Unknown bot presence preserves existing sessions.
      </p>
      {error && <p role="alert">{error}</p>}
      {data && (
        <>
          <button
            disabled={busy}
            onClick={() => void act(data.enabled ? "disable" : "enable")}
          >
            {data.enabled
              ? "Pause automatic joining everywhere"
              : "Enable automatic joining"}
          </button>
          <p>Pausing ends current voice-controlled sessions.</p>
          <label>
            Discord user ID
            <input
              value={user}
              maxLength={20}
              onChange={(e) => setUser(e.target.value)}
            />
          </label>
          <button
            disabled={busy || !/^\d{17,20}$/.test(user)}
            onClick={() => void act("block", user)}
          >
            Pause for user
          </button>
          {data.blockedUsers.map((id) => (
            <p key={id}>
              {id}{" "}
              <button disabled={busy} onClick={() => void act("allow", id)}>
                Allow automatic joining
              </button>
            </p>
          ))}
          <h3>Voice-controlled sessions ({data.sessions.length})</h3>
          <div className="admin-table-wrap">
            <table>
              <thead>
                <tr>
                  <th>User</th>
                  <th>Voice channel</th>
                  <th>Room / participant</th>
                  <th>Action</th>
                </tr>
              </thead>
              <tbody>
                {data.sessions.map((s) => (
                  <tr key={s.participantId}>
                    <td>{s.userId}</td>
                    <td>
                      {s.guildId} / {s.channelId}
                    </td>
                    <td>
                      {s.roomId}
                      <br />
                      {s.participantId}
                    </td>
                    <td>
                      <button
                        disabled={busy}
                        onClick={() => void act("block", s.userId)}
                      >
                        End and pause for user
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
  );
}
