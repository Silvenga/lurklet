import { Navigate, useLocation, useParams } from "react-router";
import { NotFoundPage } from "./NotFoundPage";

export function PostListRedirect() {
  const { subreddit, sort } = useParams();
  const { search, hash } = useLocation();
  if (sort !== "hot" && sort !== "new" && sort !== "top") {
    return <NotFoundPage />;
  }

  const query = new URLSearchParams(search);
  if (!query.has("sort")) {
    query.set("sort", sort);
  }

  return (
    <Navigate
      replace
      to={{ pathname: `/r/${encodeURIComponent(subreddit!)}`, search: query.toString(), hash }}
    />
  );
}
