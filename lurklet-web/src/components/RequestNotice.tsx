import { useEffect, useState } from "react";
import type { RequestFailure } from "@/clients/lurklet-api/request-failure";
import { LoadFailureNotice } from "@/components/LoadFailureNotice";

export function RequestNotice({
  subject,
  query,
  disabled = false,
}: {
  subject: string;
  query: { error: RequestFailure; isFetching: boolean; refetch: () => Promise<unknown> };
  disabled?: boolean;
}) {
  const { error } = query;
  const [now, setNow] = useState(Date.now);
  const waiting = error.retryable && error.retryAt > now;
  useEffect(() => {
    if (!waiting) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [waiting]);
  return (
    <LoadFailureNotice
      subject={subject}
      unavailable={error.status === 404}
      detail={
        error.status
          ? [error.message, `HTTP ${error.status}`].filter(Boolean).join(" - ")
          : error.message
      }
      onRetry={
        error.retryable
          ? () => {
              void query.refetch();
            }
          : undefined
      }
      disabled={disabled || waiting || query.isFetching}
      retryLabel={waiting ? `Retry in ${Math.ceil((error.retryAt - now) / 1000)}s` : "Retry"}
    />
  );
}
