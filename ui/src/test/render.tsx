import type { ReactElement } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import { createQueryClient } from "@/shared/query";

/** Render con un QueryClient nuevo por prueba (sin caché compartida). */
export function renderWithQuery(ui: ReactElement) {
  const client = createQueryClient();
  return { client, ...render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>) };
}
