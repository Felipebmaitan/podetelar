import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
type Account = {
  userId: string;
  displayName: string;
  disabled: number;
  avatarHidden: number;
  updatedAt: number;
  loginCount: number;
};
type Login = { id: string; createdAt: number; expiresAt: number };
export function AdminAccounts({ onExpired }: { onExpired: () => void }) {
  const [data, setData] = useState<{
      accounts: Account[];
      nextOffset: number | null;
    } | null>(null),
    [offset, setOffset] = useState(0),
    [revision, setRevision] = useState(0),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [selected, setSelected] = useState<Account | null>(null),
    [sessions, setSessions] = useState<Record<string, Login[]>>({});
  const dialog = useRef<HTMLDialogElement>(null),
    mutation = useRef<AbortController | null>(null);
  useEffect(() => () => mutation.current?.abort(), []);
  useEffect(() => {
    if (selected) dialog.current?.showModal();
    else dialog.current?.close();
  }, [selected]);
  useEffect(() => {
    const abort = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending || document.hidden) return;
      pending = true;
      try {
        const value = await request<{
          accounts: Account[];
          nextOffset: number | null;
        }>(`/api/admin/accounts?offset=${offset}`, {
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
        });
        if (!abort.signal.aborted) setData(value);
      } catch (e) {
        if (abort.signal.aborted) return;
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not load Discord accounts.");
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
  }, [offset, revision]);
  const act = async (user: string, action: string, sessionId?: string) => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    setError("");
    try {
      await request(`/api/admin/accounts/${user}`, {
        method: "POST",
        body: JSON.stringify({ action, ...(sessionId ? { sessionId } : {}) }),
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      });
      if (!abort.signal.aborted) {
        setSelected(null);
        setSessions({});
        setRevision((v) => v + 1);
      }
    } catch (e) {
      if (!abort.signal.aborted) {
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not update account.");
      }
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  const inspect = async (user: string) => {
    try {
      const result = await request<{ sessions: Login[] }>(
        `/api/admin/accounts/${user}/sessions`,
        { signal: AbortSignal.timeout(15000) },
      );
      setSessions((s) => ({ ...s, [user]: result.sessions }));
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) onExpired();
      else setError("Could not load account logins.");
    }
  };
  return (
    <section>
      <h2>Discord accounts</h2>
      <p>
        Manage remembered sign-ins and profile visibility. Room access is
        managed separately.
      </p>
      {error && <p role="alert">{error}</p>}
      {!data ? (
        <p role="status">Loading accounts…</p>
      ) : (
        <>
          {!data.accounts.length && <p>No Discord accounts yet.</p>}
          {data.accounts.map((a) => (
            <article className="admin-panel" key={a.userId}>
              <h3>{a.displayName}</h3>
              <p>
                {a.userId} · {a.loginCount} active logins ·{" "}
                {a.disabled ? "Disabled" : "Enabled"}
              </p>
              <div className="admin-profile-actions">
                <button disabled={busy} onClick={() => void inspect(a.userId)}>
                  Inspect logins
                </button>
                <button
                  disabled={busy}
                  onClick={() => void act(a.userId, "refresh-profile")}
                >
                  Refresh room profiles
                </button>
                <button
                  disabled={busy}
                  onClick={() => void act(a.userId, "revoke")}
                >
                  Revoke all logins
                </button>
                <button
                  disabled={busy}
                  onClick={() =>
                    void act(a.userId, a.disabled ? "enable" : "disable")
                  }
                >
                  {a.disabled ? "Enable account" : "Disable account"}
                </button>
                <button
                  disabled={busy}
                  onClick={() =>
                    void act(
                      a.userId,
                      a.avatarHidden ? "show-avatar" : "hide-avatar",
                    )
                  }
                >
                  {a.avatarHidden
                    ? "Show Discord avatar"
                    : "Hide Discord avatar"}
                </button>
                <button disabled={busy} onClick={() => setSelected(a)}>
                  Delete stored profile
                </button>
              </div>
              {sessions[a.userId]?.map((s) => (
                <p key={s.id}>
                  Login {s.id.slice(0, 8)} ·{" "}
                  {new Date(s.createdAt).toLocaleString()} · expires{" "}
                  {new Date(s.expiresAt).toLocaleDateString()}{" "}
                  <button
                    disabled={busy}
                    onClick={() => void act(a.userId, "revoke-session", s.id)}
                  >
                    Revoke this login
                  </button>
                </p>
              ))}
            </article>
          ))}
          <div className="admin-profile-actions">
            <button
              disabled={!offset}
              onClick={() => setOffset(Math.max(0, offset - 24))}
            >
              Previous accounts
            </button>
            <button
              disabled={data.nextOffset === null}
              onClick={() => setOffset(data.nextOffset!)}
            >
              Next accounts
            </button>
          </div>
        </>
      )}
      <dialog
        ref={dialog}
        className="admin-room-dialog"
        aria-labelledby="delete-account-title"
        onCancel={() => {
          if (!busy) setSelected(null);
        }}
      >
        <h2 id="delete-account-title">
          Delete {selected?.displayName}'s stored profile?
        </h2>
        <p>
          This removes the stored profile and revokes remembered logins. Disable
          the account instead to prevent future Discord sign-ins.
        </p>
        <div className="admin-dialog-actions">
          <button disabled={busy} onClick={() => setSelected(null)}>
            Cancel
          </button>
          <button
            disabled={busy}
            onClick={() => selected && void act(selected.userId, "delete")}
          >
            Confirm profile deletion
          </button>
        </div>
      </dialog>
    </section>
  );
}
