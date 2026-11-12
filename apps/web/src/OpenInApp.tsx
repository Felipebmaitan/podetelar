import { useEffect, useState } from "react";
import { request } from "./api";

export function OpenInApp({
  roomId,
  invitation,
}: {
  roomId: string;
  invitation: string;
}) {
  const [appUrl, setAppUrl] = useState("");
  const [error, setError] = useState("");
  const [generation, setGeneration] = useState(0);
  const [expires, setExpires] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    setAppUrl("");
    setError("");
    void request<{ appUrl: string }>(`/api/rooms/${roomId}/app-link`, {
      method: "POST",
      body: JSON.stringify({ invitation }),
      signal: AbortSignal.any([controller.signal, AbortSignal.timeout(10000)]),
    })
      .then(({ appUrl }) => {
        if (controller.signal.aborted) return;
        const url = new URL(appUrl);
        if (url.protocol !== "freedomstream:" || url.hostname !== "join")
          throw new Error("Invalid app invitation.");
        setAppUrl(appUrl);
        setExpires(Date.now() + 45000);
        timer = setTimeout(() => setGeneration((value) => value + 1), 45000);
      })
      .catch((caught: unknown) => {
        if (!controller.signal.aborted)
          setError(
            caught instanceof Error
              ? caught.message
              : "Could not prepare the app invitation.",
          );
      });
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [roomId, invitation, generation]);
  return (
    <>
      {error ? (
        <>
          <button
            className="button"
            onClick={() => setGeneration((value) => value + 1)}
          >
            Retry joining room
          </button>
          <p role="alert">{error}</p>
        </>
      ) : (
        <a
          className="button"
          role="button"
          aria-disabled={!appUrl}
          href={appUrl || undefined}
          onKeyDown={(event) => {
            if (event.key === " ") {
              event.preventDefault();
              event.currentTarget.click();
            }
          }}
          onClick={(event) => {
            if (!appUrl || Date.now() >= expires) {
              event.preventDefault();
              if (appUrl) setGeneration((value) => value + 1);
              return;
            }
            // Launch synchronously while browser user activation is still available.
            event.preventDefault();
            window.location.assign(appUrl);
            setAppUrl("");
            setGeneration((value) => value + 1);
          }}
        >
          {appUrl ? "Join room in PodeTelar" : "Preparing..."}
        </a>
      )}
    </>
  );
}
