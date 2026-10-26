import { ArrowRightIcon } from "lucide-react";
import { Link, useNavigate } from "react-router";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import { Tooltip, TooltipTrigger } from "@/components/ui/tooltip";
import { usePageTitle } from "@/hooks/usePageTitle";

const SUGGESTED_SUBREDDITS = [
  { name: "aww", description: "Cute animals and other adorable things" },
  { name: "wholesomememes", description: "Feel-good memes" },
  { name: "ContagiousLaughter", description: "Videos of infectious laughter" },
  { name: "MadeMeSmile", description: "Moments that brighten someone's day" },
  { name: "AnimalsBeingDerps", description: "Silly animals" },
];

export function HomePage() {
  const navigate = useNavigate();
  usePageTitle("Home");

  return (
    <div className="flex flex-col items-center gap-6 py-20 text-center">
      <h1 className="text-5xl font-extralight tracking-tight sm:text-7xl">Lurklet</h1>
      <p className="text-muted-foreground text-lg">
        A lightweight, privacy-focused front end for Reddit.
      </p>
      <form
        className="w-full max-w-sm text-left"
        onSubmit={(event) => {
          event.preventDefault();
          const subreddit = new FormData(event.currentTarget).get("subreddit")?.toString().trim();
          if (subreddit) {
            void navigate(`/r/${encodeURIComponent(subreddit)}`);
          }
        }}
      >
        <FieldGroup>
          <Field>
            <InputGroup>
              <InputGroupInput
                id="subreddit"
                name="subreddit"
                placeholder="subreddit"
                className="select-text"
                required
              />
              <InputGroupAddon>
                <FieldLabel htmlFor="subreddit">r/</FieldLabel>
              </InputGroupAddon>
              <InputGroupAddon align="inline-end">
                <InputGroupButton type="submit" size="icon-xs" aria-label="Go">
                  <ArrowRightIcon aria-hidden="true" data-icon="inline-end" />
                </InputGroupButton>
              </InputGroupAddon>
            </InputGroup>
          </Field>
        </FieldGroup>
      </form>
      <nav aria-label="Suggested subreddits" className="w-full max-w-lg">
        <ul className="flex flex-wrap justify-center gap-x-4 gap-y-2 text-sm">
          {SUGGESTED_SUBREDDITS.map(({ name, description }) => (
            <li key={name}>
              <TooltipTrigger>
                <Link
                  className="text-muted-foreground hover:text-foreground focus-visible:outline-ring rounded-sm whitespace-nowrap underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-offset-4"
                  to={`/r/${encodeURIComponent(name)}`}
                >
                  r/{name}
                </Link>
                <Tooltip>{description}</Tooltip>
              </TooltipTrigger>
            </li>
          ))}
        </ul>
      </nav>
    </div>
  );
}
