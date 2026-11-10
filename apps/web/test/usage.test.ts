import assert from "node:assert/strict";
import { test } from "node:test";
import { dataSize, periodRange, speed } from "../src/usage.js";

test("volume uses decimal bytes and speed uses bits", () => {
  assert.equal(dataSize(1000000), "1 MB");
  assert.equal(dataSize(1000000000), "1 GB");
  assert.equal(dataSize(0), "0 MB");
  assert.equal(speed(1250000, 1000), 10);
  assert.equal(speed(50, 0), 0);
});
test("presets use local calendar boundaries and minute precision", () => {
  const now = new Date(2026, 10, 9, 12, 30, 25);
  const today = new Date(2026, 10, 9).getTime();
  assert.equal(periodRange("Today", now)[0], today);
  assert.equal(periodRange("Month", now)[0], new Date(2026, 10, 1).getTime());
  assert.equal(periodRange("7 days", now)[0], new Date(2026, 10, 3).getTime());
  const [from, to] = periodRange("Real time", now);
  assert.equal(to - from, 15 * 60000);
  assert.equal(to % 60000, 0);
});
