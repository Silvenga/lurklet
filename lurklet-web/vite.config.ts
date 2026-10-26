import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { msw } from "msw/vite";
import { defineConfig } from "vitest/config";

export default defineConfig(({ command, mode }) => ({
  plugins: [
    react(),
    tailwindcss(),
    command === "serve" && mode === "mock" && msw({ mode: "worker-only" }),
  ],
  build: {
    sourcemap: true,
  },
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    proxy: {
      "^/api(?:/|\\?|$)": "http://127.0.0.1:3000",
    },
  },
  test: {
    environment: "./src/_tests/environment.ts",
    testTimeout: 1000,
    hookTimeout: 1000,
    teardownTimeout: 1000,
    setupFiles: ["./src/_tests/setup.ts"],
    include: ["src/**/*.tests.{ts,tsx}", "scripts/**/*.tests.ts"],
  },
}));
