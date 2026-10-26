import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

export function PostError({ error }: { error: unknown }) {
  const isNotFound =
    typeof error === "object" && error !== null && "status" in error && error.status === 404;

  return (
    <Alert variant={isNotFound ? "default" : "destructive"}>
      <AlertTitle>{isNotFound ? "Post not found" : "Unable to load post"}</AlertTitle>
      <AlertDescription>
        {isNotFound ? "This post could not be found." : "Please try again later."}
      </AlertDescription>
    </Alert>
  );
}
