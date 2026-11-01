import { cn } from "cn";
import { useId, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

export function MarkdownSpoiler({ children }: { children: ReactNode }) {
  const [isRevealed, setIsRevealed] = useState(false);
  const contentId = useId();

  return (
    <span
      role="presentation"
      data-reddit-spoiler=""
      className={cn(
        "relative cursor-pointer rounded-sm box-decoration-clone outline-offset-2 has-[>button:focus-visible]:outline-2 has-[>button:focus-visible]:outline-ring",
        isRevealed ? "bg-spoiler-revealed" : "bg-spoiler select-none",
      )}
      onClick={(event) => {
        if (
          !(event.target instanceof Element) ||
          event.target.closest("a, button, [data-reddit-spoiler]") !== event.currentTarget ||
          (isRevealed && window.getSelection()?.isCollapsed === false)
        )
          return;
        setIsRevealed(!isRevealed);
      }}
    >
      {/* A separate keyboard control keeps the text inline and avoids nesting links inside a button. */}
      <Button
        className="sr-only"
        aria-expanded={isRevealed}
        aria-controls={contentId}
        onPress={() => setIsRevealed(!isRevealed)}
      >
        {isRevealed ? "Hide spoiler" : "Show spoiler"}
      </Button>
      <span
        id={contentId}
        aria-hidden={!isRevealed}
        inert={!isRevealed}
        className={cn(!isRevealed && "invisible")}
      >
        {children}
      </span>
    </span>
  );
}
