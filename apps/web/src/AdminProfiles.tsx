import { useEffect, useRef, useState } from "react";
import type {
  AdminGuestProfile,
  AdminGuestProfiles,
} from "@podetelar/contracts";
import { ApiError, request } from "./api";

export function AdminProfiles({ onExpired }: { onExpired: () => void }) {
  const [data, setData] = useState<AdminGuestProfiles | null>(null);
  const [offset, setOffset] = useState(0),
    [revision, setRevision] = useState(0);
  const [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<AdminGuestProfile | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const mutation = useRef<AbortController | null>(null);
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
        const result = await request<AdminGuestProfiles>(
          `/api/admin/profiles?offset=${offset}`,
          {
            signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
          },
        );
        if (abort.signal.aborted) return;
        if (!result.profiles.length && offset) {
          setOffset(0);
          return;
        }
        setData(result);
        setError("");
      } catch (e) {
        if (abort.signal.aborted) return;
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not load guest profiles. Please retry.");
      } finally {
        pending = false;
      }
    };
    setData(null);
    void load();
    const timer = setInterval(() => void load(), 5000);
    return () => {
      abort.abort();
      clearInterval(timer);
    };
  }, [offset, revision]);
  const act = async (
    profile: AdminGuestProfile,
    action: "delete" | "block" | "allow",
  ) => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    setError("");
    try {
      await request(`/api/admin/profiles/${profile.id}`, {
        method: "POST",
        body: JSON.stringify({ action }),
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      });
      if (!abort.signal.aborted) {
        setSelected(null);
        setRevision((v) => v + 1);
      }
    } catch (e) {
      if (abort.signal.aborted) return;
      if (e instanceof ApiError && e.status === 401) onExpired();
      else
        setError(e instanceof Error ? e.message : "Could not update profile.");
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  return (
    <main className="admin-main">
      <div className="admin-heading">
        <div>
          <h1>Guest profiles</h1>
          <p>Pictures and upload permissions for recognized PCs.</p>
        </div>
        <button onClick={() => setRevision((v) => v + 1)}>
          Refresh profiles
        </button>
      </div>
      {error && <p role="alert">{error}</p>}
      {!data ? (
        <p role="status">Loading guest profiles…</p>
      ) : (
        <>
          {!data.profiles.length && <p>No guest profiles yet.</p>}
          <div className="admin-profile-grid">
            {data.profiles.map((profile) => (
              <article className="admin-panel admin-profile" key={profile.id}>
                {profile.avatarRevision ? (
                  <a
                    href={`/api/admin/profiles/${profile.id}/avatar/${profile.avatarRevision}`}
                    target="_blank"
                    rel="noreferrer"
                  >
                    <img
                      alt={`${profile.displayName}'s profile picture`}
                      src={`/api/admin/profiles/${profile.id}/avatar/${profile.avatarRevision}`}
                      width="96"
                      height="96"
                    />
                  </a>
                ) : (
                  <div
                    className="admin-profile-placeholder"
                    aria-label="No profile picture"
                  >
                    {profile.displayName.slice(0, 1).toUpperCase()}
                  </div>
                )}
                <h2>{profile.displayName}</h2>
                <code>{profile.deviceId}</code>
                <p>
                  {profile.bytes
                    ? `${Math.ceil(profile.bytes / 1000)} KB · ${profile.animated ? "Animated GIF" : "Static image"}`
                    : "No stored picture"}
                </p>
                <p>{new Date(profile.updatedAt).toLocaleString()}</p>
                <p>
                  {profile.uploadsBlocked
                    ? "Uploads blocked"
                    : "Uploads allowed"}
                </p>
                <div className="admin-profile-actions">
                  <button
                    disabled={busy || !profile.avatarRevision}
                    onClick={() => setSelected(profile)}
                  >
                    Delete picture
                  </button>
                  <button
                    disabled={busy}
                    onClick={() =>
                      void act(
                        profile,
                        profile.uploadsBlocked ? "allow" : "block",
                      )
                    }
                  >
                    {profile.uploadsBlocked ? "Allow uploads" : "Block uploads"}
                  </button>
                </div>
              </article>
            ))}
          </div>
          <div className="admin-profile-actions">
            <button
              disabled={!offset}
              onClick={() => setOffset(Math.max(0, offset - 24))}
            >
              Previous profiles
            </button>
            <button
              disabled={data.nextOffset === null}
              onClick={() => setOffset(data.nextOffset!)}
            >
              Next profiles
            </button>
          </div>
        </>
      )}
      <dialog
        ref={dialog}
        className="admin-room-dialog"
        aria-labelledby="delete-profile-title"
        onCancel={() => {
          if (!busy) setSelected(null);
        }}
      >
        <h2 id="delete-profile-title">
          Delete {selected?.displayName}'s picture?
        </h2>
        <p>
          The stored picture will be removed from this PC's guest profile.
          Upload permission is managed separately.
        </p>
        <div className="admin-dialog-actions">
          <button disabled={busy} onClick={() => setSelected(null)}>
            Cancel
          </button>
          <button
            disabled={busy}
            onClick={() => selected && void act(selected, "delete")}
          >
            Confirm deletion
          </button>
        </div>
      </dialog>
    </main>
  );
}
