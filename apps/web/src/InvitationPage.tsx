import { useEffect, useState } from "react";
import { ApiError, request } from "./api";
import { OpenInApp } from "./OpenInApp";

export function InvitationPage({
  roomId,
  invitation,
}: {
  roomId: string;
  invitation: string | undefined;
}) {
  const [members, setMembers] = useState<
    { id: string; displayName: string }[] | null
  >(null);
  const [error, setError] = useState("");
  const [denied, setDenied] = useState(false);
  useEffect(() => {
    if (!invitation) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    const refresh = async () => {
      try {
        const result = await request<{
          members: { id: string; displayName: string }[];
        }>(`/api/rooms/${roomId}/invitation-preview`, {
          method: "POST",
          body: JSON.stringify({ invitation }),
          signal: AbortSignal.any([
            controller.signal,
            AbortSignal.timeout(5000),
          ]),
        });
        if (!controller.signal.aborted) {
          setMembers(result.members);
          setError("");
        }
      } catch (caught) {
        if (controller.signal.aborted) return;
        setMembers(null);
        setError(
          caught instanceof Error
            ? caught.message
            : "Could not load room members.",
        );
        if (caught instanceof ApiError && caught.status === 404) {
          setDenied(true);
          return;
        }
      }
      if (!controller.signal.aborted)
        timer = setTimeout(() => void refresh(), 15000);
    };
    void refresh();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [roomId, invitation]);
  return (
    <main className="invitation-page">
      <section className="panel invitation-card" aria-label="Room invitation">
        {invitation && !denied ? (
          <>
            <OpenInApp roomId={roomId} invitation={invitation} />
            <h1>People in this room</h1>
            {members === null ? (
              !error && <p role="status">Loading people...</p>
            ) : members.length ? (
              <ul>
                {members.map((member) => (
                  <li key={member.id}>{member.displayName}</li>
                ))}
              </ul>
            ) : (
              <p>No one is in the room yet.</p>
            )}
          </>
        ) : (
          <p>Open a complete, current room invitation to join.</p>
        )}
        {error && <p role="alert">{error}</p>}
      </section>
    </main>
  );
}
