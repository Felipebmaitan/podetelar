import { useEffect, useRef } from "react";

/** Redeem in a POST body on the direct host, without sharing website cookies. */
export function DirectDownload({ ticket }: { ticket: string | null }) {
  const form = useRef<HTMLFormElement>(null);
  const submitted = useRef(false);
  const valid = ticket !== null && /^[A-Za-z0-9_-]{43}$/.test(ticket);
  useEffect(() => {
    if (valid && !submitted.current) {
      submitted.current = true;
      document
        .querySelector('meta[name="referrer"]')
        ?.setAttribute("content", "same-origin");
      form.current?.submit();
    }
  }, [valid]);
  return (
    <main className="invitation-page">
      <section className="panel invitation-card">
        <h1>Windows app download</h1>
        {valid ? (
          <>
            <p>
              Your download is starting. If it expires, return to the room and
              request a new download.
            </p>
            <form ref={form} method="post" action="/api/publisher/download">
              <input type="hidden" name="ticket" value={ticket} />
              <button className="button" type="submit">
                Download Windows app
              </button>
            </form>
          </>
        ) : (
          <p>Open a new download from your room.</p>
        )}
      </section>
    </main>
  );
}
