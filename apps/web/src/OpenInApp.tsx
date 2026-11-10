import { useState } from "react";
import { request } from "./api";

export function OpenInApp({
  roomId,
  invitation,
}: {
  roomId: string;
  invitation: string;
}) {
  const [appUrl, setAppUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [expires, setExpires] = useState(0);
  return (
    <section className="invite-panel app-link-panel">
      <h2>Open in FreedomStream</h2>
      <p>
        Run the portable app once and enable browser links in Room options. Then
        open your invitation here. Your browser may ask permission to launch the
        app.
      </p>
      <button
        className="button"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          setError("");
          setAppUrl("");
          void request<{ appUrl: string }>(`/api/rooms/${roomId}/app-link`, {
            method: "POST",
            body: JSON.stringify({ invitation }),
            signal: AbortSignal.timeout(10000),
          })
            .then((result) => {
              setAppUrl(result.appUrl);
              setExpires(Date.now() + 55000);
            })
            .catch((caught: unknown) =>
              setError(
                caught instanceof Error
                  ? caught.message
                  : "Could not prepare the app invitation.",
              ),
            )
            .finally(() => setBusy(false));
        }}
      >
        {busy
          ? "Preparing…"
          : appUrl
            ? "Prepare a fresh app invitation"
            : "Prepare app invitation"}
      </button>
      {appUrl && (
        <>
          <p>
            <a
              className="button"
              href={appUrl}
              onClick={(event) => {
                if (Date.now() >= expires) {
                  event.preventDefault();
                  setAppUrl("");
                  setError("App invitation expired. Prepare a fresh one.");
                }
              }}
            >
              Open in FreedomStream
            </a>
          </p>
          <p className="help">
            One use, valid for one minute. If nothing opens, run the EXE and
            enable browser links, then prepare a fresh invitation.
          </p>
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
