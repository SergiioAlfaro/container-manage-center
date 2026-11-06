import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api } from "@/test/apiMock";
import { App } from "./App";
import { DEFAULT_CONFIG, useApp } from "./store";
import { useEngine } from "@/features/engine/store";
import { renderWithQuery } from "@/test/render";
const DEFAULT = DEFAULT_CONFIG;
const configSet = api.configSet;

async function boot() {
  useApp.setState({ section: "dashboard", config: structuredClone(DEFAULT), saveError: null, info: null });
  useEngine.setState({ state: null, reconnecting: false, error: null });
  await act(() => useApp.getState().init());
  renderWithQuery(<App />);
}

describe("App", () => {
  beforeEach(() => {
    configSet.mockReset();
    configSet.mockImplementation(async (c) => c);
  });

  it("muestra los nueve apartados en el orden requerido", async () => {
    await boot();
    const nav = screen.getByRole("navigation", { name: "Navegación principal" });
    const labels = within(nav)
      .getAllByRole("listitem")
      .map((li) => li.textContent);
    expect(labels).toEqual([
      "Dashboard",
      "Contenedores",
      "Imágenes",
      "Redes",
      "Volúmenes",
      "Directorios vinculados",
      "Docker Compose",
      "Eventos y registros",
      "Configuración",
    ]);
  });

  it("navega entre apartados; los pendientes no tienen botones de acción falsos", async () => {
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Volúmenes" }));
    expect(screen.getByRole("heading", { level: 1, name: "Volúmenes" })).toBeInTheDocument();
    expect(screen.getByText("Disponible a partir de la fase 6")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Volúmenes" })).toHaveAttribute("aria-current", "page");
    // El área central de un apartado pendiente no contiene botones.
    const main = screen.getByRole("main");
    const content = main.querySelector(".overflow-y-auto")!;
    const centralButtons = within(content as HTMLElement).queryAllByRole("button");
    expect(centralButtons).toHaveLength(0);
  });

  it("contrae la barra lateral y lo guarda", async () => {
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Contraer barra lateral" }));
    expect(configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ sidebarCollapsed: true }) }));
    expect(screen.getByRole("button", { name: "Expandir barra lateral" })).toBeInTheDocument();
  });

  it("abre y cierra el panel de actividad con el botón y con Ctrl+Mayús+A", async () => {
    await boot();
    expect(screen.queryByRole("complementary", { name: "Actividad" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Abrir panel de actividad" }));
    expect(screen.getByRole("complementary", { name: "Actividad" })).toBeInTheDocument();
    await userEvent.keyboard("{Control>}{Shift>}A{/Shift}{/Control}");
    expect(screen.queryByRole("complementary", { name: "Actividad" })).not.toBeInTheDocument();
    expect(configSet).toHaveBeenCalledTimes(2);
  });

  it("cambia a tema oscuro e idioma inglés desde Configuración", async () => {
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Configuración" }));
    await userEvent.click(screen.getByRole("radio", { name: "Oscuro" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    await userEvent.click(screen.getByRole("radio", { name: "English" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
    expect(configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ theme: "dark", language: "en" }) }));
  });

  it("si guardar falla, revierte el cambio y muestra el error", async () => {
    configSet.mockRejectedValueOnce({ code: "config.io", message: "disco lleno" });
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Configuración" }));
    await userEvent.click(screen.getByRole("radio", { name: "Oscuro" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo guardar la configuración: disco lleno");
    expect(screen.getByRole("radio", { name: "Sistema" })).toHaveAttribute("aria-checked", "true");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("limita las operaciones conservadas al rango permitido", async () => {
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Configuración" }));
    const input = screen.getByLabelText("Operaciones conservadas");
    await userEvent.clear(input);
    await userEvent.type(input, "5{Enter}");
    expect(configSet).toHaveBeenLastCalledWith(expect.objectContaining({ activity: expect.objectContaining({ maxRecords: 50 }) }));
  });

  it("abre el panel automáticamente cuando llega una operación fallida", async () => {
    let handler: ((e: unknown) => void) | undefined;
    api.onActivity.mockImplementationOnce(async (h) => {
      handler = h as (e: unknown) => void;
      return () => {};
    });
    await boot();
    const { apiRecord } = await import("@/test/fixtures");
    await act(async () => handler!({ type: "upsert", record: apiRecord({ status: "failed", error: "boom" }) }));
    expect(screen.getByRole("complementary", { name: "Actividad" })).toBeInTheDocument();
  });
});
