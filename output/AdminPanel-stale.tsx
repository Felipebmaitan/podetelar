import { useState } from "react";
import type { FormEvent } from "react";
import type { CreatedRoom, RoomDirectory } from "@freedomstream/contracts";
import { request } from "./api";
import { RoomAccess } from "./RoomAccess";

/** Admin credentials and freshly issued invitations live only in this mount. */
export function AdminPanel() {
  const [key, setKey] = useState("");
  const [directory, setDirectory] = useState<RoomDirectory | null>(null);
  const [tab, setTab] = useState<"rooms" | "access">("rooms");
  const [selected, setSelected] = useState("");
  const [name, setName] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [created, setCreated] = useState<CreatedRoom | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");
  const options = () => ({
    headers: { Authorization: `Bearer ${key}` },
    signal: AbortSignal.timeout(15_000),
  });
  const load = async (offset = 0) => {
    const result = await request<RoomDirectory>(
      `/api/rooms?offset=${offset}`,
      options(),
    );
    setDirectory(result);
  };
  const act = async (run: () => Promise<void>) => {
    setBusy(true);
    setError("");
    try {
      await run();
    } catch (caught) {
      setError(
        caught instanceof Error
          ? caught.message
          : "Could not connect. Please try again.",
      );
    } finally {
      setBusy(false);
    }
  };
  const create = (event: FormEvent) => {
    event.preventDefault();
    void act(async () => {
      const result = await request<CreatedRoom>("/api/rooms", {
        ...options(),
        method: "POST",
        body: JSON.stringify({ name, displayName }),
      });
      setCreated(result);
      setCopied("");
      setName("");
      await load();
    });
  };
  return (
    <div className="welcome-content admin-content">
      <h1>Admin panel</h1>
      {!directory ? (
        <form
          className="panel"
          onSubmit={(event) => {
            event.preventDefault();
            void act(() => load());
          }}
        >
          <p>Unlock room administration with the server access key.</p>
          <label className="field">
            <span>Server access key</span>
            <input
              required
              type="password"
              autoComplete="off"
              maxLength={256}
              disabled={busy}
              value={key}
              onChange={(event) => setKey(event.target.value)}
            />
          </label>
          <button className="button" disabled={busy}>
            {busy ? "Unlocking…" : "Unlock admin panel"}
          </button>
        </form>
      ) : (
        <>
          <div
            className="copy-row admin-tabs"
            role="tablist"
            aria-label="Administration"
          >
            <button
              className="button secondary"
              role="tab"
              aria-selected={tab === "rooms"}
              aria-controls="rooms-panel"
              id="rooms-tab"
              disabled={busy}
              onClick={() => setTab("rooms")}
            >
              Rooms
            </button>
            <button
              className="button secondary"
              role="tab"
              aria-selected={tab === "access"}
              aria-controls="access-panel"
              id="access-tab"
              disabled={!selected || busy}
              onClick={() => setTab("access")}
            >
              Room access
            </button>
          </div>
          <button
            className="button secondary"
            disabled={busy}
            onClick={() => {
              setKey("");
              setDirectory(null);
              setCreated(null);
              setSelected("");
              setTab("rooms");
              setName("");
              setDisplayName("");
              setError("");
              setCopied("");
            }}
          >
            Lock admin panel
          </button>
          {tab === "rooms" ? (
            <section
              id="rooms-panel"
              role="tabpanel"
              aria-labelledby="rooms-tab"
            >
              <h2>Rooms</h2>
              <button
                className="button secondary"
                disabled={busy}
                onClick={() => void act(() => load())}
              >
                Refresh rooms
              </button>
              {!directory.rooms.length && (
                <p>No rooms yet. Create your first room below.</p>
              )}
              <ul className="admin-rooms">
                {directory.rooms.map((room) => (
                  <li key={room.id}>
                    <div>
                      <strong>{room.name}</strong>
                      <p>
                        Invitation{" "}
                        {room.invitationEnabled ? "enabled" : "disabled"} ·
                        Created {new Date(room.createdAt).toLocaleString()}
                      </p>
                    </div>
                    <button
                      className="button secondary"
                      disabled={busy}
                      onClick={() => {
                        setSelected(room.id);
                        setTab("access");
                      }}
                    >
                      Manage access for {room.name}
                    </button>
                  </li>
                ))}
              </ul>
              {directory.nextOffset !== null && (
                <button
                  className="button secondary"
                  disabled={busy}
                  onClick={() => void act(() => load(directory.nextOffset!))}
                >
                  Next rooms
                </button>
              )}
              <form className="panel" onSubmit={create}>
                <h2>Create a room</h2>
                <label className="field">
                  <span>Room name</span>
                  <input
                    required
                    maxLength={60}
                    value={name}
                    disabled={busy}
                    onChange={(event) => setName(event.target.value)}
                  />
                </label>
                <label className="field">
                  <span>Your name</span>
                  <input
                    required
                    maxLength={60}
                    autoComplete="nickname"
                    value={displayName}
                    disabled={busy}
                    onChange={(event) => setDisplayName(event.target.value)}
                  />
                </label>
                <button className="button" disabled={busy}>
                  {busy ? "Working…" : "Create private room"}
                </button>
              </form>
              {created && (
                <section className="panel" aria-label="Created room">
                  <h2>{created.room.name} is ready</h2>
                  <p>
                    Save this invitation in your private group chat. It is shown
                    only until you leave or lock this panel.
                  </p>
                  <label className="field">
                    <span>Room invitation</span>
                    <input
                      readOnly
                      value={created.invitationUrl}
                      onFocus={(event) => event.currentTarget.select()}
                    />
                  </label>
                  <button
                    className="button"
                    onClick={() =>
                      void navigator.clipboard
                        .writeText(created.invitationUrl)
                        .then(() => setCopied("Invitation copied."))
                        .catch(() =>
                          setCopied("Select the invitation above and copy it."),
                        )
                    }
                  >
                    Copy invite
                  </button>
                  <a href={`/r/${created.room.id}`}>Open room</a>
                  <p role="status">{copied}</p>
                </section>
              )}
            </section>
          ) : (
            <div id="access-panel" role="tabpanel" aria-labelledby="access-tab">
              <RoomAccess key={selected} roomId={selected} adminKey={key} />
            </div>
          )}
        </>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
