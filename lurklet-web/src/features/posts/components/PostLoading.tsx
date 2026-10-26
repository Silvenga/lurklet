import { Skeleton } from "@/components/ui/skeleton";

export function PostLoading() {
  return (
    <div role="status" className="flex flex-col gap-4">
      <span className="sr-only">Loading post</span>
      <Skeleton className="h-8 w-3/4" />
      <Skeleton className="h-40 w-full" />
    </div>
  );
}
