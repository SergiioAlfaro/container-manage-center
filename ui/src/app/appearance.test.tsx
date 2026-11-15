// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Terminal } from "@xterm/xterm";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected } from "@/test/apiMock";
import { apiRecord } from "@/test/fixtures";
import { App } from "./App";
import { DEFAULT_CONFIG, applyAppearance, useApp } from "./store";
import { renderWithQuery, setServerState } from "@/test/render";
import i18n from "@/shared/i18n";
import { terminalTheme, trackTerminal } from "@/shared/ui/terminalTheme";
import { ActivityPanel } from "@/features/activity/ActivityPanel";
import { useActivity } from "@/features/activity/store";
import { EMPTY_FILTER } from "@/features/activity/logic";

async function boot(saved?: string) {
  const config = structuredClone(DEFAULT_CONFIG);
  // Configuración guardada por una versión anterior (p. ej. «teal», ya migrada por el backend o no).
  if (saved) (config.ui as { accent: string }).accent = saved;
  api.configGet.mockResolvedValueOnce(config);
  useApp.setState({ section: "settings", config, saveError: null, info: null });
  setServerState(connected);
  await act(() => useApp.getState().init());
  renderWithQuery(<App />);
}

describe("Color principal (1.0.0, D-082): solo morado", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.configSet.mockImplementation(async (c) => c);
  });
  afterEach(async () => {
    await act(() => i18n.changeLanguage("es"));
  });

  it("el valor predeterminado es el morado", () => {
    expect(DEFAULT_CONFIG.ui.accent).toBe("purple");
  });

  it("Configuración ya no ofrece elegir el color principal (ni el verde)", async () => {
    await boot();
    expect(await screen.findByRole("heading", { level: 1, name: "Configuración" })).toBeInTheDocument();
    expect(await screen.findByRole("radiogroup", { name: "Tema" })).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup", { name: "Color principal" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Verde azulado/)).not.toBeInTheDocument();
    expect(document.querySelector("[data-swatch]")).toBeNull();
    // Cambiar otras preferencias no introduce ningún color distinto del morado.
    await userEvent.click(screen.getByRole("radio", { name: "Oscuro" }));
    expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ theme: "dark", accent: "purple" }) }));
  });

  it("una configuración antigua con el verde no lo aplica: los tokens son los morados", async () => {
    await boot("teal");
    // El atributo que activaba el verde azulado ya no se usa.
    expect(document.documentElement.dataset.accent).toBeUndefined();
    applyAppearance({ ...DEFAULT_CONFIG, ui: { ...DEFAULT_CONFIG.ui, theme: "dark" } });
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.dataset.accent).toBeUndefined();
  });

  it("las terminales usan el morado, también tras aplicar la apariencia", () => {
    expect(terminalTheme().cursor).toBe("#a184f2");
    const term = { options: { theme: terminalTheme() } } as unknown as Terminal;
    const untrack = trackTerminal(term);
    applyAppearance({ ...DEFAULT_CONFIG, ui: { ...DEFAULT_CONFIG.ui, theme: "light" } });
    expect(term.options.theme?.cursor).toBe("#a184f2");
    expect(term.options.theme?.selectionBackground).toBe("#a184f255");
    untrack();
  });
});

describe("Panel de actividad: operaciones de la aplicación", () => {
  it("muestra el distintivo APP, la acción realizada y no ofrece copiar un comando inexistente", async () => {
    useActivity.setState({
      records: [
        apiRecord({
          category: "system",
          kind: "ui.accent",
          target: "purple",
          transport: { via: "app", action: "Cambio del color principal" },
          equivalentCommand: null,
          httpStatus: null,
          summary: "teal → purple",
        }),
      ],
      filter: EMPTY_FILTER,
      loadError: null,
      max: 500,
    });
    render(<ActivityPanel onClose={() => {}} />);
    const item = screen.getByRole("list", { name: "Actividad" }).querySelector("li")!;
    expect(within(item).getByText("APP")).toHaveAttribute("title", "Operación de la aplicación");
    expect(within(item).getByText("Cambiar color principal")).toBeInTheDocument();
    await userEvent.click(within(item).getByRole("button", { expanded: false }));
    expect(within(item).getAllByText("Operación de la aplicación").length).toBeGreaterThan(0);
    expect(within(item).getByText("Cambio del color principal")).toBeInTheDocument();
    expect(within(item).getByText("teal → purple")).toBeInTheDocument();
    expect(within(item).queryByRole("button", { name: /Copiar/ })).not.toBeInTheDocument();
  });
});
