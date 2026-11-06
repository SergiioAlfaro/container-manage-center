import { QueryClient } from "@tanstack/react-query";

/**
 * Cada consulta es una petición real a Docker y queda en el panel de actividad,
 * así que no se reintenta automáticamente ni se repite si los datos son recientes.
 */
export function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: 3_000, refetchOnWindowFocus: true },
      mutations: { retry: false },
    },
  });
}

export const keys = {
  overview: ["overview"] as const,
  containers: ["containers"] as const,
  images: ["images"] as const,
  networks: ["networks"] as const,
  compose: ["compose"] as const,
};
