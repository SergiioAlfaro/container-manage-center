// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, details, streams } from "@/test/apiMock";
import { FakeTerminal } from "@/test/xtermMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { ContainersView } from "../ContainersView";
import { useContainersNav } from "../store";
import { appendCapped, MAX_LINES } from "./LogsPanel";
import { decodeBase64 } from "./TerminalPanel";

const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));

async function openDetail(tab: "summary" | "logs" | "terminal" | "stats" | "inspect" = "summary") {
  useContainersNav.getState().showDetail("a1b2c3d4e5f6a7b8c9d0", "web", tab);
  const r = renderWithQuery(<ContainersView />);
  await screen.findByRole("tab", { name: "Resumen" });
  return r;
}

describe("utilidades de flujos", () => {
  it("limita las líneas de log conservando las más recientes", () => {
    const mk = (n: number) => Array.from({ length: n }, (_, i) => ({ stream: "stdout" as const, ts: null, text: String(i) }));
    const r = appendCapped(mk(MAX_LINES - 1), mk(3));
    expect(r).toHaveLength(MAX_LINES);
    expect(r.at(-1)!.text).toBe("2");
  });
  it("decodifica la salida de la terminal sin romper UTF-8", () => {
    expect(new TextDecoder().decode(decodeBase64(b64("año ñ €")))).toBe("año ñ €");
  });
});

