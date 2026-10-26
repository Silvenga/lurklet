import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

export function PostListError({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  const isNotFound =
    typeof error === "object" && error !== null && "status" in error && error.status === 404;

  return (
    <Alert variant={isNotFound ? "default" : "destructive"}>
      <AlertTitle>{isNotFound ? "Subreddit not found" : "Unable to load posts"}</AlertTitle>
      <AlertDescription>
        {isNotFound ? "This subreddit could not be found." : "Please try again later."}
      </AlertDescription>
      <Button variant="outline" size="sm" className="mt-2 w-fit" onPress={onRetry}>
        Retry
      </Button>
    </Alert>
  );
}
