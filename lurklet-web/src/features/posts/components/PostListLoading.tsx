import { Skeleton } from "@/components/ui/skeleton";

export function PostListLoading() {
  return (
    <div role="status" className="flex flex-col gap-6">
      <span className="sr-only">Loading posts</span>
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
    </div>
  );
}
