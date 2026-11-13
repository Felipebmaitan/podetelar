import { DatabaseSync } from "node:sqlite";
import {
  deviceIdSchema,
  type UsageReport,
  type UsagePoint,
} from "@freedomstream/contracts";

const MINUTE = 60_000;
const RETENTION = 400 * 86400000;
const REPORT_CACHE_BYTES = 2 * 1024 * 1024;
type CachedReport = { from: number; to: number; suffix: string; bytes: number };
type Counter = {
  room: string;
  participant: string;
  direction: "incoming" | "outgoing";
  bytes: number;
};
type Baseline = { value: number; at: number };

/** Only our bounded, private SFU metric is accepted. Never expose raw scrape contents. */
export function parseUsageMetrics(body: string): {
  counters: Counter[];
  generation: number;
} {
  if (!/^freedomstream_usage_enabled 1\s*$/m.test(body))
    throw new Error("Usage metrics unavailable");
  const result: Counter[] = [];
  for (const line of body.split("\n")) {
    if (!line.startsWith("freedomstream_participant_bytes_total{")) continue;
    const match =
      /^freedomstream_participant_bytes_total\{([^}]+)\} ([0-9.e+]+)\s*$/.exec(
        line,
      );
    if (!match) throw new Error("Invalid usage counter");
    const labels = Object.fromEntries(
      [...match[1]!.matchAll(/(\w+)="([^"\\]*)"/g)].map((m) => [m[1], m[2]]),
    );
    const bytes = Number(match[2]);
    if (
      !/^freedomstream_[A-Za-z0-9_-]{22}$/.test(labels.room ?? "") ||
      !/^[0-9a-f-]{36}$/.test(labels.participant ?? "") ||
      !["incoming", "outgoing"].includes(labels.direction ?? "") ||
      !Number.isSafeInteger(bytes) ||
      bytes < 0
    )
      continue;
    result.push({
      room: labels.room!.slice(14),
      participant: labels.participant!,
      direction: labels.direction as Counter["direction"],
      bytes,
    });
  }
  const generation = Number(
    /^freedomstream_usage_started_seconds ([0-9.e+-]+)\s*$/m.exec(body)?.[1],
  );
  if (!Number.isFinite(generation) || generation <= 0)
    throw new Error("Usage generation unavailable");
  return { counters: result, generation };
}

