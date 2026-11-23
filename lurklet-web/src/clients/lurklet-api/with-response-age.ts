import { RequestFailure } from "./request-failure";

export type AgedResponse<T> = { data: T; fetchedAt: number | undefined };

export async function withResponseAge<T extends object>(
  pending: Promise<{ data: T | undefined; response?: Response; error?: unknown }>,
): Promise<AgedResponse<T>> {
  const result = await pending;
  if (result.data === undefined || !result.response) {
    if (result.error instanceof Error && result.error.name === "AbortError") throw result.error;
    throw new RequestFailure(result.response, result.error);
  }
  const { data, response } = result;
  const age = response.headers.get("Age");
  const seconds = age !== null && /^\d+$/.test(age) ? Number(age) : NaN;
  const fetchedAt = Date.now() - seconds * 1000;

  return {
    data,
    fetchedAt:
      Number.isSafeInteger(seconds) && Number.isSafeInteger(fetchedAt) ? fetchedAt : undefined,
  };
}
