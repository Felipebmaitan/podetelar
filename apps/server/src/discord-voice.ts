import { Client, GatewayIntentBits, Options, Routes } from "discord.js";
import { z } from "zod";
import type { Config } from "./config.js";

const id = z.string().regex(/^\d{17,20}$/);
const channel = z.object({
  id,
  name: z.string().max(100),
  type: z.number().int(),
});
const voice = z.object({
  guild_id: id,
  user_id: id,
  channel_id: id.nullable(),
});
type Guild = {
  id: string;
  name: string;
  available: boolean;
  channels: Map<string, string>;
  voices: Map<string, string>;
};
export type VoiceLocation = {
  guildId: string;
  guildName: string;
  channelId: string;
  channelName: string;
};
export type VoiceLookup = { known: boolean; locations: VoiceLocation[] };

/** Gateway snapshots plus ordered deltas. Unknown transport/guild state never means a departure. */
export class VoiceIndex {
  private guilds = new Map<string, Guild>();
  private online = false;
  private revision = 0;
  constructor(private guildPolicy: string[] = ["*"]) {}
  connected() {
    this.online = true;
    this.revision++;
  }
  disconnected() {
    this.online = false;
    this.revision++;
  }
  apply(type: string, data: unknown) {
    if (type === "READY") {
      this.guilds.clear();
      const ready = z
        .object({ guilds: z.array(z.object({ id })).max(2500) })
        .safeParse(data);
      if (ready.success)
        for (const guild of ready.data.guilds)
          if (this.allowed(guild.id))
            this.guilds.set(guild.id, {
              id: guild.id,
              name: "Discord server",
              available: false,
              channels: new Map(),
              voices: new Map(),
            });
      this.online = false;
      this.revision++;
      return;
    }
    if (type === "GUILD_CREATE") {
      const parsed = z
        .object({
          id,
          name: z.string().max(100).optional(),
          unavailable: z.boolean().optional(),
          channels: z.array(channel).max(1000).optional(),
          voice_states: z
            .array(voice.omit({ guild_id: true }))
            .max(10000)
            .optional(),
        })
        .safeParse(data);
      if (!parsed.success || !this.allowed(parsed.data.id)) return;
      const g = parsed.data;
      if (g.unavailable) {
        const old = this.guilds.get(g.id);
        if (old) old.available = false;
        return;
      }
      if (this.guilds.size >= 2500 && !this.guilds.has(g.id)) return;
      const channels = new Map(
        (g.channels ?? [])
          .filter((c) => c.type === 2)
          .map((c) => [c.id, c.name]),
      );
      const voices = new Map(
        (g.voice_states ?? [])
          .filter((v) => v.channel_id && channels.has(v.channel_id))
          .map((v) => [v.user_id, v.channel_id!]),
      );
      this.guilds.set(g.id, {
        id: g.id,
        name: g.name ?? "Discord server",
        available: true,
        channels,
        voices,
      });
      this.revision++;
      return;
    }
    if (type === "GUILD_DELETE") {
      const parsed = z
        .object({ id, unavailable: z.boolean().optional() })
        .safeParse(data);
      if (!parsed.success) return;
      if (parsed.data.unavailable) {
        const g = this.guilds.get(parsed.data.id);
        if (g) g.available = false;
      } else this.guilds.delete(parsed.data.id);
      this.revision++;
      return;
    }
    if (type === "VOICE_STATE_UPDATE") {
      const parsed = voice.safeParse(data);
      if (!parsed.success) return;
      const v = parsed.data,
        g = this.guilds.get(v.guild_id);
      if (!g?.available) return;
      const before = g.voices.get(v.user_id);
      if (
        v.channel_id &&
        g.channels.has(v.channel_id) &&
        (g.voices.size < 10000 || g.voices.has(v.user_id))
      )
        g.voices.set(v.user_id, v.channel_id);
      else g.voices.delete(v.user_id);
      if (before !== g.voices.get(v.user_id)) this.revision++;
      return;
    }
    if (["CHANNEL_CREATE", "CHANNEL_UPDATE", "CHANNEL_DELETE"].includes(type)) {
      const parsed = channel.extend({ guild_id: id }).safeParse(data);
      if (!parsed.success) return;
      const c = parsed.data,
        g = this.guilds.get(c.guild_id);
      if (!g) return;
      if (type === "CHANNEL_DELETE" || c.type !== 2) {
        g.channels.delete(c.id);
        for (const [u, ch] of g.voices) if (ch === c.id) g.voices.delete(u);
      } else if (g.channels.size < 1000 || g.channels.has(c.id))
        g.channels.set(c.id, c.name);
      this.revision++;
    }
  }
  private allowed(guild: string) {
    return this.guildPolicy.includes("*") || this.guildPolicy.includes(guild);
  }
  lookup(user: string, guildId?: string): VoiceLookup {
    const guilds = guildId
      ? [this.guilds.get(guildId)].filter((g): g is Guild => Boolean(g))
      : [...this.guilds.values()];
    const known =
      this.online &&
      (guildId
        ? Boolean(guilds.length && guilds[0]!.available)
        : guilds.every((g) => g.available));
    const locations = known
      ? guilds.flatMap((g) => {
          const channelId = g.voices.get(user);
          return channelId
            ? [
                {
                  guildId: g.id,
                  guildName: g.name,
                  channelId,
                  channelName: g.channels.get(channelId)!,
                },
              ]
            : [];
        })
      : [];
    return { known, locations };
  }
  channel(guildId: string, channelId: string) {
    const g = this.guilds.get(guildId);
    return this.online && g?.available && g.channels.has(channelId)
      ? {
          guildId,
          guildName: g.name,
          channelId,
          channelName: g.channels.get(channelId)!,
        }
      : null;
  }
  status() {
    return {
      online: this.online,
      revision: this.revision,
      guilds: [...this.guilds.values()]
        .map((g) => ({
          id: g.id,
          name: g.name,
          available: this.online && g.available,
          channels: [...g.channels].map(([id, name]) => ({
            id,
            name,
            members: [...g.voices.values()].filter((v) => v === id).length,
          })),
        }))
        .sort((a, b) => a.name.localeCompare(b.name)),
    };
  }
}

