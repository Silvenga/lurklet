import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "../lurklet-api/openapi-spec.json",
  output: {
    path: "src/clients/lurklet-api/generated",
    entryFile: false,
    header: ({ file, defaultValue }) =>
      file
        ? defaultValue
        : [
            ...defaultValue,
            "// @ts-nocheck - Upstream runtime templates do not support exactOptionalPropertyTypes.",
          ],
    tsConfigPath: "tsconfig.app.json",
  },
  plugins: [
    "@hey-api/typescript",
    "@hey-api/sdk",
    {
      name: "@hey-api/client-fetch",
      baseUrl: false,
      throwOnError: true,
      runtimeConfigPath: "./src/clients/lurklet-api/config.ts",
    },
  ],
});
