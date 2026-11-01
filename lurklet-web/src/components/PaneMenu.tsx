import { FileJsonIcon } from "lucide-react";
import { Text } from "react-aria-components";
import { BsThreeDotsVertical } from "react-icons/bs";
import { FaReddit } from "react-icons/fa";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export function PaneMenu({
  jsonHref,
  redditHref,
  label,
}: {
  jsonHref: string | undefined;
  redditHref?: string;
  label: string;
}) {
  return (
    <DropdownMenuTrigger>
      <Button variant="ghost" size="icon-sm" className="absolute top-0 right-0" aria-label={label}>
        <BsThreeDotsVertical aria-hidden="true" data-icon="inline-start" />
      </Button>
      <DropdownMenu placement="bottom end" className="w-max">
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
      </DropdownMenu>
    </DropdownMenuTrigger>
  );
}
