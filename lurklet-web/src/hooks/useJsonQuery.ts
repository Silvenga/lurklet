import { useQuery, type UseQueryOptions } from "@tanstack/react-query";
import type { QueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { client } from "@/clients/lurklet-api/generated/client.gen";

declare module "@tanstack/react-query" {
  interface Register {
    queryMeta: { apiPath?: string; [key: string]: unknown };
  }
}

type ApiQueryKey = QueryKey<Parameters<typeof client.buildUrl>[0]>;

export function useJsonQuery<
  TData extends { fetchedAt: string },
  TError,
  TQueryKey extends ApiQueryKey = ApiQueryKey,
>(options: UseQueryOptions<TData, TError, TData, TQueryKey>) {
  const query = useQuery(options);
  const apiPath = options.meta?.apiPath;
  const jsonHref =
    query.isEnabled && !query.isPlaceholderData && apiPath
      ? client.buildUrl({ ...options.queryKey[0], url: apiPath })
      : undefined;

  const timestamp = query.isEnabled && query.isSuccess ? query.data.fetchedAt : undefined;
  const fetchedAt = typeof timestamp === "string" ? Date.parse(timestamp) : NaN;

  return { query, jsonHref, fetchedAt: Number.isFinite(fetchedAt) ? fetchedAt : undefined };
}
