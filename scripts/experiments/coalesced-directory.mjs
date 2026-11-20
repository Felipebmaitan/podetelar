// Rejected experiment: pending requests may predate a caller's fresh check.
// Kept executable to reproduce its performance/freshness tradeoff; never used by
// the application server. There is no completed-result cache.
export class CoalescedDirectory {
  constructor(inner) {
    this.inner = inner;
    this.pending = new Map();
  }
  async participant(room, identity) {
    const key = JSON.stringify([room, identity]);
    let pending = this.pending.get(key);
    if (!pending) {
      pending = this.inner.participant(room, identity).finally(() => {
        if (this.pending.get(key) === pending) this.pending.delete(key);
      });
      this.pending.set(key, pending);
    }
    const participant = await pending;
    return participant
      ? { tracks: participant.tracks.map((track) => ({ ...track })) }
      : null;
  }
}
