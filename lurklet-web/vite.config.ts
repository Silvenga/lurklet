import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react(), tailwindcss()],
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
    environment: "happy-dom",
    setupFiles: ["./src/_tests/setup.ts"],
    include: ["src/**/*.tests.{ts,tsx}", "scripts/**/*.tests.ts"],
  },
});
