import { useQuery, type UseQueryOptions } from "@tanstack/react-query";
import type { QueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { client } from "@/clients/lurklet-api/generated/client.gen";
import type { AgedResponse } from "@/clients/lurklet-api/with-response-age";

declare module "@tanstack/react-query" {
  interface Register {
    queryMeta: { apiPath?: string; [key: string]: unknown };
  }
}

type ApiQueryKey = QueryKey<Parameters<typeof client.buildUrl>[0]>;

export function useJsonQuery<
  TQueryFnData extends AgedResponse<unknown>,
  TError,
  TData = TQueryFnData["data"],
  TQueryKey extends ApiQueryKey = ApiQueryKey,
>(options: UseQueryOptions<TQueryFnData, TError, TData, TQueryKey>) {
  const query = useQuery(options);
  // Observe timestamp changes even when the selected body is unchanged, without fetching again.
  const age = useQuery({
    ...options,
    enabled: false,
    select: (response) => response.fetchedAt,
  });
  const apiPath = options.meta?.apiPath;
  const jsonHref =
    query.isEnabled && !query.isPlaceholderData && apiPath
      ? client.buildUrl({ ...options.queryKey[0], url: apiPath })
      : undefined;

  const fetchedAt = query.isEnabled && query.isSuccess ? age.data : undefined;

  return { query, jsonHref, fetchedAt };
}
