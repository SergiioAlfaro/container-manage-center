import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, updateCheck } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { DEFAULT_CONFIG, useApp } from "@/app/store";
import { UpdatesSettings } from "./UpdatesSettings";
import { SystemSettings, systemReport } from "./SystemSettings";
import { SettingsView } from "./SettingsView";

function config(over: Partial<typeof DEFAULT_CONFIG.updates> = {}) {
  const c = structuredClone(DEFAULT_CONFIG);
  c.updates = { ...c.updates, ...over };
  return c;
}

describe("Configuración: actualizaciones", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
    useApp.setState({ config: config(), saveError: null });
  });

  it("sin fuente: no se puede comprobar ni activar al iniciar; muestra la versión y cómo está instalada", async () => {
    renderWithQuery(<UpdatesSettings />);
    expect(await screen.findByText("Paquete .deb instalado (0.9.0)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Comprobar ahora" })).toBeDisabled();
    expect(screen.getByRole("switch", { name: "Comprobar al iniciar la aplicación" })).toBeDisabled();
    expect(screen.getByText("Indica primero dónde se publican las versiones.")).toBeInTheDocument();
    expect(api.updatesCheck).not.toHaveBeenCalled();
  });

  it("comprueba con la fuente escrita, muestra la versión nueva y guarda la fuente que funciona", async () => {
    renderWithQuery(<UpdatesSettings />);
    await userEvent.type(screen.getByLabelText("Dónde se publican las versiones"), " mi-org/dcc ");
    await userEvent.click(screen.getByRole("button", { name: "Comprobar ahora" }));
    expect(api.updatesCheck).toHaveBeenCalledWith(" mi-org/dcc ");
    const result = await screen.findByTestId("update-result");
    expect(result).toHaveTextContent("Hay una versión nueva: 0.9.3 (tienes la 0.9.0).");
    expect(result).toHaveTextContent("Consultado https://api.github.com/repos/mi-org/dcc/releases/latest");
    await waitFor(() => expect(api.configSet).toHaveBeenCalledWith(expect.objectContaining({ updates: { checkOnStart: false, source: "mi-org/dcc" } })));
    // Abrir la página usa el comando del backend (solo https).
    await userEvent.click(within(result).getByRole("button", { name: "Abrir la página de la versión" }));
    expect(api.openReleasePage).toHaveBeenCalledWith("https://github.com/mi-org/dcc/releases/tag/v0.9.3");
    expect(result).toHaveTextContent("publicada el 01/12/2026 11:00:00");
    await userEvent.click(within(result).getByText("Novedades"));
    expect(within(result).getByText("Correcciones de errores")).toBeVisible();
  });

  it("al día; y un error se muestra sin guardar la fuente", async () => {
    useApp.setState({ config: config({ source: "mi-org/dcc" }) });
    api.updatesCheck.mockResolvedValueOnce(updateCheck({ latest: "0.9.0", newer: false, url: null, notes: null }));
    renderWithQuery(<UpdatesSettings />);
    expect(screen.getByLabelText("Dónde se publican las versiones")).toHaveValue("mi-org/dcc");
    await userEvent.click(screen.getByRole("button", { name: "Comprobar ahora" }));
    expect(await screen.findByText("Tienes la versión más reciente (0.9.0; la publicada es la 0.9.0).")).toBeInTheDocument();

    // Una fecha sin hora se muestra sin hora.
    api.updatesCheck.mockResolvedValueOnce(updateCheck({ published: "2026-12-01" }));
    await userEvent.click(screen.getByRole("button", { name: "Comprobar ahora" }));
    expect(await screen.findByText(/publicada el 01\/12\/2026$/)).toBeInTheDocument();

    api.updatesCheck.mockRejectedValueOnce({ code: "updates.insecureSource", message: "Solo se admiten direcciones https:// (o http:// hacia esta misma máquina).", httpStatus: null, issues: null });
    const input = screen.getByLabelText("Dónde se publican las versiones");
    await userEvent.clear(input);
    await userEvent.type(input, "http://ejemplo.es/v.json{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Solo se admiten direcciones https://");
    expect(api.configSet).not.toHaveBeenCalled();
  });

  it("comprobar al iniciar se guarda; el resultado de la última comprobación se muestra al volver", async () => {
    useApp.setState({ config: config({ source: "mi-org/dcc" }) });
    api.updatesLast.mockResolvedValueOnce({ check: null, error: "No se pudo consultar https://api.github.com/…: no se pudo conectar", automatic: true, at: "2026-12-02T09:00:00Z" });
    renderWithQuery(<UpdatesSettings />);
    expect(await screen.findByText(/La última comprobación falló: No se pudo consultar/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch", { name: "Comprobar al iniciar la aplicación" }));
    expect(api.configSet).toHaveBeenCalledWith(expect.objectContaining({ updates: { checkOnStart: true, source: "mi-org/dcc" } }));
  });
});

describe("Configuración: sistema y restablecer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
    useApp.setState({ config: config(), saveError: null });
  });

  it("muestra la aplicación, el equipo y Docker con datos reales y copia un informe", async () => {
    renderWithQuery(<SystemSettings />);
    expect(await screen.findByText("6.12.48+deb13-amd64")).toBeInTheDocument();
    // El sistema del equipo y el que informa Docker (aquí el mismo).
    expect(await screen.findAllByText("Debian GNU/Linux 13 (trixie)")).toHaveLength(2);
    expect(screen.getByText("2.41")).toBeInTheDocument();
    expect(screen.getByText("GNOME · wayland")).toBeInTheDocument();
    expect(await screen.findByText("29.9.2 (API 1.56)")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Copiar" }));
    const report = api.copyText.mock.calls[0]![0] as string;
    expect(report).toContain("## Aplicación\nVersión: 0.9.0\nPaquete .deb: 0.9.0");
    expect(report).toContain("glibc: 2.41");
    expect(report).toContain("Docker Engine: 29.9.2 (API 1.56)");
  });

  it("informe: valores ausentes como «—»", () => {
    expect(systemReport([["A", [["x", null], ["y", "1"]]]])).toBe("## A\nx: —\ny: 1");
  });

  it("restablecer pide confirmación y no toca la conexión con Docker", async () => {
    const c = config({ source: "mi-org/dcc", checkOnStart: true });
    c.ui.theme = "dark";
    c.activity.maxRecords = 2000;
    c.docker = { mode: "custom", customSocket: "/run/user/1000/docker.sock" };
    useApp.setState({ config: c });
    renderWithQuery(<SettingsView />);
    await userEvent.click(screen.getByRole("button", { name: "Restablecer" }));
    const dialog = screen.getByRole("dialog", { name: "¿Restablecer las preferencias?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(api.configSet).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Restablecer" }));
    await act(() => userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Restablecer" })));
    const saved = api.configSet.mock.calls.at(-1)![0];
    expect(saved.ui).toEqual(DEFAULT_CONFIG.ui);
    expect(saved.activity).toEqual(DEFAULT_CONFIG.activity);
    expect(saved.updates).toEqual(DEFAULT_CONFIG.updates);
    expect(saved.docker).toEqual({ mode: "custom", customSocket: "/run/user/1000/docker.sock" });
  });
});
