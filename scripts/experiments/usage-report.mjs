// MIT. Isolated full-response query experiment; no authorization cache.
const prepared = new WeakMap();
function statements(db) {
  if (prepared.has(db)) return prepared.get(db);
  const value = {
    coverage: db.prepare(
      "SELECT CAST((minute-?)/? AS INTEGER) AS bucket, SUM(milliseconds) AS duration FROM usage_coverage WHERE minute>=? AND minute<? GROUP BY bucket",
    ),
    total: db.prepare(
      "SELECT COALESCE(SUM(uploaded),0) AS uploadedBytes, COALESCE(SUM(downloaded),0) AS downloadedBytes, COUNT(DISTINCT user) AS userCount FROM usage_minutes WHERE minute>=? AND minute<?",
    ),
    users: db.prepare(
      "SELECT u.id, u.room AS roomId, u.name AS displayName, SUM(m.uploaded) AS uploadedBytes, SUM(m.downloaded) AS downloadedBytes FROM usage_minutes m JOIN usage_users u ON u.id=m.user WHERE minute>=? AND minute<? GROUP BY u.id ORDER BY SUM(m.uploaded+m.downloaded) DESC, u.id LIMIT 24 OFFSET ?",
    ),
    global: db.prepare(
      "SELECT CAST((minute-?)/? AS INTEGER) AS bucket, SUM(uploaded) AS uploaded, SUM(downloaded) AS downloaded FROM usage_minutes WHERE minute>=? AND minute<? GROUP BY bucket",
    ),
    page: db.prepare(
      `SELECT user, CAST((minute-?)/? AS INTEGER) AS bucket, SUM(uploaded) AS uploaded, SUM(downloaded) AS downloaded FROM usage_minutes WHERE minute>=? AND minute<? AND user IN (${Array(24).fill("?").join(",")}) GROUP BY user,bucket`,
    ),
  };
  prepared.set(db, value);
  return value;
}
export function reportBatched(usage, from, to, page = 0) {
  // The existing compiled Usage class owns this connection and fresh state.
  // Its private TypeScript fields are accessed only by this benchmark module.
  const q = statements(usage.db),
    minute = 60_000;
  from = Math.floor(from / minute) * minute;
  to = Math.ceil(to / minute) * minute;
  const width = Math.max(
    minute,
    Math.ceil((to - from) / minute / 240) * minute,
  );
  const covered = new Map(
    q.coverage.all(from, width, from, to).map((c) => [c.bucket, c.duration]),
  );
  const sum = q.total.get(from, to);
  const users = q.users.all(from, to, page * 24);
  const all = q.global.all(from, width, from, to);
  const byUser = new Map();
  if (users.length) {
    const ids = users.map((u) => u.id);
    while (ids.length < 24) ids.push(null);
    for (const r of q.page.all(from, width, from, to, ...ids)) {
      if (!byUser.has(r.user)) byUser.set(r.user, []);
      byUser.get(r.user).push(r);
    }
  }
  const points = (rows) => {
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
  return {
    from,
    to,
    bucketMs: width,
    lastSampleAt: usage.lastSample,
    available:
      usage.available &&
      usage.lastSample !== null &&
      usage.now() - usage.lastSample < 15000,
    ...sum,
    points: points(all),
    users: users.map((u) => ({ ...u, points: points(byUser.get(u.id) ?? []) })),
    page,
    retentionDays: 400,
  };
}
