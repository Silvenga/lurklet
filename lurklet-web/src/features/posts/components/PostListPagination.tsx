import { cn } from "cn";
import { Link, useSearchParams } from "react-router";
import { buttonVariants } from "@/components/ui/button";

export function PostListPagination({
  nextCursor,
  hasPreviousPages,
  isDisabled,
}: {
  nextCursor: string | null;
  hasPreviousPages: boolean;
  isDisabled: boolean;
}) {
  const [searchParams] = useSearchParams();
  const linkClassName = cn(
    buttonVariants({ variant: "outline" }),
    isDisabled && "cursor-not-allowed opacity-50",
  );
  const first = new URLSearchParams(searchParams);
  first.delete("after");
  const next = new URLSearchParams(searchParams);
  if (nextCursor !== null) {
    next.set("after", nextCursor);
  }

  if (!hasPreviousPages && nextCursor === null) {
    return null;
  }

  return (
    <nav aria-label="Pagination" className="flex justify-end gap-2">
      {hasPreviousPages &&
        (isDisabled ? (
          <span role="link" aria-disabled="true" className={linkClassName}>
            First page
          </span>
        ) : (
          <Link className={linkClassName} to={{ search: first.toString() }}>
            First page
          </Link>
        ))}
      {nextCursor !== null &&
        (isDisabled ? (
          <span role="link" aria-disabled="true" className={linkClassName}>
            Next
          </span>
        ) : (
          <Link className={linkClassName} to={{ search: next.toString() }}>
            Next
          </Link>
        ))}
    </nav>
  );
}
