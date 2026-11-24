import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
import { AdminChannels } from "./AdminChannels";
import { AdminAccounts } from "./AdminAccounts";
import { AdminFollowing } from "./AdminFollowing";
import { EmptyState, SectionHeader, StatusLabel } from "./discord-admin-ui";
import "./discord-admin.css";

export const discordViews = [
  "Overview",
  "Channels",
  "Accounts",
  "Auto-join",
] as const;
export type DiscordView = (typeof discordViews)[number];
type Status = {
  configured: boolean;
  online: boolean;
  error: string | null;
  revision: number;
  installation: { url: string; guildsRestricted: boolean } | null;
  guilds: {
    id: string;
    name: string;
    available: boolean;
    channels: { id: string; name: string; members: number }[];
  }[];
};
export function AdminDiscord({
  onExpired,
  view,
  onViewChange,
}: {
  onExpired: () => void;
  view: DiscordView;
  onViewChange: (view: DiscordView) => void;
}) {
  const [data, setData] = useState<Status | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [revision, setRevision] = useState(0);
  const mutation = useRef<AbortController | null>(null);
  useEffect(() => () => mutation.current?.abort(), []);
  useEffect(() => {
    const abort = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending || document.hidden) return;
      pending = true;
      try {
        const status = await request<Status>("/api/admin/discord", {
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
        });
        if (!abort.signal.aborted) {
          setData(status);
          setError("");
        }
      } catch (e) {
        if (abort.signal.aborted) return;
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not load Discord connection status.");
      } finally {
        pending = false;
      }
    };
    void load();
    const timer = setInterval(() => void load(), 5000);
    return () => {
      abort.abort();
      clearInterval(timer);
    };
  }, [revision]);
  const reconnect = async () => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    try {
      await request("/api/admin/discord/reconnect", {
        method: "POST",
        signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
      });
      if (!abort.signal.aborted) setRevision((v) => v + 1);
    } catch (e) {
      if (!abort.signal.aborted) {
        if (e instanceof ApiError && e.status === 401) onExpired();
        else setError("Could not reconnect Discord.");
      }
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  const guilds = data?.guilds ?? [];
  const synchronized = guilds.filter((guild) => guild.available).length;
  const channelCount = guilds.reduce(
    (count, guild) => count + guild.channels.length,
    0,
  );
  return (
    <main className="admin-main discord-admin">
      <h1>Discord</h1>
      <nav className="discord-nav" aria-label="Discord administration">
        {discordViews.map((item) => (
          <button
            key={item}
            aria-pressed={view === item}
            onClick={() => onViewChange(item)}
          >
            {item}
          </button>
        ))}
      </nav>
      {view === "Overview" && (
        <section className="discord-view">
          <article className="discord-surface discord-connection">
            <SectionHeader title="Bot connection" />
            <div className="discord-connection-actions">
              <div role="status">
                <StatusLabel
                  tone={
                    data?.online
                      ? "good"
                      : data?.configured
                        ? "attention"
                        : "neutral"
                  }
                >
                  {!data
                    ? "Checking connection"
                    : !data.configured
                      ? "Bot connection not configured"
                      : data.online
                        ? "Connected to Discord"
                        : "Discord voice state unavailable"}
                </StatusLabel>
              </div>
              <button
                disabled={!data?.configured || busy}
                onClick={() => void reconnect()}
              >
                {busy ? "Reconnecting…" : "Reconnect and resynchronize"}
              </button>
            </div>
            {error && (
              <p className="discord-feedback" role="alert">
                {error}
              </p>
            )}
            {data?.error && (
              <p className="discord-feedback" role="alert">
                {data.error}
              </p>
            )}
            {data && !data.configured && (
              <p className="discord-feedback">
                Set the bot token in the server environment to enable voice
                discovery.
              </p>
            )}
            {data?.installation && (
              <p className="discord-feedback">
                <a
                  href={data.installation.url}
                  target="_blank"
                  rel="noreferrer"
                >
                  Add bot to a Discord server
                </a>
                {data.installation.guildsRestricted && (
                  <>
                    {" "}
                    Add the new server’s ID to <code>DISCORD_GUILD_IDS</code> in
                    the deployment configuration to enable PodeTelar there.
                  </>
                )}
              </p>
            )}
            {data?.configured && (
              <dl className="discord-metrics">
                <div>
                  <dt>Discord servers</dt>
                  <dd>{guilds.length}</dd>
                </div>
                <div>
                  <dt>Synchronized</dt>
                  <dd>{synchronized}</dd>
                </div>
                <div>
                  <dt>Voice channels</dt>
                  <dd>{channelCount}</dd>
                </div>
              </dl>
            )}
          </article>
          {data?.configured && (
            <section>
              <SectionHeader title="Discord servers" />
              {guilds.length ? (
                <div className="discord-records">
                  {guilds.map((guild) => (
                    <details className="discord-record" key={guild.id}>
                      <summary>
                        <span className="discord-record-name">
                          <strong>{guild.name}</strong>
                          <small>{guild.channels.length} voice channels</small>
                        </span>
                        <StatusLabel
                          tone={guild.available ? "good" : "attention"}
                        >
                          {guild.available ? "Synchronized" : "Unavailable"}
                        </StatusLabel>
                      </summary>
                      {guild.channels.length ? (
                        <ul className="discord-list">
                          {guild.channels.map((channel) => (
                            <li key={channel.id}>
                              <span>{channel.name}</span>
                              <small>
                                {guild.available
                                  ? `${channel.members} in voice`
                                  : "Presence unknown"}
                              </small>
                            </li>
                          ))}
                        </ul>
                      ) : (
                        <EmptyState>No voice channels reported.</EmptyState>
                      )}
                    </details>
                  ))}
                </div>
              ) : (
                <EmptyState>No Discord servers discovered yet.</EmptyState>
              )}
            </section>
          )}
        </section>
      )}
      {view === "Channels" && (
        <AdminChannels guilds={guilds} onExpired={onExpired} />
      )}
      {view === "Accounts" && <AdminAccounts onExpired={onExpired} />}
      {view === "Auto-join" && (
        <AdminFollowing guilds={guilds} onExpired={onExpired} />
      )}
    </main>
  );
}