/** Owns bot credentials and the SDK connection; dashboard snapshots never include SDK errors. */
export class DiscordVoice {
  readonly index: VoiceIndex;
  private client: Client | null = null;
  private reconnecting: Promise<void> | null = null;
  private stopped = false;
  private failure = false;
  constructor(
    private config: Config["discord"],
    private createClient?: () => Client,
  ) {
    this.index = new VoiceIndex(config?.guildIds);
  }
  status() {
    return {
      configured: Boolean(this.config?.botToken),
      error: this.failure
        ? "Discord connection unavailable. Check the private bot configuration."
        : null,
      ...this.index.status(),
    };
  }
  async start() {
    if (!this.config?.botToken || this.stopped || this.client) return;
    const client =
      this.createClient?.() ??
      new Client({
        intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildVoiceStates],
        makeCache: Options.cacheWithLimits({
          MessageManager: 0,
          GuildMemberManager: 0,
          UserManager: 0,
          PresenceManager: 0,
        }),
        rest: { timeout: 8000, retries: 1 },
      });
    this.client = client;
    client.on("raw", (packet) => {
      if (
        this.client === client &&
        "t" in packet &&
        typeof packet.t === "string"
      )
        this.index.apply(packet.t, packet.d);
    });
    client.on("clientReady", () => {
      if (this.client === client) {
        this.failure = false;
        this.index.connected();
      }
    });
    client.on("shardResume", () => {
      if (this.client === client) this.index.connected();
    });
    client.on("shardReady", () => {
      if (this.client === client) {
        this.failure = false;
        this.index.connected();
      }
    });
    client.on("shardDisconnect", () => {
      if (this.client === client) this.index.disconnected();
    });
    client.on("shardReconnecting", () => {
      if (this.client === client) this.index.disconnected();
    });
    client.on("error", () => {
      if (this.client === client) {
        this.failure = true;
        this.index.disconnected();
      }
    });
    try {
      await client.login(this.config.botToken);
    } catch {
      if (this.client === client) {
        this.failure = true;
        this.index.disconnected();
        this.client = null;
        await client.destroy();
      }
    }
  }
  reconnect() {
    if (this.reconnecting) return this.reconnecting;
    this.reconnecting = (async () => {
      const client = this.client;
      this.client = null;
      this.index.disconnected();
      if (client) await client.destroy();
      await this.start();
    })().finally(() => {
      this.reconnecting = null;
    });
    return this.reconnecting;
  }
  async close() {
    this.stopped = true;
    const client = this.client;
    this.client = null;
    this.index.disconnected();
    if (client) await client.destroy();
    await this.reconnecting;
  }
  async member(guildId: string, userId: string) {
    if (!this.client || !this.index.status().online) return null;
    try {
      const member = await this.client.rest.get(
        Routes.guildMember(guildId, userId),
      );
      const parsed = z
        .object({
          nick: z.string().nullable().optional(),
          avatar: z.string().nullable().optional(),
          user: z.object({
            id,
            username: z.string(),
            global_name: z.string().nullable().optional(),
            avatar: z.string().nullable().optional(),
          }),
        })
        .safeParse(member);
      if (!parsed.success) return null;
      const m = parsed.data;
      return {
        displayName: (m.nick || m.user.global_name || m.user.username)
          .replace(/[\p{Cc}\p{Cf}]/gu, "")
          .slice(0, 60),
        avatarUrl: m.avatar
          ? `https://cdn.discordapp.com/guilds/${guildId}/users/${userId}/avatars/${m.avatar}.${m.avatar.startsWith("a_") ? "gif" : "png"}?size=128`
          : m.user.avatar
            ? `https://cdn.discordapp.com/avatars/${userId}/${m.user.avatar}.${m.user.avatar.startsWith("a_") ? "gif" : "png"}?size=128`
            : null,
      };
    } catch {
      return null;
    }
  }
}
