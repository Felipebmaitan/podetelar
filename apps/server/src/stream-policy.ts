import { DatabaseSync } from "node:sqlite";
import {
  defaultStreamPolicy,
  streamPolicySchema,
  type StreamPolicy,
} from "@podetelar/contracts";

export class StreamPolicies {
  private readonly db: DatabaseSync;
  private sfuReadAt = 0;

  constructor(path: string) {
    this.db = new DatabaseSync(path);
    this.db.exec(`
      PRAGMA busy_timeout = 5000;
      CREATE TABLE IF NOT EXISTS stream_policy (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        revision INTEGER NOT NULL,
        policy TEXT NOT NULL
      );
    `);
    this.db
      .prepare("INSERT OR IGNORE INTO stream_policy VALUES (1, 0, ?)")
      .run(JSON.stringify(defaultStreamPolicy));
  }

  current(): StreamPolicy {
    const row = this.db
      .prepare("SELECT policy FROM stream_policy WHERE id=1")
      .get();
    return streamPolicySchema.parse(JSON.parse(String(row?.policy)));
  }

  save(policy: StreamPolicy): StreamPolicy | null {
    const next = { ...policy, revision: policy.revision + 1 };
    const result = this.db
      .prepare(
        "UPDATE stream_policy SET revision=?, policy=? WHERE id=1 AND revision=?",
      )
      .run(next.revision, JSON.stringify(next), policy.revision);
    return result.changes === 1 ? next : null;
  }

  mediaCaps() {
    const { revision, maxBitrateBps, maxResolution, maxFps } = this.current();
    this.sfuReadAt = Date.now();
    return { revision, maxBitrateBps, maxResolution, maxFps };
  }

  enforcementReady() {
    return Date.now() - this.sfuReadAt < 10_000;
  }

  close(): void {
    this.db.close();
  }
}

export function hasStreamCaps(policy: StreamPolicy) {
  return (
    policy.maxBitrateBps !== null ||
    policy.maxFps !== null ||
    policy.maxResolution !== null
  );
}
