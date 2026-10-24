import { createBrowserRouter } from "react-router";
import { HomePage } from "@/app/routes/HomePage";

export function createAppRouter() {
  return createBrowserRouter([{ path: "/", Component: HomePage }]);
}
