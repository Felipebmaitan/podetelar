import assert from "node:assert/strict";
import { test } from "node:test";
import { networkSummary } from "../src/media/network.js";
import { reconnectPolicy } from "../src/media/recovery.js";
test("route diagnostics keep selected aggregate facts and discard RTC authority", () => {
  const stats = new Map([
    ["transport", { type: "transport", selectedCandidatePairId: "pair" }],
    [
      "pair",
      {
        type: "candidate-pair",
        localCandidateId: "local",
        currentRoundTripTime: 0.18,
      },
    ],
    [
      "local",
      {
        candidateType: "prflx",
        relayProtocol: "tls",
        address: "private-address",
        url: "turns:private-relay",
        usernameFragment: "secret",
      },
    ],
  ]) as unknown as RTCStatsReport;
  assert.deepEqual(networkSummary(stats), { route: "TURN/TLS", rttMs: 180 });
  assert.deepEqual(networkSummary(undefined), {
    route: "unknown",
    rttMs: null,
  });
});
test("reconnect policy bounds one episode by attempts and elapsed time", () => {
  assert.equal(
    reconnectPolicy.nextRetryDelayInMs({ retryCount: 0, elapsedMs: 0 }),
    0,
  );
  assert.equal(
    reconnectPolicy.nextRetryDelayInMs({ retryCount: 10, elapsedMs: 1 }),
    null,
  );
  assert.equal(
    reconnectPolicy.nextRetryDelayInMs({ retryCount: 1, elapsedMs: 60_000 }),
    null,
  );
  for (let retryCount = 1; retryCount < 10; retryCount++) {
    const delay = reconnectPolicy.nextRetryDelayInMs({
      retryCount,
      elapsedMs: 10_000,
    });
    assert.ok(delay !== null && delay >= 300 && delay < 8000);
  }
});