export class Usage {
  private db: DatabaseSync;
  private baseline = new Map<string, Baseline>();
  private work: Promise<void> | undefined;
  private timer: ReturnType<typeof setInterval> | undefined;
  private lastSample: number | null = null;
  private available = false;
  private generation: number | null = null;
  private prunedAt = 0;
  private observedSince = 0;
  private reports = new Map<string, CachedReport>();
  private reportBytes = 0;
  constructor(
    path: string,
    private url: string,
    private now = Date.now,
  ) {
    this.db = new DatabaseSync(path);
    this.db.exec(`PRAGMA busy_timeout=5000;
      CREATE TABLE IF NOT EXISTS usage_users (id TEXT PRIMARY KEY, room TEXT NOT NULL, name TEXT NOT NULL, admitted_at INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS usage_minutes (minute INTEGER NOT NULL, user TEXT NOT NULL, uploaded REAL NOT NULL, downloaded REAL NOT NULL, PRIMARY KEY(minute,user));
      CREATE TABLE IF NOT EXISTS usage_coverage (minute INTEGER PRIMARY KEY, milliseconds INTEGER NOT NULL);
      CREATE INDEX IF NOT EXISTS usage_user_minutes ON usage_minutes(user,minute);`);
    // Keep byte buckets and SFU baselines keyed by session. Only reporting groups
    // sessions, so concurrent instances and counter resets cannot overwrite data.
    const columns = this.db.prepare("PRAGMA table_info(usage_users)").all();
    if (!columns.some((column) => column.name === "device_id"))
      this.db.exec("ALTER TABLE usage_users ADD COLUMN device_id TEXT");
    this.db.exec(
      "CREATE INDEX IF NOT EXISTS usage_identity ON usage_users(COALESCE(device_id,id),admitted_at)",
    );
    this.db.exec(
      "INSERT OR IGNORE INTO usage_users(id,room,name) SELECT participant_id, room_id, display_name FROM sessions;",
    );
  }
  remember(room: string, participant: string, name: string, deviceId?: string) {
    if (deviceId !== undefined) deviceIdSchema.parse(deviceId);
    this.db
      .prepare(
        "INSERT OR IGNORE INTO usage_users(id,room,name,admitted_at,device_id) VALUES (?,?,?,?,?)",
      )
      .run(participant, room, name, this.now(), deviceId ?? null);
    this.clearReports();
  }
  start() {
    void this.sample();
    this.timer = setInterval(() => void this.sample(), 5000);
    this.timer.unref();
  }
  sample() {
    if (this.work) return this.work;
    this.work = this.collect()
      .catch(() => {
        this.available = false;
      })
      .finally(() => {
        this.work = undefined;
      });
    return this.work;
  }
  private async collect() {
    const response = await fetch(this.url, {
      signal: AbortSignal.timeout(3000),
      redirect: "error",
    });
    if (!response.ok || !response.body) throw new Error("Metrics unavailable");
    const reader = response.body.getReader();
    const chunks: Uint8Array[] = [];
    let size = 0;
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        size += value.length;
        if (size > 4 * 1024 * 1024) throw new Error("Metrics limit");
        chunks.push(value);
      }
    } finally {
      await reader.cancel();
    }
    const parsed = parseUsageMetrics(Buffer.concat(chunks).toString("utf8"));
    this.record(parsed.counters, this.now(), parsed.generation);
  }
  /** Cumulative counters become deltas once. Gaps/resets establish a fresh baseline. */
  record(counters: Counter[], at: number, generation = 1) {
    const previous = this.lastSample;
    const previousPrunedAt = this.prunedAt;
    const continuous =
      this.generation === generation &&
      previous !== null &&
      at > previous &&
      at - previous <= 15000;
    const next = new Map<string, Baseline>();
    this.db.exec("BEGIN IMMEDIATE");
    try {
      for (const row of counters) {
        const key = `${row.room}/${row.participant}/${row.direction}`;
        const prior = this.baseline.get(key);
        next.set(key, { value: row.bytes, at });
        const user = this.db
          .prepare("SELECT admitted_at FROM usage_users WHERE id=? AND room=?")
          .get(row.participant, row.room) as
          { admitted_at: number } | undefined;
        const baseline =
          prior?.value ??
          (user && user.admitted_at >= this.observedSince ? 0 : undefined);
        if (
          !user ||
          !continuous ||
          baseline === undefined ||
          row.bytes < baseline
        )
          continue;
        const difference = row.bytes - baseline;
        if (difference === 0) continue;
        this.split(previous!, at, (minute, ratio) => {
          const delta = difference * ratio;
          this.db
            .prepare(
              `INSERT INTO usage_minutes VALUES (?,?,?,?) ON CONFLICT(minute,user) DO UPDATE SET uploaded=uploaded+excluded.uploaded, downloaded=downloaded+excluded.downloaded`,
            )
            .run(
              minute,
              row.participant,
              row.direction === "incoming" ? delta : 0,
              row.direction === "outgoing" ? delta : 0,
            );
        });
      }
      if (continuous)
        this.split(previous!, at, (minute, ratio) => {
          this.db
            .prepare(
              "INSERT INTO usage_coverage VALUES (?,?) ON CONFLICT(minute) DO UPDATE SET milliseconds=MIN(60000,milliseconds+excluded.milliseconds)",
            )
            .run(minute, Math.round((at - previous!) * ratio));
        });
      if (at - this.prunedAt > 3600000) {
        this.db
          .prepare("DELETE FROM usage_minutes WHERE minute < ?")
          .run(at - RETENTION);
        this.db
          .prepare("DELETE FROM usage_coverage WHERE minute < ?")
          .run(at - RETENTION);
        this.db
          .prepare(
            "DELETE FROM usage_users WHERE id NOT IN (SELECT DISTINCT user FROM usage_minutes) AND id NOT IN (SELECT participant_id FROM sessions WHERE expires_at > ?)",
          )
          .run(at);
        this.prunedAt = at;
      }
      this.db.exec("COMMIT");
    } catch (error) {
      this.clearReports();
      this.db.exec("ROLLBACK");
      throw error;
    }
    // A successful prune can change any historical range, including user names.
    // Otherwise only minute buckets touched by this observation can change.
    if (this.prunedAt !== previousPrunedAt) this.clearReports();
    else if (previous !== null) {
      const from = Math.floor(Math.min(previous, at) / MINUTE) * MINUTE;
      const to = Math.ceil(Math.max(previous, at) / MINUTE) * MINUTE;
      for (const [key, entry] of this.reports)
        if (entry.from < to && entry.to > from) this.removeReport(key);
    }
    this.baseline = next;
    this.lastSample = at;
    this.generation = generation;
    if (!continuous) this.observedSince = at;
    this.available = true;
  }
  private split(
    from: number,
    to: number,
    add: (minute: number, ratio: number) => void,
  ) {
    for (let start = from; start < to;) {
      const minute = Math.floor(start / MINUTE) * MINUTE;
      const end = Math.min(to, minute + MINUTE);
      add(minute, (end - start) / (to - from));
      start = end;
    }
  }
  report(from: number, to: number, page = 0): UsageReport {
    // Queries and UI use minute-aligned ranges; one minute is the stored resolution.
    from = Math.floor(from / MINUTE) * MINUTE;
    to = Math.ceil(to / MINUTE) * MINUTE;
    const width = Math.max(
      MINUTE,
      Math.ceil((to - from) / MINUTE / 240) * MINUTE,
    );
    const coverage = this.db
      .prepare(
        "SELECT CAST((minute-?)/? AS INTEGER) AS bucket, SUM(milliseconds) AS duration FROM usage_coverage WHERE minute>=? AND minute<? GROUP BY bucket",
      )
      .all(from, width, from, to) as { bucket: number; duration: number }[];
    const covered = new Map(coverage.map((c) => [c.bucket, c.duration]));
    const points = (user?: string): UsagePoint[] => {
      const rows = this.db
        .prepare(
          `SELECT CAST((minute-?)/? AS INTEGER) AS bucket, SUM(uploaded) AS uploaded, SUM(downloaded) AS downloaded FROM usage_minutes m JOIN usage_users u ON u.id=m.user WHERE minute>=? AND minute<? ${user ? "AND COALESCE(u.device_id,u.id)=?" : ""} GROUP BY bucket`,
        )
        .all(from, width, from, to, ...(user ? [user] : [])) as {
        bucket: number;
        uploaded: number;
        downloaded: number;
      }[];
      const values = new Map(rows.map((r) => [r.bucket, r]));
      return Array.from(
        { length: Math.ceil((to - from) / width) },
        (_, bucket) => ({
          at: from + bucket * width,
          durationMs: covered.get(bucket) ?? 0,
          uploadedBytes: values.get(bucket)?.uploaded ?? 0,
          downloadedBytes: values.get(bucket)?.downloaded ?? 0,
        }),
      );
    };
    const sum = this.db
      .prepare(
        "SELECT COALESCE(SUM(uploaded),0) AS uploadedBytes, COALESCE(SUM(downloaded),0) AS downloadedBytes, COUNT(DISTINCT COALESCE(u.device_id,u.id)) AS userCount FROM usage_minutes m JOIN usage_users u ON u.id=m.user WHERE minute>=? AND minute<?",
      )
      .get(from, to) as {
      uploadedBytes: number;
      downloadedBytes: number;
      userCount: number;
    };
    const users = this.db
      .prepare(
        "SELECT COALESCE(u.device_id,u.id) AS id, CASE WHEN u.device_id IS NULL THEN 'session' ELSE 'device' END AS identityKind, COUNT(DISTINCT u.id) AS sessionCount, SUM(m.uploaded) AS uploadedBytes, SUM(m.downloaded) AS downloadedBytes FROM usage_minutes m JOIN usage_users u ON u.id=m.user WHERE minute>=? AND minute<? GROUP BY COALESCE(u.device_id,u.id) ORDER BY SUM(m.uploaded+m.downloaded) DESC, COALESCE(u.device_id,u.id) LIMIT 24 OFFSET ?",
      )
      .all(from, to, page * 24) as {
      id: string;
      identityKind: "device" | "session";
      sessionCount: number;
      uploadedBytes: number;
      downloadedBytes: number;
    }[];
    return {
      from,
      to,
      bucketMs: width,
      lastSampleAt: this.lastSample,
      available:
        this.available &&
        this.lastSample !== null &&
        this.now() - this.lastSample < 15000,
      ...sum,
      points: points(),
      users: users.map((u) => {
        // Bound drill-down size independently of lifetime session churn. All
        // totals and graphs include every session, including those beyond 50.
        const sessions = this.db
          .prepare(
            "SELECT u.id, u.room AS roomId, u.name AS displayName, u.admitted_at AS admittedAt, SUM(m.uploaded) AS uploadedBytes, SUM(m.downloaded) AS downloadedBytes FROM usage_minutes m JOIN usage_users u ON u.id=m.user WHERE minute>=? AND minute<? AND COALESCE(u.device_id,u.id)=? GROUP BY u.id ORDER BY u.admitted_at DESC,u.id DESC LIMIT 50",
          )
          .all(from, to, u.id) as UsageReport["users"][number]["sessions"];
        return {
          ...u,
          displayName: sessions[0]!.displayName,
          sessions: sessions.map((session) => ({ ...session })),
          points: points(u.id),
        };
      }),
      page,
      retentionDays: 400,
    };
  }
  /** Cache closed history only; collector status and HTTP authorization stay live. */
  reportJson(from: number, to: number, page = 0): string {
    from = Math.floor(from / MINUTE) * MINUTE;
    to = Math.ceil(to / MINUTE) * MINUTE;
    if (to > Math.floor(this.now() / MINUTE) * MINUTE)
      return JSON.stringify(this.report(from, to, page));
    const key = `${from}/${to}/${page}`;
    let entry = this.reports.get(key);
    if (entry) {
      this.reports.delete(key);
      this.reports.set(key, entry);
    } else {
      const {
        lastSampleAt: _last,
        available: _available,
        ...stable
      } = this.report(from, to, page);
      const suffix = JSON.stringify(stable).slice(1);
      // Account conservatively for UTF-16 strings, keys and fixed entry data.
      // This bounds retained content, not the JavaScript engine's total heap.
      const bytes = (suffix.length + key.length) * 2 + 64;
      entry = { from, to, suffix, bytes };
      if (bytes <= REPORT_CACHE_BYTES) {
        while (
          this.reports.size >= 4 ||
          this.reportBytes + bytes > REPORT_CACHE_BYTES
        )
          this.removeReport(this.reports.keys().next().value!);
        this.reports.set(key, entry);
        this.reportBytes += bytes;
      }
    }
    const available =
      this.available &&
      this.lastSample !== null &&
      this.now() - this.lastSample < 15000;
    return `{"lastSampleAt":${this.lastSample},"available":${available},${entry.suffix}`;
  }
  private removeReport(key: string) {
    const entry = this.reports.get(key);
    if (entry) {
      this.reportBytes -= entry.bytes;
      this.reports.delete(key);
    }
  }
  private clearReports() {
    this.reports.clear();
    this.reportBytes = 0;
  }
  async close() {
    if (this.timer) clearInterval(this.timer);
    await this.work;
    this.clearReports();
    this.db.close();
  }
}
