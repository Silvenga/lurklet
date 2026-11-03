import { useState } from "react";
import { useMinuteClock } from "@/hooks/useMinuteClock";
import { formatRelativeTime } from "@/utils/format-relative-time";

export function FetchedAgo({ fetchedAt, id }: { fetchedAt: number; id: string }) {
  const [openedAt] = useState(Date.now);
  const now = Math.max(useMinuteClock(), openedAt);

  return (
    <p id={id} className="text-muted-foreground px-1.5 py-1 text-xs whitespace-nowrap">
      Fetched {formatRelativeTime(fetchedAt, now)}
    </p>
  );
}
