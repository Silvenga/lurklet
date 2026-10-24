import { createBrowserRouter } from "react-router";
import { HomePage } from "@/app/routes/HomePage";

export const router = createBrowserRouter([{ path: "/", Component: HomePage }]);
