import { Link, Outlet, useParams } from "react-router";
import { SubredditMetadata } from "@/features/subreddits/components/SubredditMetadata";

export function SubredditLayout() {
  const { subreddit } = useParams();

  return (
    <div className="grid gap-8 md:grid-cols-[minmax(0,1fr)_16rem]">
      <section aria-label="Page content" className="min-w-0">
        <Outlet />
      </section>
      <aside aria-label="Subreddit" className="flex min-w-0 flex-col gap-2 select-text">
        <h2 className="text-lg font-medium">
          <Link to={`/r/${encodeURIComponent(subreddit!)}`}>r/{subreddit}</Link>
        </h2>
        <SubredditMetadata subreddit={subreddit!} />
      </aside>
    </div>
  );
}
