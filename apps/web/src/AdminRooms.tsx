import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import type { AdminCreatedRoom, RoomDirectory } from "@freedomstream/contracts";
import { ApiError, request } from "./api";

export function AdminRooms({ onExpired }: { onExpired: () => void }) {
  const [directory, setDirectory] = useState<RoomDirectory | null>(null);
  const [offset, setOffset] = useState(0);
  const [refresh, setRefresh] = useState(0);
  const [name, setName] = useState("");
  const [created, setCreated] = useState<AdminCreatedRoom | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [feedback, setFeedback] = useState("");
  const mutation = useRef<AbortController | null>(null);
  useEffect(() => () => mutation.current?.abort(), []);
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    setDirectory(null);
    setError("");
    request<RoomDirectory>(`/api/admin/rooms?offset=${offset}`, {
      signal: AbortSignal.any([abort.signal, AbortSignal.timeout(10000)]),
    })
      .then((result) => {
        if (!abort.signal.aborted) setDirectory(result);
      })
      .catch((caught: unknown) => {
        if (abort.signal.aborted) return;
        if (caught instanceof ApiError && caught.status === 401) onExpired();
        else
          setError(
            caught instanceof Error
              ? caught.message
              : "Could not load rooms. Please retry.",
          );
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
    // onExpired only transitions authentication, as in the usage dashboard.
  }, [offset, refresh]);
  const create = async (event: FormEvent) => {
    event.preventDefault();
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    setError("");
    try {
      const result = await request<AdminCreatedRoom>("/api/admin/rooms", {
        method: "POST",
        body: JSON.stringify({ name }),
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      });
      if (abort.signal.aborted) return;
      setCreated(result);
      setName("");
      setFeedback("");
      setOffset(0);
      setRefresh((value) => value + 1);
    } catch (caught) {
      if (abort.signal.aborted) return;
      if (caught instanceof ApiError && caught.status === 401) onExpired();
      else
        setError(
          caught instanceof Error
            ? caught.message
            : "Could not create the room. Please retry.",
        );
    } finally {
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  return (
    <main className="admin-main">
      <div className="admin-heading">
        <div>
          <h1>Rooms</h1>
          <p>Private rooms for your friends</p>
        </div>
        <button
          disabled={loading || busy}
          onClick={() => setRefresh((value) => value + 1)}
        >
          Refresh rooms
        </button>
      </div>
      {error && (
        <p className="admin-notice" role="alert">
          {error}
        </p>
      )}
      {loading ? (
        <p role="status">Loading rooms…</p>
      ) : (
        directory && (
          <section className="admin-panel" aria-label="Room directory">
            {!directory.rooms.length ? (
              <p>No rooms on this page.</p>
            ) : (
              <ul className="admin-room-list">
                {directory.rooms.map((room) => (
                  <li key={room.id}>
                    <div>
                      <h2>{room.name}</h2>
                      <p>Created {new Date(room.createdAt).toLocaleString()}</p>
                    </div>
                    <span className="admin-status">
                      Invitation{" "}
                      {room.invitationEnabled ? "enabled" : "disabled"}
                    </span>
                  </li>
                ))}
              </ul>
            )}
            <nav className="admin-pagination" aria-label="Room pages">
              <button
                disabled={offset === 0 || busy}
                onClick={() => setOffset(Math.max(0, offset - 50))}
              >
                Previous rooms
              </button>
              <span>Page {Math.floor(offset / 50) + 1}</span>
              <button
                disabled={directory.nextOffset === null || busy}
                onClick={() => setOffset(directory.nextOffset!)}
              >
                Next rooms
              </button>
            </nav>
          </section>
        )
      )}
      <form
        className="admin-panel admin-room-create"
        onSubmit={(event) => void create(event)}
      >
        <h2>Create a room</h2>
        <label htmlFor="new-room-name">Room name</label>
        <input
          id="new-room-name"
          required
          maxLength={60}
          value={name}
          disabled={busy}
          onChange={(event) => setName(event.target.value)}
          placeholder="The usual place"
        />
        <button className="admin-primary" disabled={busy}>
          {busy ? "Creating…" : "Create room"}
        </button>
      </form>
      {created && (
        <section
          className="admin-panel admin-room-created"
          aria-label="New room invitation"
        >
          <h2>{created.room.name} is ready</h2>
          <p>
            Save this invitation in your private group chat. It is shown only
            until you leave this tab or sign out.
          </p>
          <label htmlFor="new-room-invitation">Room invitation</label>
          <div className="admin-invitation-row">
            <input
              id="new-room-invitation"
              readOnly
              value={created.invitationUrl}
              onFocus={(event) => event.currentTarget.select()}
            />
            <button
              onClick={() =>
                void navigator.clipboard
                  .writeText(created.invitationUrl)
                  .then(() => setFeedback("Invitation copied."))
                  .catch(() =>
                    setFeedback("Select the invitation above and copy it."),
                  )
              }
            >
              Copy invitation
            </button>
          </div>
          <p role="status">{feedback}</p>
        </section>
      )}
    </main>
  );
}
