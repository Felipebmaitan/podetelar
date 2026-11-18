import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
import {
  ActionGroup,
  EmptyState,
  SectionHeader,
  StatusLabel,
} from "./discord-admin-ui";
type Account = {
  userId: string;
  displayName: string;
  disabled: number;
  avatarHidden: number;
  updatedAt: number;
  loginCount: number;
};
type Login = { id: string; createdAt: number; expiresAt: number };
type AccountAction = "delete" | "disable" | "revoke";
const confirmations = {
  delete: {
    verb: "Delete stored profile",
    description:
      "The stored profile and remembered logins are removed. The user can sign in again.",
  },
  disable: {
    verb: "Disable account",
    description:
      "Their remembered logins end and future Discord sign-ins are blocked until you enable the account again.",
  },
  revoke: {
    verb: "Revoke all logins",
    description:
      "This signs the user out on all devices. They can sign in again.",
  },
};
export function AdminAccounts({ onExpired }: { onExpired: () => void }) {
  const [data, setData] = useState<{
      accounts: Account[];
      nextOffset: number | null;
    } | null>(null),
    [offset, setOffset] = useState(0),
    [revision, setRevision] = useState(0),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [selected, setSelected] = useState<{
      account: Account;
      action: AccountAction;
    } | null>(null),
    [open, setOpen] = useState<string | null>(null),
    [sessions, setSessions] = useState<Login[] | null>(null),
    [sessionRevision, setSessionRevision] = useState(0),
    [sessionError, setSessionError] = useState("");
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
        if (action === "delete") setOpen(null);
        setSessionRevision((v) => v + 1);
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
  useEffect(() => {
    if (!open) return;
    const abort = new AbortController();
    setSessionError("");
    void request<{ sessions: Login[] }>(
      `/api/admin/accounts/${open}/sessions`,
      {
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      },
    )
      .then((result) => {
        if (!abort.signal.aborted) setSessions(result.sessions);
      })
      .catch((e: unknown) => {
        if (abort.signal.aborted) return;
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setSessionError("Could not load account logins.");
      });
    return () => abort.abort();
  }, [open, sessionRevision]);
  const confirmation = selected && confirmations[selected.action];
  const changePage = (next: number) => {
    setOpen(null);
    setSessions(null);
    setOffset(next);
  };
  return (
    <section className="discord-view">
      <SectionHeader
        title="Discord accounts"
        description="Manage remembered sign-ins and Discord profiles."
      />
      {error && !selected && (
        <p className="discord-feedback" role="alert">
          {error}
        </p>
      )}
      {!data ? (
        !error && <EmptyState>Loading accounts…</EmptyState>
      ) : (
        <>
          {!data.accounts.length && (
            <EmptyState>No Discord accounts yet.</EmptyState>
          )}
          <div className="discord-records">
            {data.accounts.map((a) => {
              const expanded = open === a.userId;
              return (
                <article className="discord-record" key={a.userId}>
                  <button
                    className="discord-record-toggle"
                    aria-expanded={expanded}
                    disabled={busy}
                    onClick={() => {
                      setSessions(null);
                      setOpen(expanded ? null : a.userId);
                    }}
                  >
                    <span className="discord-initial" aria-hidden="true">
                      {Array.from(a.displayName)[0]?.toUpperCase()}
                    </span>
                    <span className="discord-record-name">
                      <strong>{a.displayName}</strong>
                      <small>
                        {a.loginCount} active{" "}
                        {a.loginCount === 1 ? "login" : "logins"}
                      </small>
                    </span>
                    <StatusLabel tone={a.disabled ? "attention" : "good"}>
                      {a.disabled ? "Disabled" : "Enabled"}
                    </StatusLabel>
                  </button>
                  {expanded && (
                    <div className="discord-record-body">
                      <p className="discord-account-id">
                        Discord user ID <code>{a.userId}</code>
                      </p>
                      <ActionGroup
                        title="Room profile"
                        description={
                          a.avatarHidden
                            ? "Discord avatar hidden."
                            : "Discord avatar visible."
                        }
                      >
                        <button
                          disabled={busy}
                          onClick={() => void act(a.userId, "refresh-profile")}
                        >
                          Refresh room profiles
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
                      </ActionGroup>
                      <ActionGroup title="Remembered logins">
                        <button
                          disabled={busy || !a.loginCount}
                          onClick={() =>
                            setSelected({ account: a, action: "revoke" })
                          }
                        >
                          Revoke all logins
                        </button>
                      </ActionGroup>
                      {sessionError && (
                        <p className="discord-feedback" role="alert">
                          {sessionError}
                        </p>
                      )}
                      {!sessions && !sessionError && (
                        <EmptyState>Loading logins…</EmptyState>
                      )}
                      {sessions?.length === 0 && (
                        <p className="discord-subtext">No active logins.</p>
                      )}
                      {Boolean(sessions?.length) && (
                        <ul className="discord-list discord-login-list">
                          {sessions?.map((session) => (
                            <li key={session.id}>
                              <span className="discord-record-name">
                                <strong>Login {session.id.slice(0, 8)}</strong>
                                <small>
                                  Created{" "}
                                  {new Date(
                                    session.createdAt,
                                  ).toLocaleDateString()}{" "}
                                  · Expires{" "}
                                  {new Date(
                                    session.expiresAt,
                                  ).toLocaleDateString()}
                                </small>
                              </span>
                              <button
                                disabled={busy}
                                onClick={() =>
                                  void act(
                                    a.userId,
                                    "revoke-session",
                                    session.id,
                                  )
                                }
                              >
                                Revoke this login
                              </button>
                            </li>
                          ))}
                        </ul>
                      )}
                      <ActionGroup title="Account access">
                        <button
                          disabled={busy}
                          onClick={() =>
                            a.disabled
                              ? void act(a.userId, "enable")
                              : setSelected({ account: a, action: "disable" })
                          }
                        >
                          {a.disabled ? "Enable account" : "Disable account"}
                        </button>
                        <button
                          className="discord-danger"
                          disabled={busy}
                          onClick={() =>
                            setSelected({ account: a, action: "delete" })
                          }
                        >
                          Delete stored profile
                        </button>
                      </ActionGroup>
                    </div>
                  )}
                </article>
              );
            })}
          </div>
          {(offset > 0 || data.nextOffset !== null) && (
            <div className="discord-actions">
              <button
                disabled={busy || !offset}
                onClick={() => changePage(Math.max(0, offset - 24))}
              >
                Previous accounts
              </button>
              <button
                disabled={busy || data.nextOffset === null}
                onClick={() =>
                  data.nextOffset !== null && changePage(data.nextOffset)
                }
              >
                Next accounts
              </button>
            </div>
          )}
        </>
      )}
      <dialog
        ref={dialog}
        className="admin-room-dialog"
        aria-labelledby="discord-account-action"
        onCancel={(event) => {
          if (busy) event.preventDefault();
          else setSelected(null);
        }}
      >
        <h2 id="discord-account-action">{confirmation?.verb}</h2>
        <p>
          <strong>{selected?.account.displayName}</strong>
        </p>
        <p>{confirmation?.description}</p>
        {error && (
          <p className="discord-feedback" role="alert">
            {error}
          </p>
        )}
        <div className="admin-dialog-actions">
          <button disabled={busy} onClick={() => setSelected(null)}>
            Cancel
          </button>
          <button
            disabled={busy}
            className="admin-danger"
            onClick={() =>
              selected && void act(selected.account.userId, selected.action)
            }
          >
            {confirmation?.verb}
          </button>
        </div>
      </dialog>
    </section>
  );
}
