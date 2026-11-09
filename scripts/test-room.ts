import type { APIRequestContext } from "@playwright/test";

/** Never let Playwright's network-error call log echo the administrator header. */
export async function createTestRoom(
  request: APIRequestContext,
  origin: string,
  name: string,
) {
  return request
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
}
