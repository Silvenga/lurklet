import { FileJsonIcon } from "lucide-react";
import { useId } from "react";
import { Menu, Text } from "react-aria-components";
import { BsThreeDotsVertical } from "react-icons/bs";
import { FaReddit } from "react-icons/fa";
import { FetchedAgo } from "@/components/FetchedAgo";
import { Button } from "@/components/ui/button";
import {
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Popover } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";

export function PaneMenu({
  jsonHref,
  fetchedAt,
  redditHref,
  label,
}: {
  jsonHref: string | undefined;
  fetchedAt?: number | undefined;
  redditHref?: string;
  label: string;
}) {
  const ageId = useId();

  return (
    <DropdownMenuTrigger>
      <Button variant="ghost" size="icon-sm" className="absolute top-0 right-0" aria-label={label}>
        <BsThreeDotsVertical aria-hidden="true" data-icon="inline-start" />
      </Button>
      <Popover
        data-slot="dropdown-menu-content"
        placement="bottom end"
        className="w-max min-w-32 gap-0 overflow-x-hidden overflow-y-auto p-1"
      >
        <Menu
          {...(fetchedAt === undefined ? {} : { "aria-describedby": ageId })}
          className="**:data-[slot$=-item]:data-focused:bg-foreground/10 max-h-[inherit] overflow-x-hidden overflow-y-auto outline-hidden"
        >
          <DropdownMenuGroup>
            {redditHref && (
              <DropdownMenuItem
                id="reddit"
                textValue="Open on Reddit"
                href={redditHref}
                target="_blank"
                rel="noopener noreferrer"
              >
                <FaReddit aria-hidden="true" data-icon="inline-start" />
                <Text slot="label" className="whitespace-nowrap">
                  Open on Reddit
                </Text>
                <Text slot="description" className="sr-only">
                  Opens in a new tab
                </Text>
              </DropdownMenuItem>
            )}
            <DropdownMenuItem
              id="json"
              textValue="JSON"
              {...(jsonHref ? { href: jsonHref } : {})}
              target="_blank"
              rel="noopener noreferrer"
              isDisabled={!jsonHref}
            >
              <FileJsonIcon aria-hidden="true" data-icon="inline-start" />
              <Text slot="label">JSON</Text>
              {jsonHref && (
                <Text slot="description" className="sr-only">
                  Opens in a new tab
                </Text>
              )}
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </Menu>
        {fetchedAt !== undefined && (
          <>
            <Separator className="my-1" />
            <FetchedAgo id={ageId} fetchedAt={fetchedAt} />
          </>
        )}
      </Popover>
    </DropdownMenuTrigger>
  );
}
