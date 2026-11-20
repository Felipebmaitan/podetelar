import { WindowsInstallerLink } from "./WindowsInstallerLink";
import { useEffect, useRef } from "react";
import { CardBrand } from "./BrandMark";

/** Redeem in a POST body on the direct host, without sharing website cookies. */
export function DirectDownload({
  ticket,
  expired,
}: {
  ticket: string | null;
  expired: boolean;
}) {
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
        <CardBrand />
        <h1>
          {valid
            ? "Windows app download"
            : expired
              ? "Download link expired"
              : "Portable download link missing"}
        </h1>
        {valid ? (
          <>
            <p>Your download is starting.</p>
            <form ref={form} method="post" action="/api/publisher/download">
              <input type="hidden" name="ticket" value={ticket} />
              <button className="button" type="submit">
                Download Windows app
              </button>
            </form>
          </>
        ) : (
          <>
            <p>
              Reopen your room invitation for a fresh portable download, or use
              the Windows installer.
            </p>
            <WindowsInstallerLink primary />
          </>
        )}
      </section>
    </main>
  );
}
