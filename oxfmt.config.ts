import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: ["*.md", ".agents/**"],
  sortImports: {
    newlinesBetween: false,
  },
  sortTailwindcss: true,
  sortPackageJson: true,
});
