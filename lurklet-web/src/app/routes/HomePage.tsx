import { ArrowRightIcon } from "lucide-react";
import { useNavigate } from "react-router";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";

export function HomePage() {
  const navigate = useNavigate();

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
    </div>
  );
}
