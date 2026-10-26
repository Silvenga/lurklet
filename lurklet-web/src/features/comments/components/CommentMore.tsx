import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import {
  getPostCommentsOptions,
  getPostCommentsQueryKey,
} from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import type { CommentContext } from "@/features/comments/types";
import { CommentList } from "./CommentList";

export function CommentMore({
  cursor,
  context,
  depth,
}: {
  cursor: string;
  context: CommentContext;
  depth: number;
}) {
  const [isRequested, setIsRequested] = useState(false);
  const queryClient = useQueryClient();
  const expansion = useQuery({
    ...getPostCommentsOptions({
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
    return expansion.data.items.length > 0 ? (
      <CommentList items={expansion.data.items} context={context} depth={depth} />
    ) : (
      <p className="text-muted-foreground text-sm">No more replies.</p>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {expansion.isError && (
        <Alert variant="destructive">
          <AlertTitle>Unable to load replies</AlertTitle>
          <AlertDescription>Please try again.</AlertDescription>
        </Alert>
      )}
      <Button
        className="w-fit"
        variant="outline"
        size="sm"
        isDisabled={context.isUpdating || expansion.isFetching}
        onPress={() => {
          if (expansion.isError) {
            void expansion.refetch();
          } else {
            setIsRequested(true);
          }
        }}
      >
        {expansion.isFetching
          ? "Loading replies"
          : expansion.isError
            ? "Retry"
            : "Load more replies"}
      </Button>
    </div>
  );
}
