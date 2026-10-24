import "@fontsource-variable/inter";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "react-router/dom";
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

createRoot(root).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
