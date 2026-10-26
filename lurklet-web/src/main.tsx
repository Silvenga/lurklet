import "@fontsource-variable/inter";
import { QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "react-router/dom";
import { createQueryClient } from "@/app/query-client";
import { createAppRouter } from "@/app/router";
import "@/app/styles.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Root element is missing");
}

if (import.meta.env.DEV && import.meta.env.MODE === "mock") {
  await import("@/_tests/mocks/browser");
}

const router = createAppRouter();
const queryClient = createQueryClient();

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);
