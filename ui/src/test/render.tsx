// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import type { ReactElement, ReactNode } from "react";
import { render } from "@testing-library/react";
import { createQueryClient } from "@/shared/query";
import { LOCAL_SERVER, ServerScope, useServers } from "@/app/server";
import type { ConnectionState } from "@/shared/ipc/bindings/ConnectionState";

/** Render con un QueryClient nuevo por prueba (sin caché compartida), dentro
 *  del ámbito de un servidor (por defecto, el equipo local). */
export function renderWithQuery(ui: ReactElement, server: string = LOCAL_SERVER) {
  const client = createQueryClient();
  // Como `wrapper`: `rerender` conserva el ámbito.
  const wrapper = ({ children }: { children: ReactNode }) => (
    <ServerScope server={server} client={client}>
      {children}
    </ServerScope>
  );
  return { client, ...render(ui, { wrapper }) };
}

/** Estado de la conexión de un servidor (por defecto, el local) en la interfaz. */
export function setServerState(state: ConnectionState | null, server: string = LOCAL_SERVER) {
  useServers.setState((s) => {
    const states = { ...s.states };
    if (state === null) delete states[server];
    else states[server] = state;
    return { states };
  });
}

/** Reconexión en curso de un servidor. */
export function setReconnecting(on: boolean, server: string = LOCAL_SERVER) {
  useServers.setState((s) => ({ busy: { ...s.busy, [server]: on } }));
}
