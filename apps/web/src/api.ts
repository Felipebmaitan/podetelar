import type { ApiErrorBody } from "@freedomstream/contracts";

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
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
    throw new ApiError(response.status, error.error);
  }
  return response.status === 204
    ? (undefined as T)
    : (response.json() as Promise<T>);
}
