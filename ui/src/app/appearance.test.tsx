import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Terminal } from "@xterm/xterm";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected } from "@/test/apiMock";
import { apiRecord } from "@/test/fixtures";
import { App } from "./App";
import { DEFAULT_CONFIG, applyAppearance, useApp } from "./store";
import { useEngine } from "@/features/engine/store";
import { renderWithQuery } from "@/test/render";
import i18n from "@/shared/i18n";
import type { Accent } from "@/shared/ipc/bindings/Accent";
import { terminalTheme, trackTerminal } from "@/shared/ui/terminalTheme";
import { ActivityPanel } from "@/features/activity/ActivityPanel";
import { useActivity } from "@/features/activity/store";
import { EMPTY_FILTER } from "@/features/activity/logic";

async function boot(accent: Accent = "teal") {
  const config = structuredClone(DEFAULT_CONFIG);
  config.ui.accent = accent;
  api.configGet.mockResolvedValueOnce(config);
  useApp.setState({ section: "settings", config, saveError: null, info: null });
  useEngine.setState({ state: connected, reconnecting: false, error: null });
  await act(() => useApp.getState().init());
  renderWithQuery(<App />);
}

describe("Color principal (0.9.2)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.configSet.mockImplementation(async (c) => c);
  });
  afterEach(async () => {
    await act(() => i18n.changeLanguage("es"));
    document.documentElement.dataset.accent = "teal";
  });

  it("por defecto se mantiene el color existente (verde azulado)", () => {
    expect(DEFAULT_CONFIG.ui.accent).toBe("teal");
  });

  it("elegir morado se aplica al momento, sin reiniciar, y se guarda", async () => {
    await boot();
    const group = await screen.findByRole("radiogroup", { name: "Color principal" });
    expect(within(group).getAllByRole("radio").map((r) => r.textContent)).toEqual(["Verde azulado", "Morado"]);
    expect(within(group).getByRole("radio", { name: "Verde azulado" })).toHaveAttribute("aria-checked", "true");
    expect(document.documentElement.dataset.accent).toBe("teal");
    await userEvent.click(within(group).getByRole("radio", { name: "Morado" }));
    expect(document.documentElement.dataset.accent).toBe("purple");
    expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ accent: "purple" }) }));
    expect(within(group).getByRole("radio", { name: "Morado" })).toHaveAttribute("aria-checked", "true");
    // Se navega con el teclado como cualquier grupo de opciones.
    within(group).getByRole("radio", { name: "Morado" }).focus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(document.documentElement.dataset.accent).toBe("teal");
  });

  it("el color guardado se aplica al abrir la aplicación (persistencia), en los dos temas", async () => {
    await boot("purple");
    expect(document.documentElement.dataset.accent).toBe("purple");
    applyAppearance({ ...DEFAULT_CONFIG, ui: { ...DEFAULT_CONFIG.ui, theme: "dark", accent: "purple" } });
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.dataset.accent).toBe("purple");
  });

  it("si no se puede guardar, se informa y se vuelve al color anterior", async () => {
    await boot();
    api.configSet.mockRejectedValueOnce({ code: "store.io", message: "Disco lleno" });
    await userEvent.click(within(await screen.findByRole("radiogroup", { name: "Color principal" })).getByRole("radio", { name: "Morado" }));
    expect(await screen.findByText(/Disco lleno/)).toBeInTheDocument();
    expect(document.documentElement.dataset.accent).toBe("teal");
  });

  it("las terminales abiertas cambian de color sin reiniciarse", () => {
    const term = { options: { theme: terminalTheme("teal") } } as unknown as Terminal;
    const untrack = trackTerminal(term);
    applyAppearance({ ...DEFAULT_CONFIG, ui: { ...DEFAULT_CONFIG.ui, accent: "purple" } });
    expect(term.options.theme?.cursor).toBe("#a184f2");
    applyAppearance({ ...DEFAULT_CONFIG, ui: { ...DEFAULT_CONFIG.ui, accent: "teal" } });
    expect(term.options.theme?.cursor).toBe("#25b0b3");
    untrack();
    applyAppearance({ ...DEFAULT_CONFIG, ui: { ...DEFAULT_CONFIG.ui, accent: "purple" } });
    expect(term.options.theme?.cursor).toBe("#25b0b3");
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
