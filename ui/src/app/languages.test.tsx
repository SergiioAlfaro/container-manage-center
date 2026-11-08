import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected } from "@/test/apiMock";
import { App } from "./App";
import { DEFAULT_CONFIG, useApp } from "./store";
import { useEngine } from "@/features/engine/store";
import { renderWithQuery } from "@/test/render";
import i18n from "@/shared/i18n";
import { formatTimestamp, numberFormat } from "@/shared/format";
import type { Language } from "@/shared/ipc/bindings/Language";

async function boot(language: Language = "es") {
  const config = structuredClone(DEFAULT_CONFIG);
  config.ui.language = language;
  api.configGet.mockResolvedValueOnce(config);
  useApp.setState({ section: "settings", config, saveError: null, info: null });
  useEngine.setState({ state: connected, reconnecting: false, error: null });
  await act(() => useApp.getState().init());
  renderWithQuery(<App />);
}

describe("Idiomas (0.9.1): francés e italiano", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.configSet.mockImplementation(async (c) => c);
  });
  afterEach(async () => {
    await act(() => i18n.changeLanguage("es"));
  });

  it("el selector ofrece los cuatro idiomas; elegir francés cambia la interfaz y se guarda", async () => {
    await boot();
    const selector = await screen.findByRole("radiogroup", { name: "Idioma" });
    expect(within(selector).getAllByRole("radio").map((r) => r.textContent)).toEqual(["Español", "English", "Français", "Italiano"]);
    await userEvent.click(within(selector).getByRole("radio", { name: "Français" }));
    expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ language: "fr" }) }));
    expect(await screen.findByRole("heading", { level: 1, name: "Paramètres" })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("fr");
    const nav = screen.getByRole("navigation", { name: "Navigation principale" });
    expect(within(nav).getByRole("button", { name: "Conteneurs" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Terminaux" })).toBeInTheDocument();
    // Y luego italiano.
    await userEvent.click(within(screen.getByRole("radiogroup", { name: "Langue" })).getByRole("radio", { name: "Italiano" }));
    expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ language: "it" }) }));
    expect(await screen.findByRole("heading", { level: 1, name: "Impostazioni" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Terminali" })).toBeInTheDocument();
  });

  it("el idioma guardado se aplica al abrir la aplicación (persistencia)", async () => {
    await boot("it");
    expect(await screen.findByRole("heading", { level: 1, name: "Impostazioni" })).toBeInTheDocument();
    expect(i18n.language).toBe("it");
  });

  it("diálogos, avisos de error y validación aparecen traducidos", async () => {
    await boot("fr");
    // Diálogo de confirmación de Configuración.
    await userEvent.click(screen.getByRole("button", { name: "Réinitialiser" }));
    const dialog = screen.getByRole("dialog", { name: "Réinitialiser les préférences ?" });
    expect(within(dialog).getByRole("button", { name: "Annuler" })).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Annuler" }));
    // Mensajes de error con texto fijo del backend.
    const { localizeError } = await vi.importActual<typeof import("@/shared/ipc/client")>("@/shared/ipc/client");
    expect(localizeError({ code: "engine.notConnected", message: "Sin conexión", httpStatus: null, issues: null }).message).toBe("Pas de connexion à Docker Engine.");
    expect(localizeError({ code: "packages.cancelled", message: "x", httpStatus: null, issues: null }).message).toBe("L’authentification a été annulée : rien n’a été désinstallé.");
    // Validación (clave usada por el asistente de terminales).
    expect(i18n.t("terminals.nameProblem.long", { max: 60 })).toBe("Utilisez au maximum 60 caractères.");
  });

  it("fechas y números siguen el formato de cada idioma", () => {
    expect(numberFormat("fr").format(10000.5)).toMatch(/^10\s000,5$/);
    expect(numberFormat("it").format(10000.5)).toBe("10.000,5");
    const d = new Date("2026-12-01T10:00:00Z");
    expect(formatTimestamp(d, "fr", { date: true })).toMatch(/01\/12\/2026/);
    expect(formatTimestamp(d, "it", { date: true })).toMatch(/01\/12\/2026/);
  });
});
