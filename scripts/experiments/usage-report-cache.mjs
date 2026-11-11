// MIT. Bounded derived-report cache prototype; authorization is outside this API.
import { Usage } from "../../apps/server/dist/usage.js";
const rawReport = Usage.prototype.report;
const states = new WeakMap();
const MINUTE = 60000,
  MAX_BYTES = 2 * 1024 * 1024,
  MAX_ENTRIES = 4;
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
    const prior = this.lastSample,
      pruned = this.prunedAt;
    try {
      return record.call(this, counters, at, generation);
    } finally {
      if (this.prunedAt !== pruned) clear();
      else if (prior !== null) {
        const from = Math.floor(Math.min(prior, at) / MINUTE) * MINUTE,
          to = Math.ceil(Math.max(prior, at) / MINUTE) * MINUTE;
        for (const [key, entry] of state.entries)
          if (entry.from < to && entry.to > from) remove(key);
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
export function reportCached(usage, from, to, page = 0) {
  const state = stateFor(usage);
  from = Math.floor(from / MINUTE) * MINUTE;
  to = Math.ceil(to / MINUTE) * MINUTE;
  const key = `${from}/${to}/${page}`;
  let report;
  if (state.entries.has(key)) {
    const entry = state.entries.get(key);
    state.entries.delete(key);
    state.entries.set(key, entry);
    state.hits++;
    report = JSON.parse(entry.json);
  } else {
    state.misses++;
    report = rawReport.call(usage, from, to, page);
    const json = JSON.stringify(report),
      bytes = json.length * 2;
    if (bytes <= MAX_BYTES) {
      while (
        state.entries.size >= MAX_ENTRIES ||
        state.bytes + bytes > MAX_BYTES
      ) {
        const oldest = state.entries.keys().next().value;
        state.bytes -= state.entries.get(oldest).bytes;
        state.entries.delete(oldest);
      }
      state.entries.set(key, { json, bytes, from, to });
      state.bytes += bytes;
    }
  }
  // Fresh state on every read, including collector failure without a DB write.
  return {
    ...report,
    lastSampleAt: usage.lastSample,
    available:
      usage.available &&
      usage.lastSample !== null &&
      usage.now() - usage.lastSample < 15000,
  };
}
export function cacheStats(usage) {
  const state = states.get(usage);
  return state
    ? {
        entries: state.entries.size,
        accountedBytes: state.bytes,
        hits: state.hits,
        misses: state.misses,
        invalidations: state.invalidations,
      }
    : null;
}
