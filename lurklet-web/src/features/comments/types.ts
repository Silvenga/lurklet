import type { CommentSort } from "@/clients/lurklet-api/generated/types.gen";

export type CommentContext = {
  postId: string;
  subreddit: string;
  slug: string;
  sort: CommentSort;
  focusedCommentId: string | null;
  isUpdating: boolean;
};
