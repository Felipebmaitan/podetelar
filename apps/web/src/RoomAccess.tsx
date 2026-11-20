import { useState } from "react";
import type {
  ChangedAccess,
  RoomAccess as AccessState,
} from "@podetelar/contracts";
import { ApiError, request } from "./api";
import { CardBrand } from "./BrandMark";

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
    } catch (error) {
      if (error instanceof ApiError && [401, 403].includes(error.status)) {
        setState(null);
        setInvitation("");
        setRevokeSessions(false);
        setFeedback("Check the server access key and try again.");
      } else {
        setFeedback(
          error instanceof ApiError
            ? error.message
            : "Could not reach the server. Try again.",
        );
      }
    } finally {
      setBusy(false);
    }
  };
  const changed = async (result: ChangedAccess, replaceInvitation = true) => {
    if (replaceInvitation) setInvitation(result.invitationUrl ?? "");
    setFeedback(
      result.pendingDisconnects
        ? "Access ended. Disconnecting active streams…"
        : "Access updated.",
    );
    await refresh();
  };
  const changeInvitation = (invitation: "rotate" | "disable") => {
    const action =
      invitation === "rotate"
        ? "Replace the current invitation?"
        : "Disable the current invitation?";
    const consequence = revokeSessions
      ? "The current link will stop working and all current sessions will end."
      : "The current link will stop working.";
    if (!window.confirm(action + " " + consequence)) return;
    void act(async () =>
      changed(
        await request<ChangedAccess>(`/api/rooms/${roomId}/access`, {
          method: "POST",
          ...options(),
          body: JSON.stringify({ invitation, revokeSessions }),
        }),
      ),
    );
  };
  return (
    <section className="panel invitation-card access-panel">
      <CardBrand />
      <h1>Manage room access</h1>
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
              setRevokeSessions(false);
              setFeedback("");
            }}
          />
        </label>
        <div className="access-form-actions">
          {!state && (
            <button className="button" disabled={busy || !key.trim()}>
              {busy ? "Loading…" : "Load access"}
            </button>
          )}
          {(state || key) && (
            <button
              type="button"
              className="button secondary"
              disabled={busy || !key}
              onClick={() => {
                setKey("");
                setState(null);
                setInvitation("");
                setFeedback("");
                setRevokeSessions(false);
              }}
            >
              Lock
            </button>
          )}
        </div>
      </form>
      {state && (
        <div className="access-section">
          <div className="access-status-row">
            <span
              className={`access-pill ${state.invitationEnabled ? "is-enabled" : "is-disabled"}`}
            >
              Invitation {state.invitationEnabled ? "enabled" : "disabled"}
            </span>
            {state.pendingDisconnects > 0 && (
              <span className="access-pill is-pending">
                {state.pendingDisconnects} pending
              </span>
            )}
          </div>
          <label className="access-check">
            <input
              type="checkbox"
              disabled={busy}
              checked={revokeSessions}
              onChange={(event) => setRevokeSessions(event.target.checked)}
            />
            <span>Also end every current session</span>
          </label>
          <div className="copy-row">
            <button
              className="button"
              disabled={busy}
              onClick={() => changeInvitation("rotate")}
            >
              Rotate invitation
            </button>
            <button
              className="button secondary"
              disabled={busy}
              onClick={() => changeInvitation("disable")}
            >
              Disable invitation
            </button>
            <button
              className="button secondary"
              disabled={busy}
              onClick={() => void act(refresh)}
            >
              Refresh
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
          <div className="access-sessions">
            <h2>Current sessions</h2>
            {state.sessions.map((session) => (
              <div className="access-session" key={session.id}>
                <span>
                  <strong>{session.displayName}</strong>
                  <small>
                    Expires {new Date(session.expiresAt).toLocaleString()}
                  </small>
                </span>
                <button
                  className="button secondary"
                  disabled={busy}
                  aria-label={`End session for ${session.displayName}`}
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
                  End session
                </button>
              </div>
            ))}
            {!state.sessions.length && <p>No active sessions.</p>}
          </div>
        </div>
      )}
      {feedback && <p role="status">{feedback}</p>}
      <a className="access-back" href={`/r/${roomId}`}>
        Return to room
      </a>
    </section>
  );
}
