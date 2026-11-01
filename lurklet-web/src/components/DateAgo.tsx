import { cn } from "cn";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipTrigger } from "@/components/ui/tooltip";
import { useMinuteClock } from "@/hooks/useMinuteClock";

const RELATIVE_FORMAT = new Intl.RelativeTimeFormat(undefined, { numeric: "always" });
const DATE_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "long" });
const DAY = 24 * 60 * 60 * 1000;
const UNITS = [
  ["year", 365 * DAY],
  ["month", 30 * DAY],
  ["day", DAY],
  ["hour", 60 * 60 * 1000],
  ["minute", 60 * 1000],
] as const;

export function DateAgo({ dateTime, className }: { dateTime: string; className?: string }) {
  const [isOpen, setIsOpen] = useState(false);
  const now = useMinuteClock();
  const date = new Date(dateTime);
  const difference = date.getTime() - now;
  const unit = UNITS.find(([, duration]) => Math.abs(difference) >= duration);
  const label = unit
    ? RELATIVE_FORMAT.format(Math.trunc(difference / unit[1]), unit[0])
    : "just now";

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
