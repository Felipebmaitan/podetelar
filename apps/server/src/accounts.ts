import { createHash, randomBytes, randomUUID } from "node:crypto";
import { DatabaseSync } from "node:sqlite";
import { z } from "zod";
import { secretsEqual } from "./rooms.js";
const hash = (value: string) =>
  createHash("sha256").update(value).digest("hex");
const secret = () => randomBytes(32).toString("base64url");
export const discordUserSchema = z.object({
  id: z.string().regex(/^\d{17,20}$/),
  username: z.string().min(1).max(128),
  global_name: z.string().max(128).nullable().optional(),
  avatar: z
    .string()
    .regex(/^(?:a_)?[0-9a-f]{32}$/)
    .nullable()
    .optional(),
});
export type DiscordUser = z.infer<typeof discordUserSchema>;
export type Account = {
  userId: string;
  displayName: string;
  avatarUrl: string | null;
  sessionId: string;
  expiresAt: number;
};
type Flow = {
  id: string;
  state: string;
  challenge: string;
  expires: number;
  browserHash: string | null;
  status: "pending" | "exchanging" | "ready" | "failed";
  user?: DiscordUser;
};
export class Accounts {
  private db: DatabaseSync;
  private flows = new Map<string, Flow>();
  constructor(
    path: string,
    private origin: string,
    private now = Date.now,
  ) {
    this.db = new DatabaseSync(path);
    this.db.exec(`PRAGMA foreign_keys=ON;PRAGMA busy_timeout=5000;
      CREATE TABLE IF NOT EXISTS discord_accounts(user_id TEXT PRIMARY KEY,display_name TEXT NOT NULL,avatar_hash TEXT,disabled INTEGER NOT NULL DEFAULT 0,avatar_hidden INTEGER NOT NULL DEFAULT 0,updated_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS discord_logins(id TEXT PRIMARY KEY,token_hash TEXT NOT NULL UNIQUE,user_id TEXT NOT NULL REFERENCES discord_accounts(user_id) ON DELETE CASCADE,issuer TEXT NOT NULL,created_at INTEGER NOT NULL,expires_at INTEGER NOT NULL);
      CREATE INDEX IF NOT EXISTS discord_logins_user ON discord_logins(user_id,expires_at);`);
  }
  start(challenge: string) {
    this.prune();
    if (this.flows.size >= 128) return null;
    const flow: Flow = {
      id: randomBytes(16).toString("base64url"),
      state: secret(),
      challenge,
      expires: this.now() + 5 * 60_000,
      browserHash: null,
      status: "pending",
    };
    this.flows.set(flow.id, flow);
    return { id: flow.id, expiresAt: flow.expires };
  }
  authorize(id: string) {
    this.prune();
    const flow = this.flows.get(id);
    if (!flow || flow.status !== "pending" || flow.browserHash) return null;
    const browser = secret();
    flow.browserHash = hash(browser);
    return { state: flow.state, browser };
  }
  consume(state: string, browser: string | undefined) {
    this.prune();
    const flow = [...this.flows.values()].find((f) => f.state === state);
    if (
      !flow ||
      flow.status !== "pending" ||
      !browser ||
      !flow.browserHash ||
      !secretsEqual(hash(browser), flow.browserHash)
    )
      return null;
    flow.status = "exchanging";
    return flow.id;
  }
  complete(id: string, user: DiscordUser | null) {
    const flow = this.flows.get(id);
    if (!flow || flow.status !== "exchanging" || flow.expires <= this.now())
      return false;
    if (!user) {
      flow.status = "failed";
      return false;
    }
    flow.user = user;
    flow.status = "ready";
    return true;
  }
  poll(id: string, proof: string) {
    this.prune();
    const flow = this.flows.get(id);
    if (!flow || !secretsEqual(hash(proof), flow.challenge))
      return { status: "denied" as const };
    if (flow.status === "failed") {
      this.flows.delete(id);
      return { status: "denied" as const };
    }
    if (flow.status !== "ready" || !flow.user)
      return { status: "pending" as const };
    this.flows.delete(id);
    const issued = this.issue(flow.user);
    return issued
      ? { status: "ready" as const, ...issued }
      : { status: "denied" as const };
  }
  flowKey(id: string, proof: string) {
    const flow = this.flows.get(id);
    return flow &&
      flow.expires > this.now() &&
      secretsEqual(hash(proof), flow.challenge)
      ? flow.id
      : null;
  }
  private issue(user: DiscordUser) {
    const name =
      [user.global_name, user.username]
        .map((n) => n?.replace(/[\p{Cc}\p{Cf}]/gu, "").trim())
        .find(Boolean)
        ?.slice(0, 60) || "Discord participant";
    this.db
      .prepare(
        `INSERT INTO discord_accounts(user_id,display_name,avatar_hash,updated_at) VALUES (?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET display_name=excluded.display_name,avatar_hash=excluded.avatar_hash,updated_at=excluded.updated_at`,
      )
      .run(user.id, name, user.avatar ?? null, this.now());
    const disabled = this.db
      .prepare("SELECT disabled FROM discord_accounts WHERE user_id=?")
      .get(user.id);
    if (disabled?.disabled) return null;
    const token = secret(),
      id = randomUUID(),
      expires = this.now() + 30 * 86400_000;
    this.db
      .prepare(
        "DELETE FROM discord_logins WHERE user_id=? AND id NOT IN (SELECT id FROM discord_logins WHERE user_id=? ORDER BY created_at DESC LIMIT 15)",
      )
      .run(user.id, user.id);
    this.db
      .prepare("INSERT INTO discord_logins VALUES (?,?,?,?,?,?)")
      .run(id, hash(token), user.id, this.origin, this.now(), expires);
    return { token, account: this.authenticate(token)! };
  }
  authenticate(token: string | undefined): Account | null {
    if (!token || !/^[-\w]{43}$/.test(token)) return null;
    const row = this.db
      .prepare(
        `SELECT a.*,l.id AS session_id,l.expires_at FROM discord_logins l JOIN discord_accounts a ON a.user_id=l.user_id WHERE l.token_hash=? AND l.issuer=? AND l.expires_at>? AND a.disabled=0`,
      )
      .get(hash(token), this.origin, this.now()) as
      | {
          user_id: string;
          display_name: string;
          avatar_hash: string | null;
          avatar_hidden: number;
          session_id: string;
          expires_at: number;
        }
      | undefined;
    if (!row) return null;
    return {
      userId: row.user_id,
      displayName: row.display_name,
      avatarUrl:
        row.avatar_hash && !row.avatar_hidden
          ? `https://cdn.discordapp.com/avatars/${row.user_id}/${row.avatar_hash}.${row.avatar_hash.startsWith("a_") ? "gif" : "png"}?size=128`
          : null,
      sessionId: row.session_id,
      expiresAt: row.expires_at,
    };
  }
  logout(token: string) {
    this.db
      .prepare("DELETE FROM discord_logins WHERE token_hash=? AND issuer=?")
      .run(hash(token), this.origin);
  }
  bySession(id: string): Account | null {
    const row = this.db
      .prepare(
        `SELECT a.*,l.id AS session_id,l.expires_at FROM discord_logins l JOIN discord_accounts a ON a.user_id=l.user_id WHERE l.id=? AND l.issuer=? AND l.expires_at>? AND a.disabled=0`,
      )
      .get(id, this.origin, this.now()) as
      | {
          user_id: string;
          display_name: string;
          avatar_hash: string | null;
          avatar_hidden: number;
          session_id: string;
          expires_at: number;
        }
      | undefined;
    if (!row) return null;
    return {
      userId: row.user_id,
      displayName: row.display_name,
      avatarUrl:
        row.avatar_hash && !row.avatar_hidden
          ? `https://cdn.discordapp.com/avatars/${row.user_id}/${row.avatar_hash}.${row.avatar_hash.startsWith("a_") ? "gif" : "png"}?size=128`
          : null,
      sessionId: row.session_id,
      expiresAt: row.expires_at,
    };
  }
  avatarAllowed(user: string) {
    return Boolean(
      this.db
        .prepare(
          "SELECT 1 FROM discord_accounts WHERE user_id=? AND disabled=0 AND avatar_hidden=0",
        )
        .get(user),
    );
  }
  sessions(user: string) {
    return this.db
      .prepare(
        "SELECT id,created_at AS createdAt,expires_at AS expiresAt FROM discord_logins WHERE user_id=? AND expires_at>? ORDER BY created_at DESC LIMIT 16",
      )
      .all(user, this.now());
  }
  revokeSession(user: string, id: string) {
    return (
      this.db
        .prepare("DELETE FROM discord_logins WHERE user_id=? AND id=?")
        .run(user, id).changes > 0
    );
  }
  directory(offset: number) {
    const rows = this.db
      .prepare(
        `SELECT a.user_id AS userId,a.display_name AS displayName,a.disabled,a.avatar_hidden AS avatarHidden,a.updated_at AS updatedAt,(SELECT COUNT(*) FROM discord_logins l WHERE l.user_id=a.user_id AND l.expires_at>?) AS loginCount FROM discord_accounts a ORDER BY a.updated_at DESC,a.user_id LIMIT 25 OFFSET ?`,
      )
      .all(this.now(), offset);
    return {
      accounts: rows.slice(0, 24),
      nextOffset: rows.length > 24 ? offset + 24 : null,
    };
  }
  manage(
    user: string,
    action:
      | "disable"
      | "enable"
      | "revoke"
      | "delete"
      | "hide-avatar"
      | "show-avatar",
  ) {
    if (
      !this.db
        .prepare("SELECT 1 FROM discord_accounts WHERE user_id=?")
        .get(user)
    )
      return false;
    if (action === "revoke" || action === "disable" || action === "delete")
      this.db.prepare("DELETE FROM discord_logins WHERE user_id=?").run(user);
    if (action === "delete")
      this.db.prepare("DELETE FROM discord_accounts WHERE user_id=?").run(user);
    if (action === "disable" || action === "enable")
      this.db
        .prepare("UPDATE discord_accounts SET disabled=? WHERE user_id=?")
        .run(Number(action === "disable"), user);
    if (action === "hide-avatar" || action === "show-avatar")
      this.db
        .prepare("UPDATE discord_accounts SET avatar_hidden=? WHERE user_id=?")
        .run(Number(action === "hide-avatar"), user);
    return true;
  }
  private prune() {
    for (const [id, flow] of this.flows)
      if (flow.expires <= this.now()) this.flows.delete(id);
    this.db
      .prepare("DELETE FROM discord_logins WHERE expires_at<=?")
      .run(this.now());
  }
  close() {
    this.flows.clear();
    this.db.close();
  }
}

export function accountBearer(header: string | undefined) {
  const match = /^Bearer ([-\w]{43})$/.exec(header ?? "");
  return match?.[1];
}
