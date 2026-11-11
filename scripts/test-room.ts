import { test, type APIRequestContext } from "@playwright/test";
import { setTimeout as delay } from "node:timers/promises";
import { appendFileSync } from "node:fs";

/** Never let Playwright's network-error call log echo the administrator header. */
export async function createTestRoom(
  request: APIRequestContext,
  origin: string,
  name: string,
) {
  const create = () =>
    request
      .post("/api/rooms", {
        headers: {
          Origin: origin,
          Authorization: `Bearer ${process.env.ADMIN_TOKEN!}`,
        },
        data: { name, displayName: "Owner" },
      })
      .catch(() => {
        throw new Error(
          "Could not create the private test room. Check test-container readiness.",
        );
      });
  let response = await create();
  const retry = Number(response.headers()["retry-after"]);
  // Several suites can share this local API. Respect its production rate limit
  // instead of weakening it for tests; one bounded retry is enough for a window.
  if (
    response.status() === 429 &&
    Number.isInteger(retry) &&
    retry >= 1 &&
    retry <= 60
  ) {
    // Admission may legitimately wait longer than Playwright's default 30s.
    // Extend only for this one server-directed wait; streaming deadlines stay bounded.
    test.setTimeout(test.info().timeout + retry * 1000 + 1000);
    await delay(retry * 1000 + 250);
    response = await create();
  }
  // Optional cleanup ledger contains public room IDs only, never invitations/cookies.
  const ledger = process.env.FREEDOMSTREAM_TEST_ROOM_IDS;
  if (ledger && response.status() === 201) {
    const body = await response.json();
    if (!/^[A-Za-z0-9_-]{22}$/.test(body.room.id))
      throw new Error("Invalid test room ID.");
    appendFileSync(ledger, body.room.id + "\n", { mode: 0o600 });
  }
  return response;
}
