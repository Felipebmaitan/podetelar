import { useEffect, useRef, useState } from "react";
import { ApiError, request } from "./api";
import {
  ActionGroup,
  EmptyState,
  SectionHeader,
  StatusLabel,
} from "./discord-admin-ui";

type Guild = {
  id: string;
  name: string;
  available: boolean;
  channels: { id: string; name: string }[];
};
type Channel = {
  guildId: string;
  channelId: string;
  name: string;
  roomId: string | null;
  guestEnabled: number;
  autoEnabled: number;
  announcementError: number;
};
export function AdminChannels({
  guilds,
  onExpired,
}: {
  guilds: Guild[];
  onExpired: () => void;
}) {
  const [rows, setRows] = useState<Channel[] | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [revision, setRevision] = useState(0),
    [query, setQuery] = useState(""),
    [open, setOpen] = useState<string | null>(null),
    [feedback, setFeedback] = useState(""),
    [invitation, setInvitation] = useState("");
  const mutation = useRef<AbortController | null>(null);
  useEffect(() => () => mutation.current?.abort(), []);
  useEffect(() => {
    const abort = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending || document.hidden) return;
      pending = true;
      try {
        const result = await request<{ channels: Channel[] }>(
          "/api/admin/discord/channels",
          {
            signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
          },
        );
        if (!abort.signal.aborted) setRows(result.channels);
      } catch (e) {
        if (!abort.signal.aborted) {
          if (e instanceof ApiError && e.status === 401) onExpired();
          else setError("Could not load channel rooms.");
        }
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
  const act = async (
    c: { guildId: string; channelId: string },
    action: string,
  ) => {
    if (mutation.current) return;
    const abort = new AbortController();
    mutation.current = abort;
    setBusy(true);
    setError("");
    setInvitation("");
    setFeedback("");
    try {
      const result = await request<{ invitationUrl?: string }>(
        `/api/admin/discord/channels/${c.guildId}/${c.channelId}`,
        {
          method: "POST",
          body: JSON.stringify({ action }),
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(15000)]),
        },
      );
      if (abort.signal.aborted) return;
      if (result?.invitationUrl) {
        try {
          await navigator.clipboard.writeText(result.invitationUrl);
          if (!abort.signal.aborted) setFeedback("Invitation copied.");
        } catch {
          if (!abort.signal.aborted) setInvitation(result.invitationUrl);
        }
      }
      if (abort.signal.aborted) return;
      if (action === "repair") setFeedback("Announcement repair requested.");
      setRevision((v) => v + 1);
    } catch (e) {
      if (!abort.signal.aborted) {
        if (e instanceof ApiError && e.status === 401) onExpired();
        else
          setError(
            e instanceof Error ? e.message : "Could not manage channel.",
          );
      }
    } finally {
      mutation.current = null;
      if (!abort.signal.aborted) setBusy(false);
    }
  };
  const key = (c: { guildId: string; channelId: string }) =>
    `${c.guildId}:${c.channelId}`;
  const channels = new Map(
    (rows ?? []).map((channel) => [key(channel), channel]),
  );
  const knownGuilds = new Map(guilds.map((guild) => [guild.id, guild]));
  for (const guild of guilds) {
    for (const channel of guild.channels) {
      const ids = { guildId: guild.id, channelId: channel.id };
      const saved = channels.get(key(ids));
      channels.set(
        key(ids),
        saved
          ? { ...saved, name: channel.name }
          : {
              ...ids,
              name: channel.name,
              roomId: null,
              guestEnabled: 1,
              autoEnabled: 1,
              announcementError: 0,
            },
      );
    }
  }
  const groups = new Map<string, Channel[]>();
  const filter = query.trim().toLocaleLowerCase();
  for (const channel of channels.values()) {
    const guild = knownGuilds.get(channel.guildId);
    if (
      !`${guild?.name ?? channel.guildId} ${channel.name}`
        .toLocaleLowerCase()
        .includes(filter)
    )
      continue;
    const group = groups.get(channel.guildId) ?? [];
    group.push(channel);
    groups.set(channel.guildId, group);
  }
  return (
    <section className="discord-view">
      <SectionHeader title="Channel rooms">
        {channels.size > 0 && (
          <input
            className="discord-search"
            type="search"
            aria-label="Find a voice channel"
            placeholder="Search channels…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        )}
      </SectionHeader>
      {error && (
        <p className="discord-feedback" role="alert">
          {error}
        </p>
      )}
      {rows === null && !error && (
        <EmptyState>Loading channel rooms…</EmptyState>
      )}
      {rows !== null && !groups.size && (
        <EmptyState>
          {filter
            ? "No channels match your search."
            : "No voice channels discovered yet."}
        </EmptyState>
      )}
      {rows !== null &&
        Array.from(groups, ([guildId, group]) => (
          <section key={guildId} className="discord-channel-group">
            <h3>{knownGuilds.get(guildId)?.name ?? `Server ${guildId}`}</h3>
            <div className="discord-records">
              {group
                .sort((a, b) => a.name.localeCompare(b.name))
                .map((c) => {
                  const id = key(c),
                    expanded = open === id;
                  return (
                    <article className="discord-record" key={id}>
                      <button
                        className="discord-record-toggle"
                        aria-expanded={expanded}
                        disabled={busy}
                        onClick={() => {
                          setOpen(expanded ? null : id);
                          setInvitation("");
                          setFeedback("");
                          setError("");
                        }}
                      >
                        <span className="discord-record-name">
                          <strong>{c.name}</strong>
                        </span>
                        <StatusLabel tone={c.roomId ? "good" : "neutral"}>
                          {c.roomId ? "Room open" : "Idle"}
                        </StatusLabel>
                      </button>
                      {expanded && (
                        <div className="discord-record-body">
                          <ActionGroup
                            title="Guest invitation"
                            description={
                              c.guestEnabled ? undefined : "Disabled"
                            }
                          >
                            <button
                              className="discord-primary"
                              disabled={busy || !c.guestEnabled}
                              onClick={() => void act(c, "invitation")}
                            >
                              Copy invitation
                            </button>
                            <button
                              disabled={busy}
                              onClick={() => void act(c, "rotate")}
                            >
                              {c.guestEnabled
                                ? "Replace link"
                                : "Replace and enable link"}
                            </button>
                            <button
                              disabled={busy}
                              onClick={() =>
                                void act(
                                  c,
                                  c.guestEnabled ? "disable" : "enable",
                                )
                              }
                            >
                              {c.guestEnabled ? "Disable link" : "Enable link"}
                            </button>
                          </ActionGroup>
                          {invitation && (
                            <label className="discord-invite">
                              Select and copy the invitation
                              <input
                                readOnly
                                value={invitation}
                                onFocus={(e) => e.currentTarget.select()}
                              />
                            </label>
                          )}
                          <ActionGroup
                            title="Automatic joining"
                            description={c.autoEnabled ? undefined : "Paused"}
                          >
                            <button
                              disabled={busy}
                              onClick={() =>
                                void act(c, c.autoEnabled ? "pause" : "resume")
                              }
                            >
                              {c.autoEnabled
                                ? "Pause automatic joins"
                                : "Resume automatic joins"}
                            </button>
                          </ActionGroup>
                          <ActionGroup
                            title="Discord announcement"
                            description={
                              c.announcementError
                                ? "Announcement unavailable. Check bot permissions."
                                : undefined
                            }
                          >
                            <button
                              disabled={busy}
                              onClick={() => void act(c, "repair")}
                            >
                              Repair announcement
                            </button>
                          </ActionGroup>
                          {feedback && (
                            <p className="discord-feedback" role="status">
                              {feedback}
                            </p>
                          )}
                        </div>
                      )}
                    </article>
                  );
                })}
            </div>
          </section>
        ))}
    </section>
  );
}
