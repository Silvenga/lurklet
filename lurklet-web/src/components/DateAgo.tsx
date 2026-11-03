import { cn } from "cn";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipTrigger } from "@/components/ui/tooltip";
import { useMinuteClock } from "@/hooks/useMinuteClock";
import { formatRelativeTime } from "@/utils/format-relative-time";

const DATE_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "long" });

export function DateAgo({ dateTime, className }: { dateTime: string; className?: string }) {
  const [isOpen, setIsOpen] = useState(false);
  const now = useMinuteClock();
  const date = new Date(dateTime);
  const label = formatRelativeTime(date.getTime(), now);

  return (
    <TooltipTrigger
      trigger="focus"
      isOpen={isOpen}
      onOpenChange={setIsOpen}
      shouldCloseOnPress={false}
    >
      <Button
        type="button"
        slot={null}
        variant="ghost"
        className={cn("h-auto p-0 font-[weight:inherit]", className)}
        onPress={() => setIsOpen((open) => !open)}
      >
        <time dateTime={dateTime}>{label}</time>
      </Button>
      <Tooltip>{DATE_FORMAT.format(date)}</Tooltip>
    </TooltipTrigger>
  );
}
