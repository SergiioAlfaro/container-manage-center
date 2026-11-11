import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected } from "@/test/apiMock";
import { App } from "./App";
import { DEFAULT_CONFIG, useApp } from "./store";
import { renderWithQuery, setServerState } from "@/test/render";
import i18n from "@/shared/i18n";
import { formatTimestamp, numberFormat } from "@/shared/format";
import type { Language } from "@/shared/ipc/bindings/Language";

async function boot(language: Language = "es") {
  const config = structuredClone(DEFAULT_CONFIG);
  config.ui.language = language;
  api.configGet.mockResolvedValueOnce(config);
  useApp.setState({ section: "settings", config, saveError: null, info: null });
  setServerState(connected);
  await act(() => useApp.getState().init());
  renderWithQuery(<App />);
}

describe("Idiomas: ocho idiomas (0.9.1 francés e italiano; 0.9.2 catalán, gallego, euskera y portugués)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.configSet.mockImplementation(async (c) => c);
  });
  afterEach(async () => {
    await act(() => i18n.changeLanguage("es"));
  });

  it("el selector ofrece los ocho idiomas en el orden pedido; elegir francés cambia la interfaz y se guarda", async () => {
    await boot();
    const selector = await screen.findByRole("combobox", { name: "Idioma" });
    expect(within(selector).getAllByRole("option").map((o) => o.textContent)).toEqual([
      "English", "Español", "Català", "Galego", "Euskara", "Français", "Italiano", "Português",
    ]);
    // Cada opción declara su idioma (lectores de pantalla).
    expect(within(selector).getAllByRole("option").map((o) => o.getAttribute("lang"))).toEqual(["en", "es", "ca", "gl", "eu", "fr", "it", "pt"]);
    expect(selector).toHaveValue("es");
    await userEvent.selectOptions(selector, "Français");
    expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ language: "fr" }) }));
    expect(await screen.findByRole("heading", { level: 1, name: "Paramètres" })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("fr");
    const nav = screen.getByRole("navigation", { name: "Navigation principale" });
    expect(within(nav).getByRole("button", { name: "Conteneurs" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Terminaux" })).toBeInTheDocument();
    // Y luego italiano.
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Langue" }), "Italiano");
    expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ language: "it" }) }));
    expect(await screen.findByRole("heading", { level: 1, name: "Impostazioni" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Terminali" })).toBeInTheDocument();
  });

  it.each([
    ["Català", "ca", "Configuració", "Navegació principal", "Contenidors", "Terminals", "Idioma"],
    ["Galego", "gl", "Configuración", "Navegación principal", "Contedores", "Terminais", "Idioma"],
    ["Euskara", "eu", "Konfigurazioa", "Nabigazio nagusia", "Edukiontziak", "Terminalak", "Hizkuntza"],
    ["Português", "pt", "Configuração", "Navegação principal", "Contentores", "Terminais", "Idioma"],
  ])("elegir %s cambia toda la interfaz al momento y se guarda", async (name, code, title, navLabel, containers, terminals, languageLabel) => {
    await boot();
    await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Idioma" }), name);
    expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ ui: expect.objectContaining({ language: code }) }));
    expect(await screen.findByRole("heading", { level: 1, name: title })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe(code);
    const nav = screen.getByRole("navigation", { name: navLabel });
    expect(within(nav).getByRole("button", { name: containers })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: terminals })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: languageLabel })).toHaveValue(code);
  });

  it("los caracteres especiales se muestran tal cual (ç, ã, ·, ñ)", async () => {
    await boot("pt");
    expect(await screen.findByRole("heading", { level: 1, name: "Configuração" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Roxo" })).toBeInTheDocument();
    await act(() => i18n.changeLanguage("ca"));
    expect(i18n.t("common.cancel")).toBe("Cancel·lar");
    expect(i18n.t("sections.settings.title")).toBe("Configuració");
    await act(() => i18n.changeLanguage("es"));
    expect(i18n.t("settings.accentOptions.teal")).toBe("Verde azulado");
    expect(screen.getByRole("option", { name: "Português" })).toBeInTheDocument();
  });

  it.each([
    ["it", "Impostazioni"],
    ["eu", "Konfigurazioa"],
    ["pt", "Configuração"],
  ])("el idioma guardado (%s) se aplica al abrir la aplicación (persistencia)", async (lang, title) => {
    await boot(lang as Language);
    expect(await screen.findByRole("heading", { level: 1, name: title })).toBeInTheDocument();
    expect(i18n.language).toBe(lang);
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
    // Lo mismo en los idiomas nuevos.
    for (const [lang, text] of [
      ["ca", "No hi ha connexió amb Docker Engine."],
      ["gl", "Non hai conexión con Docker Engine."],
      ["eu", "Ez dago Docker Enginerekiko konexiorik."],
      ["pt", "Não há ligação ao Docker Engine."],
    ] as const) {
      await act(() => i18n.changeLanguage(lang));
      expect(localizeError({ code: "engine.notConnected", message: "Sin conexión", httpStatus: null, issues: null }).message, lang).toBe(text);
    }
    expect(i18n.t("terminals.nameProblem.long", { max: 60 })).toBe("Use no máximo 60 carateres.");
  });

  it("fechas y números siguen el formato de cada idioma", () => {
    expect(numberFormat("fr").format(10000.5)).toMatch(/^10\s000,5$/);
    expect(numberFormat("it").format(10000.5)).toBe("10.000,5");
    const d = new Date("2026-12-01T10:00:00Z");
    expect(formatTimestamp(d, "fr", { date: true })).toMatch(/01\/12\/2026/);
    expect(formatTimestamp(d, "it", { date: true })).toMatch(/01\/12\/2026/);
    // 0.9.2: portugués de Portugal y lenguas peninsulares.
    expect(numberFormat("pt").format(10000.5)).toMatch(/^10\s000,5$/);
    expect(numberFormat("ca").format(10000.5)).toBe("10.000,5");
    expect(formatTimestamp(d, "pt", { date: true })).toMatch(/01\/12\/2026/);
    expect(formatTimestamp(d, "gl", { date: true })).toMatch(/01\/12\/2026/);
  });
});
