import { useEffect, useState } from "react";
import { request } from "./api";

export function PublisherDownload({ roomId }: { roomId: string }) {
  const [release, setRelease] = useState<{
    version: string;
    downloadUrl: string;
    sha256: string;
  } | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    setRelease(null);
    void request<NonNullable<typeof release>>(
      `/api/rooms/${roomId}/publisher`,
      {
        signal: AbortSignal.any([controller.signal, AbortSignal.timeout(5000)]),
      },
    )
      .then((value) => {
        if (!controller.signal.aborted) setRelease(value);
      })
      .catch(() => {});
    return () => controller.abort();
  }, [roomId]);
  if (!release) return null;
  return (
    <section className="invite-panel">
      <h2>Share from Windows</h2>
      <p>
        Get the desktop publisher for game audio and hardware encoding. Extract
        the ZIP, then right-click Install.ps1 and choose Run with PowerShell.
        Open FreedomStream from the Start menu and paste your room invitation.
      </p>
      <a className="button secondary" href={release.downloadUrl}>
        Download Windows publisher {release.version}
      </a>
      <details>
        <summary>Verify download</summary>
        <code>SHA-256: {release.sha256}</code>
      </details>
    </section>
  );
}
