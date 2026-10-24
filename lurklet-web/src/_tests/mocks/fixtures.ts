import type { Health } from "@/clients/lurklet-api/generated/types.gen";

export const HEALTH_RESPONSE = { health: "ok" } satisfies Health;
