import { useParams } from "react-router";
import { usePageTitle } from "@/hooks/usePageTitle";

export function UserPage() {
  const { username } = useParams();
  usePageTitle(`u/${username}`);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-2xl font-semibold">u/{username}</h1>
      <p className="text-muted-foreground">User profiles aren’t supported yet.</p>
    </div>
  );
}
