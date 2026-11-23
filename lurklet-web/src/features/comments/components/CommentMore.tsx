import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CirclePlusIcon } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { getPostCommentsQueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type { CommentItem } from "@/clients/lurklet-api/generated/types.gen";
import { apiQueryOptions } from "@/clients/lurklet-api/query-options";
import { RequestNotice } from "@/components/RequestNotice";
import { Button } from "@/components/ui/button";
import type { CommentContext } from "@/features/comments/types";

export function CommentMore({
  cursor,
  context,
  renderItems,
  renderRow,
}: {
  cursor: string;
  context: CommentContext;
  renderItems: (items: CommentItem[]) => ReactNode;
  renderRow: (content: ReactNode) => ReactNode;
}) {
  const [isRequested, setIsRequested] = useState(false);
  const queryClient = useQueryClient();
  const expansion = useQuery({
    ...apiQueryOptions.getPostComments({
      path: { id: context.postId },
      query: { sort: context.sort, cursor },
    }),
    enabled: isRequested && !context.isUpdating,
  });

  useEffect(() => {
    if (context.isUpdating) {
      void queryClient.cancelQueries({
        queryKey: getPostCommentsQueryKey({
          path: { id: context.postId },
          query: { sort: context.sort, cursor },
        }),
        exact: true,
      });
    }
  }, [queryClient, context.isUpdating, context.postId, context.sort, cursor]);

  if (expansion.isSuccess) {
    return expansion.data.items.length > 0
      ? renderItems(expansion.data.items)
      : renderRow(<p className="text-muted-foreground text-sm/6">No more replies.</p>);
  }

  if (expansion.isError)
    return renderRow(
      <RequestNotice subject="Replies" query={expansion} disabled={context.isUpdating} />,
    );

  return renderRow(
    <div className="flex flex-col gap-2">
      <Button
        className="w-fit"
        variant="ghost"
        size="xs"
        isDisabled={context.isUpdating || expansion.isFetching}
        onPress={() => setIsRequested(true)}
      >
        <CirclePlusIcon aria-hidden="true" data-icon="inline-start" />
        {expansion.isFetching ? "Loading replies" : "Load more replies"}
      </Button>
    </div>,
  );
}
