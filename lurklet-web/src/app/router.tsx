import { createBrowserRouter } from "react-router";
import { AppLayout } from "@/app/AppLayout";
import { HomePage } from "@/app/routes/HomePage";

export function createAppRouter() {
  return createBrowserRouter([
    {
      path: "/",
      Component: AppLayout,
      children: [{ index: true, Component: HomePage }],
    },
  ]);
}
