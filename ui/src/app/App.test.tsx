import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
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

  it("muestra los diez apartados en el orden requerido (0.9.1 añade «Terminales»)", async () => {
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
      "Terminales",
      "Eventos y registros",
      "Configuración",
    ]);
  });

  it("navega entre apartados; desde la Fase 8 ninguno queda pendiente", async () => {
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Eventos y registros" }));
    expect(screen.getByRole("heading", { level: 1, name: "Eventos y registros" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Eventos y registros" })).toHaveAttribute("aria-current", "page");
    // Los apartados se cargan al abrirlos (Fase 9).
    expect(await screen.findByRole("radio", { name: "En directo" })).toBeInTheDocument();
    // Ningún apartado del menú muestra el aviso de "disponible a partir de la fase N".
    const nav = screen.getByRole("navigation", { name: "Navegación principal" });
    for (const item of within(nav).getAllByRole("button").filter((b) => b.textContent && !/Contraer|Expandir/.test(b.textContent))) {
      await userEvent.click(item);
      // Se comprueba con el apartado ya cargado, no mientras se carga.
      await waitFor(() => expect(screen.queryByText("Cargando…")).not.toBeInTheDocument());
      expect(screen.getByRole("main").textContent?.length).toBeGreaterThan(40);
      expect(screen.queryByText(/Disponible a partir de la fase/)).not.toBeInTheDocument();
    }
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
    await userEvent.click(await screen.findByRole("radio", { name: "Oscuro" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    await userEvent.click(screen.getByRole("radio", { name: "English" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
    expect(configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ theme: "dark", language: "en" }) }));
  });

  it("si guardar falla, revierte el cambio y muestra el error", async () => {
    configSet.mockRejectedValueOnce({ code: "config.io", message: "disco lleno" });
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Configuración" }));
    await userEvent.click(await screen.findByRole("radio", { name: "Oscuro" }));
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

  it("el historial entre sesiones se puede desactivar desde Configuración", async () => {
    await boot();
    await userEvent.click(screen.getByRole("button", { name: "Configuración" }));
    const sw = screen.getByRole("switch", { name: "Guardar el historial entre sesiones" });
    expect(sw).toHaveAttribute("aria-checked", "true");
    await userEvent.click(sw);
    expect(configSet).toHaveBeenLastCalledWith(expect.objectContaining({ activity: expect.objectContaining({ keepHistory: false, maxRecords: 500 }) }));
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

  it("si la comprobación al iniciar encuentra una versión nueva, lo avisa y lleva a Configuración", async () => {
    let handler: ((o: import("@/shared/ipc/bindings/UpdateOutcome").UpdateOutcome) => void) | undefined;
    api.onUpdateResult.mockImplementationOnce(async (h) => {
      handler = h;
      return () => {};
    });
    await boot();
    await waitFor(() => expect(handler).toBeDefined());
    const { updateCheck } = await import("@/test/apiMock");
    // Sin versión nueva no hay aviso.
    act(() => handler!({ check: updateCheck({ newer: false, latest: "0.9.0" }), error: null, automatic: true, at: "2026-12-02T09:00:00Z" }));
    expect(screen.queryByTestId("update-banner")).not.toBeInTheDocument();
    act(() => handler!({ check: updateCheck(), error: null, automatic: true, at: "2026-12-02T09:00:00Z" }));
    expect(screen.getByTestId("update-banner")).toHaveTextContent("Hay una versión nueva de Docker Control Center: 0.9.3.");
    await userEvent.click(screen.getByRole("button", { name: "Ver en Configuración" }));
    expect(useApp.getState().section).toBe("settings");
    await userEvent.click(within(screen.getByTestId("update-banner")).getByRole("button", { name: "Cerrar aviso" }));
    expect(screen.queryByTestId("update-banner")).not.toBeInTheDocument();
  });
});