describe("Detalle del contenedor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    streams.reset();
    setServerState(connected);
    useContainersNav.setState({ page: { kind: "list" } });
  });

  it("se abre desde la lista y muestra la configuración real", async () => {
    renderWithQuery(<ContainersView />);
    await userEvent.click(await screen.findByRole("button", { name: "web" }));
    expect(await screen.findByRole("tab", { name: "Resumen" })).toHaveAttribute("aria-selected", "true");
    expect(api.containerInspect).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", "web");
    expect(screen.getByText("8080→80/tcp")).toBeInTheDocument();
    expect(screen.getByText("Expuestos sin publicar: 443/tcp")).toBeInTheDocument();
    expect(screen.getByText("Si falla (máx. 3 intentos)")).toBeInTheDocument();
    expect(screen.getByText("512 MiB")).toBeInTheDocument();
  });

  it("oculta los valores sensibles hasta que el usuario los muestra", async () => {
    await openDetail();
    expect(document.body.textContent).not.toContain("s3creto");
    expect(screen.getByText((_, el) => el?.tagName === "LI" && el.textContent === "PATH=/usr/bin")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Mostrar 1 valor oculto" }));
    expect(screen.getByText((_, el) => el?.tagName === "LI" && el.textContent === "DB_PASSWORD=s3creto")).toBeInTheDocument();
  });

  it("logs: abre el flujo, muestra stdout y stderr, filtra y lo cierra al salir", async () => {
    await openDetail();
    await userEvent.click(screen.getByRole("tab", { name: "Logs" }));
    await waitFor(() => expect(api.logsOpen).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", "web", 500, expect.any(Function)));
    act(() =>
      streams.emit(1, {
        event: "lines",
        lines: [
          { stream: "stdout", ts: "2026-11-05T10:00:00.000000001Z", text: "arrancando" },
          { stream: "stderr", ts: "2026-11-05T10:00:01Z", text: "aviso: algo" },
        ],
      }),
    );
    const box = screen.getByTestId("log-box");
    expect(within(box).getByText("arrancando")).toBeInTheDocument();
    expect(within(box).getByText("aviso: algo").parentElement).toHaveClass("text-err");
    expect(screen.getByText("2 líneas")).toBeInTheDocument();
    await userEvent.type(screen.getByRole("searchbox", { name: "Filtrar líneas" }), "aviso");
    expect(within(box).queryByText("arrancando")).not.toBeInTheDocument();
    // Volver a la lista cierra el flujo en el backend.
    await userEvent.click(screen.getByRole("button", { name: "Contenedores" }));
    expect(api.streamClose).toHaveBeenCalledWith(1);
  });

  it("logs: muestra cómo terminó el flujo y permite reabrirlo", async () => {
    await openDetail("logs");
    await waitFor(() => expect(api.logsOpen).toHaveBeenCalledTimes(1));
    await userEvent.click(await screen.findByRole("button", { name: "Dejar de seguir" }));
    expect(api.streamClose).toHaveBeenCalledWith(1);
    expect(await screen.findByText("Flujo cerrado por el usuario")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Volver a abrir" }));
    await waitFor(() => expect(api.logsOpen).toHaveBeenCalledTimes(2));
  });

  it("estadísticas: muestra las muestras que envía Docker", async () => {
    await openDetail("stats");
    await waitFor(() => expect(api.statsOpen).toHaveBeenCalled());
    act(() =>
      streams.emit(1, {
        event: "sample",
        sample: { cpuPercent: 12.5, onlineCpus: 4, memoryUsed: 64 * 1024 ** 2, memoryLimit: 512 * 1024 ** 2, memoryPercent: 12.5, netRx: 2048, netTx: 1024, blockRead: 0, blockWrite: 4096, pids: 3, read: "2026-11-05T10:00:00Z" },
      }),
    );
    expect(await screen.findAllByText("12,50 %")).toHaveLength(1);
    expect(screen.getByText("64 MiB")).toBeInTheDocument();
    expect(screen.getByText("de 512 MiB · 12,50 %")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("estadísticas y terminal no se abren si el contenedor no está en ejecución", async () => {
    api.containerInspect.mockResolvedValueOnce(details({ status: "exited", running: false, pid: null }));
    await openDetail("stats");
    expect(await screen.findByText(/no hay estadísticas que mostrar/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("tab", { name: "Terminal" }));
    expect(await screen.findByRole("button", { name: "Abrir terminal" })).toBeDisabled();
    expect(api.statsOpen).not.toHaveBeenCalled();
  });

  it("terminal: se abre a petición, reenvía lo tecleado y muestra la salida", async () => {
    await openDetail("terminal");
    expect(api.terminalOpen).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("radio", { name: "bash" }));
    await userEvent.click(screen.getByRole("button", { name: "Abrir terminal" }));
    await waitFor(() => expect(api.terminalOpen).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", "web", "bash", 80, 24, expect.any(Function)));
    await screen.findByText("Sesión abierta");
    const term = FakeTerminal.last!;
    act(() => streams.emit(1, { event: "output", data: b64("root@web:/# ") }));
    expect(term.text()).toContain("root@web:/# ");
    act(() => term.type("ls\r"));
    expect(api.terminalInput).toHaveBeenCalledWith(1, "ls\r");
    act(() => term.doResize(120, 40));
    expect(api.terminalResize).toHaveBeenLastCalledWith(1, 120, 40);
    // El backend informa del final de la sesión.
    act(() => streams.emit(1, { event: "end", closed: { end: { reason: "finished", summary: "El proceso terminó con código 0", exitCode: 0 }, error: null } }));
    expect(await screen.findByText("El proceso terminó con código 0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Abrir otra sesión" })).toBeEnabled();
  });

  it("inspección: muestra el JSON que devuelve Docker", async () => {
    await openDetail("inspect");
    expect(screen.getByTestId("inspect-json")).toHaveTextContent('"Name": "/web"');
  });

  it("duplicar abre el asistente con la configuración y las advertencias", async () => {
    await openDetail();
    await userEvent.click(screen.getByRole("button", { name: "Duplicar" }));
    expect(await screen.findByText("Duplicar web")).toBeInTheDocument();
    expect(screen.getByText(/Las etiquetas de Compose se han quitado/)).toBeInTheDocument();
    expect(screen.getByLabelText("Nombre")).toHaveValue("web-copia");
    expect(screen.getByLabelText("Imagen")).toHaveValue("nginx:1.27");
    await waitFor(() => expect(api.containerPreview).toHaveBeenLastCalledWith(expect.objectContaining({ name: "web-copia", env: [{ key: "MODO", value: "produccion" }] })));
  });

  it("las acciones del encabezado usan el ID real y recargan la inspección", async () => {
    await openDetail();
    await userEvent.click(screen.getByRole("button", { name: "Detener" }));
    expect(api.containerAction).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", "web", "stop");
    await waitFor(() => expect(api.containerInspect).toHaveBeenCalledTimes(2));
  });

  it("eliminar pide confirmación y vuelve a la lista", async () => {
    await openDetail();
    await userEvent.click(screen.getByRole("button", { name: "Eliminar" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByRole("button", { name: "Eliminar contenedor" })).toBeDisabled();
    await userEvent.click(within(dialog).getByLabelText("Detenerlo a la fuerza y eliminarlo"));
    await userEvent.click(within(dialog).getByRole("button", { name: "Eliminar contenedor" }));
    expect(api.containerRemove).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", "web", true, false);
    await waitFor(() => expect(useContainersNav.getState().page.kind).toBe("list"));
  });

  it("si el contenedor ya no existe lo dice", async () => {
    api.containerInspect.mockRejectedValueOnce({ code: "engine.docker", message: "No such container: web", httpStatus: 404, issues: null });
    useContainersNav.getState().showDetail("a1b2c3d4e5f6a7b8c9d0", "web");
    renderWithQuery(<ContainersView />);
    expect(await screen.findByText("El contenedor web ya no existe en Docker.")).toBeInTheDocument();
  });
});
