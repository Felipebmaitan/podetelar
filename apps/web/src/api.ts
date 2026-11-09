import type { ApiErrorBody } from "@freedomstream/contracts";

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
    public readonly retryAfterMs = 0,
  ) {
    super(message);
  }
}

export async function request<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  if (typeof init.body === "string" && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  const response = await fetch(path, {
    ...init,
    credentials: "same-origin",
    headers,
  });
  if (!response.ok) {
    const error = (await response.json().catch(() => ({
      error: "The server could not be reached. Please try again.",
    }))) as ApiErrorBody;
    const retrySeconds = Number(response.headers.get("Retry-After"));
    throw new ApiError(
      response.status,
      error.error,
      Number.isFinite(retrySeconds)
        ? Math.min(60, Math.max(0, retrySeconds)) * 1000
        : 0,
    );
  }
  return response.status === 204
    ? (undefined as T)
    : (response.json() as Promise<T>);
}
