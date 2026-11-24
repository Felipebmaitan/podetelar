import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import type {
  AdminCreatedRoom,
  AdminRoom,
  AdminRoomDirectory,
} from "@podetelar/contracts";
import { ApiError, request } from "./api";

const presenceText = {
  active: "Active",
  waiting: "Awaiting first join",
  closing: "Closing soon",
  unavailable: "Presence unavailable",
};
type Modal = { kind: "create" } | { kind: "close" | "rotate"; room: AdminRoom };

export function AdminRooms({ onExpired }: { onExpired: () => void }) {
  const [directory, setDirectory] = useState<AdminRoomDirectory | null>(null);
  const [offset, setOffset] = useState(0);
  const [refresh, setRefresh] = useState(0);
  const [search, setSearch] = useState("");
  const [expanded, setExpanded] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [modal, setModal] = useState<Modal | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [loadError, setLoadError] = useState("");
  const [feedback, setFeedback] = useState("");
  const [fallback, setFallback] = useState<{ id: string; url: string } | null>(
    null,
  );
  const [unrecoverable, setUnrecoverable] = useState<string | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const mutation = useRef<AbortController | null>(null);
  useEffect(() => () => mutation.current?.abort(), []);
  useEffect(() => {
    if (modal && !dialog.current?.open) dialog.current?.showModal();
    else if (!modal && dialog.current?.open) dialog.current?.close();
  }, [modal]);
  useEffect(() => {
    const abort = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending || document.hidden) return;
      pending = true;
      try {
        const result = await request<AdminRoomDirectory>(
          `/api/admin/rooms?offset=${offset}`,
          {
            signal: AbortSignal.any([abort.signal, AbortSignal.timeout(35000)]),
          },
        );
        if (abort.signal.aborted) return;
        if (!result.rooms.length && offset > 0) {
          setOffset(0);
          return;
        }
        setDirectory(result);
        setLoadError("");
        setFallback((value) =>
          value && result.rooms.some((room) => room.id === value.id)
            ? value
            : null,
        );
      } catch (caught) {
        if (abort.signal.aborted) return;
        if (caught instanceof ApiError && caught.status === 401) onExpired();
        else
          setLoadError(
            "Could not refresh rooms. Retry to see their current state.",
          );
      } finally {
        pending = false;
        if (!abort.signal.aborted) setLoading(false);
      }
    };
    setLoading(true);
    void load();
    const timer = setInterval(() => void load(), 5000);
    const visible = () => {
      if (!document.hidden) void load();
    };
    document.addEventListener("visibilitychange", visible);
    return () => {
      abort.abort();
      clearInterval(timer);
      document.removeEventListener("visibilitychange", visible);
    };
  }, [offset, refresh]);

  const act = async (run: (signal: AbortSignal) => Promise<void>) => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    setError("");
    setFeedback("");
    try {
      await run(AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]));
    } catch (caught) {
      if (abort.signal.aborted) return;
      if (caught instanceof ApiError && caught.status === 401) onExpired();
      else
        setError(
          caught instanceof Error
            ? caught.message
            : "Could not complete this action. Please retry.",
        );
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  const copy = async (id: string, url: string) => {
    try {
      await navigator.clipboard.writeText(url);
      setFeedback("Invitation copied.");
      setFallback(null);
    } catch {
      setFallback({ id, url });
      setFeedback("Select the invitation below and copy it.");
    }
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!modal) return;
    void act(async (signal) => {
      if (modal.kind === "create") {
        const created = await request<AdminCreatedRoom>("/api/admin/rooms", {
          method: "POST",
          body: JSON.stringify({ name }),
          signal,
        });
        setExpanded(created.room.id);
        setName("");
        setSearch("");
        setOffset(0);
        setFeedback("Room created. Copy the invite from its expanded row.");
      } else if (modal.kind === "close") {
        const result = await request<{ pendingDisconnects: number }>(
          `/api/admin/rooms/${modal.room.id}`,
          { method: "DELETE", signal },
        );
        setFallback(null);
        setExpanded(null);
        setFeedback(
          result.pendingDisconnects
            ? "Room closed. Disconnection is being retried for remaining members."
            : "Room closed.",
        );
      } else {
        const result = await request<{ invitationUrl: string }>(
          `/api/admin/rooms/${modal.room.id}/invitation/rotate`,
          { method: "POST", signal },
        );
        setUnrecoverable(null);
        await copy(modal.room.id, result.invitationUrl);
      }
      setModal(null);
      setRefresh((value) => value + 1);
    });
  };
  const copyInvite = (room: AdminRoom) =>
    void act(async (signal) => {
      try {
        const result = await request<{ invitationUrl: string }>(
          `/api/admin/rooms/${room.id}/invitation`,
          { method: "POST", signal },
        );
        await copy(room.id, result.invitationUrl);
      } catch (caught) {
        if (caught instanceof ApiError && caught.status === 409)
          setUnrecoverable(room.id);
        throw caught;
      }
    });
  const rooms =
    directory?.rooms.filter((room) =>
      room.name.toLocaleLowerCase().includes(search.toLocaleLowerCase()),
    ) ?? [];
  const activeCount =
    directory?.rooms.filter((room) => room.presence === "active").length ?? 0;
  const onlineCount =
    directory?.rooms.reduce((count, room) => count + room.members.length, 0) ??
    0;
  return (
    <main className="admin-main admin-rooms-main">
      <div className="admin-heading">
        <div className="admin-heading-row">
          <h1>Rooms</h1>
          {directory && offset === 0 && directory.nextOffset === null && (
            <div className="admin-stat-pills">
              <span className="admin-stat-pill">
                <strong>{activeCount}</strong> active
              </span>
              <span className="admin-stat-pill">
                <strong>{onlineCount}</strong> online
              </span>
              {directory.rooms.some(
                (room) => room.presence === "unavailable",
              ) && (
                <span className="admin-stat-pill is-warn">
                  Presence degraded
                </span>
              )}
            </div>
          )}
        </div>
        <button
          className="admin-primary"
          onClick={() => {
            setModal({ kind: "create" });
            setError("");
          }}
          disabled={busy}
        >
          + New room
        </button>
      </div>
      <div className="admin-rooms-toolbar">
        <label className="admin-search">
          <span className="sr-only">Search rooms on this page</span>
          <input
            type="search"
            placeholder="Search rooms…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </label>
        <span
          className={`admin-live ${loadError ? "is-stale" : ""}`}
          role="status"
        >
          {loading ? "Syncing…" : loadError ? "Paused" : "Live"}
        </span>
      </div>
      {(error || loadError) && (
        <p className="admin-notice" role="alert">
          {error || loadError}{" "}
          <button onClick={() => setRefresh((value) => value + 1)}>
            Retry
          </button>
        </p>
      )}
      {feedback && (
        <p className="admin-room-feedback" role="status">
          {feedback}
        </p>
      )}
      <section aria-label="Room directory" className="admin-directory">
        <div className="admin-room-columns" aria-hidden="true">
          <span>Room</span>
          <span>Members</span>
          <span>Status</span>
        </div>
        {!directory && loading ? (
          <p className="admin-empty" role="status">
            Loading rooms…
          </p>
        ) : !directory ? null : !rooms.length ? (
          <div className="admin-empty">
            <h2>{search ? "No matching rooms" : "No rooms yet"}</h2>
          </div>
        ) : (
          <ul className="admin-room-accordion">
            {rooms.map((room) => {
              const open = expanded === room.id;
              return (
                <li key={room.id}>
                  <button
                    className="admin-room-toggle"
                    aria-expanded={open}
                    aria-controls={`room-${room.id}`}
                    onClick={() => {
                      setExpanded(open ? null : room.id);
                      setFallback(null);
                      setFeedback("");
                    }}
                  >
                    <span className="admin-room-title">
                      <span
                        className={`admin-chevron ${open ? "is-open" : ""}`}
                        aria-hidden="true"
                      >
                        ›
                      </span>
                      <span>
                        <strong>{room.name}</strong>
                        <small>
                          {new Date(room.createdAt).toLocaleString(undefined, {
                            month: "short",
                            day: "numeric",
                            hour: "2-digit",
                            minute: "2-digit",
                          })}
                        </small>
                      </span>
                    </span>
                    <span className="admin-room-members">
                      <span className="admin-avatars" aria-hidden="true">
                        {room.members.slice(0, 4).map((member, index) => (
                          <i key={member.id} className={`avatar-${index}`}>
                            {member.displayName.slice(0, 1).toLocaleUpperCase()}
                          </i>
                        ))}
                      </span>
                      {room.presence === "unavailable"
                        ? "Unknown"
                        : room.members.length
                          ? `${room.members.length} member${room.members.length === 1 ? "" : "s"}`
                          : "Empty"}
                    </span>
                    <span className={`admin-presence is-${room.presence}`}>
                      {presenceText[room.presence]}
                    </span>
                  </button>
                  {open && (
                    <div id={`room-${room.id}`} className="admin-room-expanded">
                      <h2>Members</h2>
                      {room.presence === "unavailable" ? (
                        <p>Could not check connected members.</p>
                      ) : room.members.length ? (
                        <ul className="admin-member-list">
                          {room.members.map((member, index) => (
                            <li key={member.id}>
                              <span
                                className={`admin-avatar avatar-${index % 4}`}
                                aria-hidden="true"
                              >
                                {member.displayName
                                  .slice(0, 1)
                                  .toLocaleUpperCase()}
                              </span>
                              <span>{member.displayName}</span>
                            </li>
                          ))}
                        </ul>
                      ) : (
                        <p>
                          {room.presence === "waiting"
                            ? "Waiting for first join."
                            : "Closing in 60s unless someone rejoins."}
                        </p>
                      )}
                      <div className="admin-room-actions">
                        <button
                          className="admin-primary"
                          disabled={busy || !room.invitationEnabled}
                          onClick={() => copyInvite(room)}
                        >
                          Copy invite
                        </button>
                        <button
                          className="admin-danger"
                          disabled={busy}
                          onClick={() => {
                            setModal({ kind: "close", room });
                            setError("");
                          }}
                        >
                          Close room
                        </button>
                      </div>
                      {unrecoverable === room.id && (
                        <button
                          disabled={busy}
                          onClick={() => setModal({ kind: "rotate", room })}
                        >
                          Create new invite
                        </button>
                      )}
                      {!room.invitationEnabled && (
                        <p>Invitations are disabled for this room.</p>
                      )}
                      {fallback?.id === room.id && (
                        <label className="admin-copy-fallback">
                          Room invitation
                          <input
                            readOnly
                            value={fallback.url}
                            onFocus={(event) => event.currentTarget.select()}
                          />
                        </label>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </section>
      {directory && (offset > 0 || directory.nextOffset !== null) && (
        <nav className="admin-pagination" aria-label="Room pages">
          <button
            disabled={!offset || loading}
            onClick={() => {
              setDirectory(null);
              setOffset(Math.max(0, offset - 50));
            }}
          >
            Previous rooms
          </button>
          <span>Page {Math.floor(offset / 50) + 1}</span>
          <button
            disabled={directory.nextOffset === null || loading}
            onClick={() => {
              setDirectory(null);
              setOffset(directory.nextOffset!);
            }}
          >
            Next rooms
          </button>
        </nav>
      )}
      <dialog
        ref={dialog}
        className="admin-room-dialog"
        onCancel={(event) => {
          if (busy) event.preventDefault();
          else setModal(null);
        }}
        onClose={() => setModal(null)}
        aria-labelledby="room-dialog-title"
      >
        {modal && (
          <form onSubmit={submit}>
            <h2 id="room-dialog-title">
              {modal.kind === "create"
                ? "New room"
                : modal.kind === "close"
                  ? `Close ${modal.room.name}?`
                  : "Replace this invitation?"}
            </h2>
            {modal.kind === "create" ? (
              <>
                <label htmlFor="new-room-name">Room name</label>
                <input
                  id="new-room-name"
                  autoFocus
                  required
                  maxLength={60}
                  value={name}
                  disabled={busy}
                  onChange={(event) => setName(event.target.value)}
                  placeholder="Friday games"
                />
                <p>
                  Ready for friends to join. Closes after the last member
                  leaves.
                </p>
              </>
            ) : (
              <p>
                {modal.kind === "close"
                  ? "Everyone will be disconnected and this invitation will stop working. This cannot be undone."
                  : "The old invitation will stop working. Current members can stay connected."}
              </p>
            )}
            {error && (
              <p role="alert" className="admin-error">
                {error}
              </p>
            )}
            <div className="admin-dialog-actions">
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setModal(null);
                  setError("");
                }}
              >
                Cancel
              </button>
              <button
                className={
                  modal.kind === "close" ? "admin-danger" : "admin-primary"
                }
                disabled={busy}
              >
                {busy
                  ? "Working…"
                  : modal.kind === "create"
                    ? "Create room"
                    : modal.kind === "close"
                      ? "Close room"
                      : "Replace and copy invite"}
              </button>
            </div>
          </form>
        )}
      </dialog>
    </main>
  );
}
