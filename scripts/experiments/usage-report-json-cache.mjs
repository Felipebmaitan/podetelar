// MIT. Same bounded historical cache with ready-to-send JSON, avoiding hit parsing.
import { Usage } from "../../apps/server/dist/usage.js";
const rawReport = Usage.prototype.report,
  states = new WeakMap(),
  MINUTE = 60000,
  MAX_BYTES = 2 * 1024 * 1024;
function stateFor(usage) {
  if (states.has(usage)) return states.get(usage);
  const state = {
    entries: new Map(),
    bytes: 0,
    hits: 0,
    misses: 0,
    invalidations: 0,
  };
  states.set(usage, state);
  const remove = (key) => {
    const entry = state.entries.get(key);
    if (entry) {
      state.bytes -= entry.bytes;
      state.entries.delete(key);
      state.invalidations++;
    }
  };
  const clear = () => {
    for (const key of state.entries.keys()) remove(key);
  };
  const record = usage.record;
  usage.record = function (counters, at, generation) {
    const previous = this.lastSample,
      pruned = this.prunedAt;
    try {
      return record.call(this, counters, at, generation);
    } finally {
      if (this.prunedAt !== pruned) clear();
      else if (previous !== null) {
        const from = Math.floor(Math.min(previous, at) / MINUTE) * MINUTE,
          to = Math.ceil(Math.max(previous, at) / MINUTE) * MINUTE;
        for (const [key, e] of state.entries)
          if (e.from < to && e.to > from) remove(key);
      }
    }
  };
  const remember = usage.remember;
  usage.remember = function (...args) {
    clear();
    return remember.apply(this, args);
  };
  const close = usage.close;
  usage.close = function (...args) {
    clear();
    states.delete(this);
    return close.apply(this, args);
  };
  return state;
}
export function reportCachedJson(usage, from, to, page = 0) {
  from = Math.floor(from / MINUTE) * MINUTE;
  to = Math.ceil(to / MINUTE) * MINUTE;
  // Live/open ranges keep the original path; current writes would invalidate them.
  if (to > Math.floor(usage.now() / MINUTE) * MINUTE)
    return JSON.stringify(rawReport.call(usage, from, to, page));
  const state = stateFor(usage),
    key = `${from}/${to}/${page}`;
  let suffix;
  if (state.entries.has(key)) {
    const entry = state.entries.get(key);
    state.entries.delete(key);
    state.entries.set(key, entry);
    state.hits++;
    suffix = entry.suffix;
  } else {
    state.misses++;
    const {
      lastSampleAt: _last,
      available: _available,
      ...stable
    } = rawReport.call(usage, from, to, page);
    suffix = JSON.stringify(stable).slice(1);
    const bytes = suffix.length * 2;
    if (bytes <= MAX_BYTES) {
      while (state.entries.size >= 4 || state.bytes + bytes > MAX_BYTES) {
        const oldest = state.entries.keys().next().value;
        state.bytes -= state.entries.get(oldest).bytes;
        state.entries.delete(oldest);
      }
      state.entries.set(key, { from, to, suffix, bytes });
      state.bytes += bytes;
    }
  }
  const available =
    usage.available &&
    usage.lastSample !== null &&
    usage.now() - usage.lastSample < 15000;
  return `{"lastSampleAt":${usage.lastSample},"available":${available},${suffix}`;
}
export function jsonCacheStats(usage) {
  const s = states.get(usage);
  return s
    ? {
        entries: s.entries.size,
        accountedBytes: s.bytes,
        hits: s.hits,
        misses: s.misses,
        invalidations: s.invalidations,
      }
    : null;
}
