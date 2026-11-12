import type { AdminRoom, ManagedRoom } from "@freedomstream/contracts";
import type { Rooms } from "./rooms.js";
import type { MediaDirectory } from "./media.js";
import type { Access } from "./access.js";

/** Shares in-flight presence reads between the dashboard and bounded background sweeps. */
export class RoomLifecycle {
  private pending = new Map<string, Promise<AdminRoom | null>>();
  private offset = 0;
  private sweep: Promise<void> | null = null;
  constructor(
    private rooms: Rooms,
    private media: MediaDirectory,
    private access: Access,
  ) {}

  describe(room: ManagedRoom): Promise<AdminRoom | null> {
    const existing = this.pending.get(room.id);
    if (existing) return existing;
    const work = this.read(room).finally(() => this.pending.delete(room.id));
    this.pending.set(room.id, work);
    return work;
  }

  private async read(room: ManagedRoom): Promise<AdminRoom | null> {
    const state = this.rooms.lifecycle(room.id);
    if (!state) return null;
    if (!state.joined) return { ...room, members: [], presence: "waiting" };
    try {
      const members = (await this.media.members(room.id)).filter((member) =>
        this.rooms.active(room.id, member.id),
      );
      if (!this.rooms.lifecycle(room.id)) return null;
      if (this.rooms.observePresence(room.id, members.length > 0)) {
        void this.access.settle();
        return null;
      }
      return {
        ...room,
        members,
        presence: members.length ? "active" : "closing",
      };
    } catch {
      this.rooms.presenceUnavailable(room.id);
      return this.rooms.lifecycle(room.id)
        ? { ...room, members: [], presence: "unavailable" }
        : null;
    }
  }

  async describePage(rooms: ManagedRoom[]): Promise<AdminRoom[]> {
    const result: AdminRoom[] = [];
    for (let i = 0; i < rooms.length; i += 5) {
      const batch = await Promise.all(
        rooms.slice(i, i + 5).map((room) => this.describe(room)),
      );
      result.push(...batch.filter((room): room is AdminRoom => room !== null));
    }
    return result.filter((room) => this.rooms.lifecycle(room.id) !== null);
  }

  tick(): Promise<void> {
    if (this.sweep) return this.sweep;
    this.sweep = (async () => {
      const page = this.rooms.list(this.offset);
      await this.describePage(page.rooms);
      this.offset = page.nextOffset ?? 0;
    })().finally(() => {
      this.sweep = null;
    });
    return this.sweep;
  }

  async settle(): Promise<void> {
    await this.sweep;
    await Promise.allSettled([...this.pending.values()]);
  }
}
