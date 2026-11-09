import { useState } from "react";
import type {
  ChangedAccess,
  RoomAccess as AccessState,
} from "@freedomstream/contracts";
import { request } from "./api";

/** The operator key and new invitation exist only in this mounted form. */
export function RoomAccess({ roomId }: { roomId: string }) {
  const [key, setKey] = useState("");
  const [state, setState] = useState<AccessState | null>(null);
  const [revokeSessions, setRevokeSessions] = useState(false);
  const [invitation, setInvitation] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState("");
  const headers = { Authorization: `Bearer ${key}` };
  const options = () => ({ headers, signal: AbortSignal.timeout(15_000) });
  const refresh = async () =>
    setState(
      await request<AccessState>(`/api/rooms/${roomId}/access`, options()),
    );
  const act = async (run: () => Promise<void>) => {
    setBusy(true);
    setFeedback("");
    try {
      await run();
    } catch {
      setFeedback(
        "Room management failed. Check the server access key and connection, then retry.",
      );
    } finally {
      setBusy(false);
    }
  };
  const changed = async (result: ChangedAccess, replaceInvitation = true) => {
    if (replaceInvitation) setInvitation(result.invitationUrl ?? "");
    setFeedback(
      result.pendingDisconnects
        ? "API access ended. Live media disconnection is pending; the server is retrying. Refresh to check."
        : "Access updated. Save the new invitation in your private group chat if you rotated it.",
    );
    await refresh();
  };
  return (
    <section className="panel access-panel">
      <h1>Manage room access</h1>
      <p>
        The server access key controls invitations and sessions. Each browser or
        publisher joins as a separate session.
      </p>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void act(refresh);
        }}
      >
        <label className="field">
          <span>Server access key</span>
          <input
            type="password"
            autoComplete="off"
            required
            maxLength={256}
            disabled={busy}
            value={key}
            onChange={(event) => {
              setKey(event.target.value);
              setState(null);
              setInvitation("");
            }}
          />
        </label>
        <button className="button" disabled={busy}>
          Load access
        </button>
        <button
          type="button"
          className="button secondary"
          disabled={busy}
          onClick={() => {
            setKey("");
            setState(null);
            setInvitation("");
            setFeedback("");
          }}
        >
          Lock
        </button>
      </form>
      {state && (
        <>
          <p>
            Invitation {state.invitationEnabled ? "enabled" : "disabled"}.{" "}
            {state.pendingDisconnects} pending live disconnections.
          </p>
          <label>
            <input
              type="checkbox"
              checked={revokeSessions}
              onChange={(event) => setRevokeSessions(event.target.checked)}
            />{" "}
            Also end every current session, including yours
          </label>
          <p className="help">
            Rotating stops the old link admitting new sessions. Disabling
            prevents new joins. Current sessions stay until you end them or they
            expire after 24 hours. Someone with a still-valid invitation can
            join again after their session is revoked.
          </p>
          <div className="copy-row">
            <button
              className="button"
              disabled={busy}
              onClick={() =>
                void act(async () =>
                  changed(
                    await request<ChangedAccess>(
                      `/api/rooms/${roomId}/access`,
                      {
                        method: "POST",
                        ...options(),
                        body: JSON.stringify({
                          invitation: "rotate",
                          revokeSessions,
                        }),
                      },
                    ),
                  ),
                )
              }
            >
              Rotate invitation
            </button>
            <button
              className="button secondary"
              disabled={busy}
              onClick={() =>
                void act(async () =>
                  changed(
                    await request<ChangedAccess>(
                      `/api/rooms/${roomId}/access`,
                      {
                        method: "POST",
                        ...options(),
                        body: JSON.stringify({
                          invitation: "disable",
                          revokeSessions,
                        }),
                      },
                    ),
                  ),
                )
              }
            >
              Disable invitation
            </button>
            <button
              className="button secondary"
              disabled={busy}
              onClick={() => void act(refresh)}
            >
              Refresh access
            </button>
          </div>
          {invitation && (
            <label className="field">
              <span>New room invitation</span>
              <input
                readOnly
                value={invitation}
                onFocus={(event) => event.currentTarget.select()}
              />
            </label>
          )}
          <h2>Current sessions</h2>
          {state.sessions.map((session) => (
            <div className="access-session" key={session.id}>
              <span>
                {session.displayName}{" "}
                <small>
                  expires {new Date(session.expiresAt).toLocaleString()}
                </small>
              </span>
              <button
                className="button secondary"
                disabled={busy}
                onClick={() =>
                  void act(async () =>
                    changed(
                      await request<ChangedAccess>(
                        `/api/rooms/${roomId}/access/sessions/${session.id}`,
                        { method: "DELETE", ...options() },
                      ),
                      false,
                    ),
                  )
                }
              >
                End session for {session.displayName}
              </button>
            </div>
          ))}
          {!state.sessions.length && <p>No current sessions.</p>}
        </>
      )}
      {feedback && <p role="status">{feedback}</p>}
      <a href={`/r/${roomId}`}>Return to room</a>
    </section>
  );
}
