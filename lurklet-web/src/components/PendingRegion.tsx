import { cn } from "cn";
import type { ReactNode } from "react";

export function PendingRegion({
  pending,
  children,
  className,
}: {
  pending: boolean;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div aria-busy={pending} className={cn(className, pending && "opacity-80")}>
      {children}
    </div>
  );
}
