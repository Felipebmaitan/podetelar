import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { invitationSchema, roomIdSchema } from "@freedomstream/contracts";
import type { CreatedRoom, RoomSession } from "@freedomstream/contracts";
import { ApiError, request } from "./api";
import { useMediaRoom } from "./media/useMediaRoom";
import { LiveStreams } from "./media/LiveStreams";
import { RoomAccess } from "./RoomAccess";

const managedRoomId = /^\/manage\/([A-Za-z0-9_-]{22})$/.exec(
  window.location.pathname,
)?.[1];

const entryRoomId = /^\/r\/([A-Za-z0-9_-]{22})$/.exec(
  window.location.pathname,
)?.[1];
const invitation = new URLSearchParams(window.location.hash.slice(1)).get(
  "invite",
);
const entryInvitation = invitationSchema.safeParse(invitation).success
  ? invitation!
  : undefined;
// Keep the invitation in memory only; never leave it in history or browser storage.
if (window.location.hash)
  window.history.replaceState(
    null,
    "",
    window.location.pathname + window.location.search,
  );
const message = (error: unknown) =>
  error instanceof Error
    ? error.message
    : "Could not connect. Please try again.";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}

export function App() {
  const [session, setSession] = useState<RoomSession | null>(null);
  const [loading, setLoading] = useState(Boolean(entryRoomId));
  const [error, setError] = useState("");
  const [invitationUrl, setInvitationUrl] = useState<string | undefined>(
    undefined,
  );
  const [copyState, setCopyState] = useState("");
  const [leaving, setLeaving] = useState(false);
  const media = useMediaRoom(session);

  useEffect(() => {
    if (!session) return;
    const abort = new AbortController();
    let pending = false;
    const check = async () => {
      if (pending) return;
      pending = true;
      try {
        await request<RoomSession>(`/api/rooms/${session.room.id}/session`, {
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(5000)]),
        });
      } catch (caught) {
        if (
          !abort.signal.aborted &&
          caught instanceof ApiError &&
          caught.status === 401
        ) {
          await media.disconnect();
          setSession(null);
          setInvitationUrl(undefined);
          setError(
            "Your room access ended. Open a current invitation to join again.",
          );
        }
      } finally {
        pending = false;
      }
    };
    // Connection changes check immediately; the quiet-room poll leaves room in
    // the shared IP request budget for five friends and their preview traffic.
    void check();
    const timer = setInterval(() => void check(), 20_000);
    return () => {
      clearInterval(timer);
      abort.abort();
    };
  }, [session, media.disconnect, media.snapshot.connection]);

  useEffect(() => {
    if (!entryRoomId) return;
    const controller = new AbortController();
    request<RoomSession>(`/api/rooms/${entryRoomId}/session`, {
      signal: controller.signal,
    })
      .then((result) => {
        setSession(result);
        if (entryInvitation)
          setInvitationUrl(
            `${window.location.origin}/r/${entryRoomId}#invite=${entryInvitation}`,
          );
      })
      .catch((caught: unknown) => {
        if (
          !controller.signal.aborted &&
          !(caught instanceof ApiError && caught.status === 401)
        )
          setError(message(caught));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, []);

  const entered = (result: RoomSession, url?: string) => {
    setSession(result);
    setInvitationUrl(url);
    setError("");
    window.history.replaceState(null, "", `/r/${result.room.id}`);
  };

  const copy = async () => {
    if (!invitationUrl) return;
    try {
      await navigator.clipboard.writeText(invitationUrl);
      setCopyState("Invitation copied. You can pin it in your group chat.");
    } catch {
      setCopyState("Select the invitation below and copy it.");
    }
  };

  const leave = async () => {
    if (!session) return;
    setLeaving(true);
    try {
      await media.disconnect();
      await request<void>(`/api/rooms/${session.room.id}/leave`, {
        method: "POST",
      });
      window.location.assign("/");
    } catch (caught) {
      setError(message(caught));
      setLeaving(false);
    }
  };

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a className="brand" href="/">
          <span className="brand-mark" aria-hidden="true">
            f
          </span>
          <span>freedomstream</span>
        </a>
        <div className="room-navigation">
          <p className="eyebrow">YOUR ROOM</p>
          <div className="room-link">
            <span aria-hidden="true">#</span>
            {session?.room.name ?? "A space for friends"}
          </div>
        </div>
        {session && (
          <section className="participant-list" aria-label="People in the room">
            <p className="eyebrow">IN THIS ROOM</p>
            {media.snapshot.participants.map((participant) => (
              <div key={participant.id} className="participant-row">
                <span className="presence-dot" aria-hidden="true" />
                <span>
                  {participant.name}
                  {participant.local ? " (you)" : ""}
                </span>
              </div>
            ))}
          </section>
        )}
        <div className="sidebar-footer">
          <span className="avatar" aria-hidden="true">
            {session?.participant.displayName.slice(0, 1).toUpperCase() ?? "F"}
          </span>
          <div>
            <strong>{session?.participant.displayName ?? "Welcome"}</strong>
            <span>
              {session ? "Room access verified" : "Your screen. Your people."}
            </span>
          </div>
        </div>
      </aside>
      <main>
        <header className="topbar">
          <span>{session?.room.name ?? "Welcome to FreedomStream"}</span>
          <span className="privacy-label">Invite only</span>
        </header>
        {error && (
          <div className="error-banner" role="alert">
            {error}
          </div>
        )}
        {managedRoomId ? (
          <div className="welcome-content">
            <RoomAccess roomId={managedRoomId} />
          </div>
        ) : loading ? (
          <div className="centered" role="status">
            Opening your room…
          </div>
        ) : session ? (
          <div className="room-content">
            <div className="room-heading">
              <div>
                <p className="eyebrow">YOUR PRIVATE SPACE</p>
                <h1>{session.room.name}</h1>
              </div>
              <button
                className="button secondary"
                onClick={() => void leave()}
                disabled={leaving}
              >
                {leaving ? "Leaving…" : "Leave room"}
              </button>
            </div>
            <LiveStreams
              roomId={session.room.id}
              snapshot={media.snapshot}
              watch={media.watch}
              toggle={media.toggle}
              reconnect={media.reconnect}
              setPlaybackBuffer={media.setPlaybackBuffer}
              route={media.route}
              setRoute={media.setRoute}
            />
            <section className="invite-panel">
              <div>
                <h2>Bring your friends</h2>
                <a href={`/manage/${session.room.id}`}>Manage room access</a>
                <p>
                  {invitationUrl
                    ? "Anyone with this invitation can join. Keep it in your private group chat."
                    : "Use the invitation saved in your group chat to invite friends."}
                </p>
              </div>
              {invitationUrl && (
                <>
                  <div className="copy-row">
                    <input
                      aria-label="Room invitation"
                      readOnly
                      value={invitationUrl}
                      onFocus={(event) => event.currentTarget.select()}
                    />
                    <button className="button" onClick={() => void copy()}>
                      Copy invite
                    </button>
                  </div>
                  <p className="feedback" role="status">
                    {copyState ||
                      "This invitation stays valid when you leave the room."}
                  </p>
                </>
              )}
            </section>
          </div>
        ) : entryRoomId ? (
          <JoinRoom
            roomId={entryRoomId}
            invitation={entryInvitation}
            onJoined={entered}
          />
        ) : window.location.pathname === "/" ? (
          <Start
            onCreated={(result) => entered(result, result.invitationUrl)}
          />
        ) : (
          <div className="centered">
            <h1>That room link doesn't look right.</h1>
            <a href="/">Return to FreedomStream</a>
          </div>
        )}
      </main>
    </div>
  );
}

function Start({ onCreated }: { onCreated: (result: CreatedRoom) => void }) {
  const [roomName, setRoomName] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [accessKey, setAccessKey] = useState("");
  const [link, setLink] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const create = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      const result = await request<CreatedRoom>("/api/rooms", {
        method: "POST",
        headers: { Authorization: `Bearer ${accessKey}` },
        body: JSON.stringify({ name: roomName, displayName }),
      });
      setAccessKey("");
      onCreated(result);
    } catch (caught) {
      setError(message(caught));
    } finally {
      setBusy(false);
    }
  };
  const openInvitation = (event: FormEvent) => {
    event.preventDefault();
    try {
      const url = new URL(link);
      const roomId = /^\/r\/([^/]+)$/.exec(url.pathname)?.[1];
      const secret = new URLSearchParams(url.hash.slice(1)).get("invite");
      if (
        url.origin !== window.location.origin ||
        !roomIdSchema.safeParse(roomId).success ||
        !invitationSchema.safeParse(secret).success
      )
        throw new Error();
      window.location.assign(url.pathname + url.hash);
    } catch {
      setError("Paste a complete room invitation from this server.");
    }
  };
  return (
    <div className="welcome-content">
      <div className="welcome-heading">
        <p className="eyebrow">GET TOGETHER</p>
        <h1>A room for your people.</h1>
        <p>
          Create a private room, or open the invitation from your group chat.
        </p>
      </div>
      <div className="welcome-grid">
        <form className="panel" onSubmit={(event) => void create(event)}>
          <h2>Create a room</h2>
          <Field label="Room name">
            <input
              required
              maxLength={60}
              value={roomName}
              onChange={(event) => setRoomName(event.target.value)}
              placeholder="The usual place"
            />
          </Field>
          <Field label="Your name">
            <input
              required
              maxLength={60}
              autoComplete="nickname"
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
              placeholder="What your friends call you"
            />
          </Field>
          <Field label="Server access key">
            <input
              required
              type="password"
              autoComplete="off"
              maxLength={256}
              value={accessKey}
              onChange={(event) => setAccessKey(event.target.value)}
              placeholder="Ask your server owner"
            />
          </Field>
          <p className="help">
            Only someone with the server access key can create a room.
          </p>
          <button className="button" disabled={busy}>
            {busy ? "Creating…" : "Create private room"}
          </button>
        </form>
        <form className="panel invitation-form" onSubmit={openInvitation}>
          <span className="small-mark" aria-hidden="true">
            #
          </span>
          <h2>Already invited?</h2>
          <p>Open your invitation directly, or paste it here.</p>
          <Field label="Room invitation">
            <input
              required
              type="url"
              value={link}
              onChange={(event) => setLink(event.target.value)}
              placeholder={`${window.location.origin}/r/…#invite=…`}
            />
          </Field>
          <button className="button secondary">Open invitation</button>
        </form>
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

function JoinRoom({
  roomId,
  invitation: secret,
  onJoined,
}: {
  roomId: string;
  invitation: string | undefined;
  onJoined: (result: RoomSession, url?: string) => void;
}) {
  const [displayName, setDisplayName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const join = async (event: FormEvent) => {
    event.preventDefault();
    if (!secret) return;
    setBusy(true);
    setError("");
    try {
      const result = await request<RoomSession>(`/api/rooms/${roomId}/join`, {
        method: "POST",
        body: JSON.stringify({ displayName, invitation: secret }),
      });
      onJoined(
        result,
        `${window.location.origin}/r/${roomId}#invite=${secret}`,
      );
    } catch (caught) {
      setError(message(caught));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="centered">
      <form className="panel join-panel" onSubmit={(event) => void join(event)}>
        <p className="eyebrow">YOU'RE INVITED</p>
        <h1>Take your place.</h1>
        {secret ? (
          <>
            <p>Choose a name so your friends recognize you.</p>
            <Field label="Your name">
              <input
                required
                autoFocus
                maxLength={60}
                autoComplete="nickname"
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
              />
            </Field>
            <button className="button" disabled={busy}>
              {busy ? "Joining…" : "Join private room"}
            </button>
          </>
        ) : (
          <>
            <p>
              Open the complete invitation pinned in your group chat to join
              this room.
            </p>
            <a href="/">Back to start</a>
          </>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </form>
    </div>
  );
}
