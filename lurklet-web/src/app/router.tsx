import { createBrowserRouter, type RouteObject } from "react-router";
import { AppLayout } from "@/app/AppLayout";
import { HomePage } from "@/app/routes/HomePage";
import { NotFoundPage } from "@/app/routes/NotFoundPage";
import { PostListPage } from "@/app/routes/PostListPage";
import { PostListRedirect } from "@/app/routes/PostListRedirect";
import { SubredditLayout } from "@/app/routes/SubredditLayout";
import { postListLoader } from "@/features/posts/api/post-list-loader";
import { PostListLoading } from "@/features/posts/components/PostListLoading";
import { PostLoading } from "@/features/posts/components/PostLoading";

export const APP_ROUTES: RouteObject[] = [
  {
    path: "/",
    Component: AppLayout,
    children: [
      { index: true, Component: HomePage },
      {
        path: "r/:subreddit",
        Component: SubredditLayout,
        children: [
          {
            index: true,
            Component: PostListPage,
            loader: postListLoader,
            HydrateFallback: PostListLoading,
            ErrorBoundary: NotFoundPage,
            shouldRevalidate({
              currentParams,
              nextParams,
              currentUrl,
              nextUrl,
              defaultShouldRevalidate,
            }) {
              const currentSort = currentUrl.searchParams.get("sort") ?? "hot";
              const nextSort = nextUrl.searchParams.get("sort") ?? "hot";
              if (
                currentParams.subreddit !== nextParams.subreddit ||
                currentSort !== nextSort ||
                currentUrl.searchParams.get("after") !== nextUrl.searchParams.get("after") ||
                (nextSort === "top" &&
                  (currentUrl.searchParams.get("t") ?? "all") !==
                    (nextUrl.searchParams.get("t") ?? "all"))
              ) {
                return true;
              }
              return currentUrl.href === nextUrl.href && defaultShouldRevalidate;
            },
          },
          { path: ":sort", Component: PostListRedirect },
          {
            path: "comments/:postId/:slug/:commentId",
            lazy: async () => ({ Component: (await import("@/app/routes/PostPage")).PostPage }),
            HydrateFallback: PostLoading,
          },
          {
            path: "comments/:postId/:slug?",
            lazy: async () => ({ Component: (await import("@/app/routes/PostPage")).PostPage }),
            HydrateFallback: PostLoading,
          },
          { path: "*", Component: NotFoundPage },
        ],
      },
      { path: "*", Component: NotFoundPage },
    ],
  },
];

export function createAppRouter() {
  return createBrowserRouter(APP_ROUTES);
}
