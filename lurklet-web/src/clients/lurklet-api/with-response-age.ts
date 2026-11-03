export type AgedResponse<T> = { data: T; fetchedAt: number | undefined };

export async function withResponseAge<T>(
  pending: Promise<{ data: T; response: Response }>,
): Promise<AgedResponse<T>> {
  const { data, response } = await pending;
  const age = response.headers.get("Age");
  const seconds = age !== null && /^\d+$/.test(age) ? Number(age) : NaN;
  const fetchedAt = Date.now() - seconds * 1000;

  return {
    data,
    fetchedAt:
      Number.isSafeInteger(seconds) && Number.isSafeInteger(fetchedAt) ? fetchedAt : undefined,
  };
}
