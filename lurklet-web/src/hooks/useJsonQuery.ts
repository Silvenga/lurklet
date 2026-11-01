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
  TQueryFnData,
  TError,
  TData = TQueryFnData,
  TQueryKey extends ApiQueryKey = ApiQueryKey,
>(options: UseQueryOptions<TQueryFnData, TError, TData, TQueryKey>) {
  const query = useQuery(options);
  const apiPath = options.meta?.apiPath;
  const jsonHref =
    query.isEnabled && !query.isPlaceholderData && apiPath
      ? client.buildUrl({ ...options.queryKey[0], url: apiPath })
      : undefined;

  return { query, jsonHref };
}
