import { builtinEnvironments, type Environment } from "vitest/runtime";

export default {
  ...builtinEnvironments["happy-dom"],
  name: "lurklet-dom",
  async setup(global: typeof globalThis, options) {
    const transport = {
      fetch: global.fetch,
      Request: global.Request,
      Response: global.Response,
      Headers: global.Headers,
      AbortController: global.AbortController,
      AbortSignal: global.AbortSignal,
    };
    const environment = await builtinEnvironments["happy-dom"].setup(global, options);

    // Use one owned Node transport rather than Happy DOM's separate HTTP client.
    Object.assign(global, transport);
    Object.assign(global.window, transport);
    return environment;
  },
} satisfies Environment;
