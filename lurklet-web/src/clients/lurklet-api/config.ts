import type { CreateClientConfig } from "./generated/client.gen";

export const createClientConfig: CreateClientConfig = (config) => ({
  ...config,
  baseUrl: globalThis.location.origin,
});
