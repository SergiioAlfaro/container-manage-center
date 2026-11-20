// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, bulkInventory, execPreview, fullId, historyEntry, streams } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useApp } from "@/app/store";
import { useActivity } from "@/features/activity/store";
import { EMPTY_FILTER } from "@/features/activity/logic";
import type { BulkExecRequest } from "@/shared/ipc/bindings/BulkExecRequest";
import type { ExecEvent } from "@/shared/ipc/bindings/ExecEvent";
import type { ExecItemResult } from "@/shared/ipc/bindings/ExecItemResult";
import { BulkView } from "./BulkView";
import { useBulkExec } from "./execStore";
import { useBulkRun } from "./store";

const BATCH = "1f0e0d0c-0b0a-4908-8706-050403020100";

async function boot(tab: RegExp = /Comandos masivos/) {
  const user = userEvent.setup();
  renderWithQuery(<BulkView />);
  await screen.findByRole("table", { name: "Contenedores de todos los servidores" });
  await user.click(screen.getByRole("tab", { name: tab }));
  return { user };
}

const pick = (name: string, env: string) => within(screen.getByTestId("exec-picker")).getByRole("checkbox", { name: `Seleccionar ${name} (${env})` });

function lastExec(): { req: BulkExecRequest; emit: (e: ExecEvent) => void } {
  const call = vi.mocked(api.bulkExecOpen).mock.calls.at(-1)!;
  return { req: call[0] as BulkExecRequest, emit: (e) => act(() => (call[1] as (e: ExecEvent) => void)(e)) };
}

function item(index: number, req: BulkExecRequest, over: Partial<ExecItemResult>): ExecItemResult {
  return {
    index,
    target: req.targets[index]!,
    status: "succeeded",
    output: { end: "exited", exitCode: 0, stillRunning: false, stdout: "", stderr: "", stdoutTruncated: false, stderrTruncated: false, launchFailed: false, startedAt: "2026-11-20T10:00:00Z" },
    skip: null,
    cancelled: false,
    error: null,
    stage: null,
    durationMs: 80,
    ...over,
  };
}

describe("Gestión masiva: apartados y comandos masivos", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    streams.reset();
    useBulkRun.setState({ phase: "idle", sid: null, batch: null, operation: null, targets: [], results: [], active: [], summary: null, error: null, cancelling: false, onFinish: null });
    useBulkExec.setState({ phase: "idle", sid: null, batch: null, command: null, commandText: "", targets: [], rows: [], concurrency: 0, active: [], results: [], summary: null, error: null, cancelling: false, interrupting: false, onFinish: null });
    useActivity.setState({ filter: EMPTY_FILTER });
  });
  afterEach(() => {
    vi.mocked(api.bulkInventory).mockImplementation(async () => bulkInventory());
    vi.mocked(api.bulkExecOpen).mockImplementation(async (_r, h) => streams.add(h as never));
    vi.mocked(api.bulkExecPreview).mockImplementation(async (c) => execPreview(c));
    vi.mocked(api.execTemplatesSave).mockImplementation(async (templates) => ({ ...useApp.getState().config, exec: { templates } }));
    useApp.setState((s) => ({ config: { ...s.config, exec: { templates: [] } } }));
  });

  it("organiza el módulo en seis apartados", async () => {
    const user = userEvent.setup();
    renderWithQuery(<BulkView />);
    await screen.findByRole("table");
    expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual(["Resumen", "Contenedores", "Operaciones", "Configuración masiva", "Comandos masivos", "Historial"]);
    await user.click(screen.getByRole("tab", { name: "Resumen" }));
    const summary = screen.getByTestId("bulk-summary");
    const rows = within(summary).getAllByRole("row");
    expect(rows[1]).toHaveTextContent("Este equipoConectado3");
    expect(rows[2]).toHaveTextContent("ProducciónConectado2");
    expect(rows[3]).toHaveTextContent("PruebasSin conexión0");
    await user.click(screen.getByRole("tab", { name: "Operaciones" }));
    expect(screen.getByTestId("bulk-operations-empty")).toBeInTheDocument();
  });

  it("configuración masiva: política con vista previa y etiquetas explicadas y desactivadas", async () => {
    const { user } = await boot(/Contenedores/);
    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    await user.click(screen.getByRole("tab", { name: /Configuración masiva/ }));
    const cfg = screen.getByTestId("bulk-config");
    expect(within(cfg).getByTestId("config-policies")).toHaveTextContent("unless-stopped: 2");
    expect(within(cfg).getByText(/1 creado con --rm/)).toBeInTheDocument();
    for (const n of ["Añadir etiqueta", "Modificar etiqueta", "Eliminar etiqueta"]) {
      const b = within(cfg).getByRole("button", { name: n });
      expect(b).toBeDisabled();
      expect(b).toHaveAttribute("title", "Docker no permite cambiar las etiquetas de un contenedor existente.");
    }
    await user.click(within(cfg).getByRole("button", { name: "Cambiar la política de reinicio…" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("Nueva política de reinicio");
  });

  it("historial: operaciones y comandos guardados, con las órdenes de cada lote", async () => {
    const summaryEntry = historyEntry({ id: "h1", kind: "container.exec.bulk", batch: BATCH, status: "failed", error: "1 correctos, 1 con error", transport: { via: "app", action: "Comando masivo en 2 contenedor(es): df -h" }, target: "2 contenedor(es)" });
    const order = historyEntry({ id: "h2", kind: "container.exec.run", batch: BATCH, target: "web", status: "failed", error: "Código de salida 2", exitCode: 2 });
    vi.mocked(api.historyList).mockImplementation(async (f = {}) => {
      const all = [summaryEntry, order, historyEntry({ id: "h3" })];
      const m = f.text ? all.filter((e) => e.kind.includes(f.text!)) : all;
      return { entries: m, total: m.length, stored: 3, persistent: true, sessionStart: "", location: null };
    });
    const { user } = await boot(/Historial/);
    const rows = await screen.findAllByTestId("bulk-history-row");
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent("Comando masivo");
    expect(rows[0]).toHaveTextContent("df -h");
    expect(rows[0]).toHaveTextContent("1 correctos, 1 con error");
    await user.click(within(rows[0]!).getByRole("button", { name: "Ver las órdenes de cada contenedor" }));
    const orders = await screen.findByTestId("bulk-history-orders");
    expect(orders).toHaveTextContent("web");
    expect(orders).toHaveTextContent("código 2");
    await user.click(within(rows[0]!).getByRole("button", { name: /Ver en actividad/ }));
    expect(useActivity.getState().filter.batch).toBe(BATCH);
  });

  it("editor: modalidad directa con argumentos exactos y riesgo calculado por el backend", async () => {
    const { user } = await boot();
    expect(screen.getByTestId("exec-preview")).toHaveTextContent("Completa el comando");
    await user.type(screen.getByRole("textbox", { name: "Ejecutable" }), "echo");
    await user.type(screen.getByRole("textbox", { name: /Argumentos/ }), "a b{Enter}$(id)");
    const argv = await screen.findByTestId("exec-argv");
    expect(within(argv).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["echo", "a b", "$(id)"]);
    expect(screen.getByTestId("exec-risk")).toHaveTextContent("Informativo");
    expect(api.bulkExecPreview).toHaveBeenLastCalledWith({ command: { mode: "direct", program: "echo", args: ["a b", "$(id)"] }, user: null, workdir: null, timeoutSecs: 30 });
    // Sin contenedores en ejecución seleccionados no se puede ejecutar.
    expect(screen.getByTestId("exec-start")).toBeDisabled();
  });

  it("editor: intérprete elegido, secuencia de varias líneas, usuario, directorio y errores de validación", async () => {
    const { user } = await boot();
    await user.click(screen.getByRole("radio", { name: "Secuencia con un intérprete" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Intérprete" }), "/bin/bash");
    await user.type(screen.getByRole("textbox", { name: "Secuencia" }), "apt-get update{Enter}echo fin");
    await user.type(screen.getByRole("textbox", { name: "Usuario" }), "root");
    await user.type(screen.getByRole("textbox", { name: "Directorio de trabajo" }), "/tmp");
    await waitFor(() =>
      expect(api.bulkExecPreview).toHaveBeenLastCalledWith({ command: { mode: "script", interpreter: "/bin/bash", flag: "-c", script: "apt-get update\necho fin" }, user: "root", workdir: "/tmp", timeoutSecs: 30 }),
    );
    expect(await screen.findByTestId("exec-risk")).toHaveTextContent("Administrativo");
    expect(screen.getByText(/no funcionar en sh, BusyBox/)).toBeInTheDocument();
    // Otro intérprete: ruta libre.
    await user.selectOptions(screen.getByRole("combobox", { name: "Intérprete" }), "custom");
    expect(screen.getByTestId("exec-preview")).toHaveTextContent("Completa el comando");
    // Error de validación del backend.
    vi.mocked(api.bulkExecPreview).mockImplementation(async () => {
      throw { code: "engine.invalidInput", message: "usuario no válido", httpStatus: null, issues: null };
    });
    await user.type(screen.getByRole("textbox", { name: "Ruta del intérprete" }), "/x");
    expect(await screen.findByText("usuario no válido")).toBeInTheDocument();
  });

  it("selector: busca y añade contenedores, avisa de los que no están en ejecución", async () => {
    const { user } = await boot();
    expect(screen.getByTestId("exec-selected-count")).toHaveTextContent("0 seleccionados (0 en ejecución)");
    expect(within(screen.getByTestId("exec-picker")).getAllByRole("listitem")).toHaveLength(3);
    await user.click(pick("web", "Este equipo"));
    await user.click(screen.getByRole("checkbox", { name: "Solo en ejecución" }));
    await user.type(screen.getByRole("searchbox", { name: "Buscar contenedores para añadir" }), "db");
    await user.click(pick("db", "Este equipo"));
    expect(screen.getByTestId("exec-selected-count")).toHaveTextContent("2 seleccionados (1 en ejecución)");
    expect(screen.getByTestId("exec-selected")).toHaveTextContent("no se ejecutará: está «Detenido»");
    await user.click(screen.getByRole("button", { name: "Quitar db" }));
    expect(screen.getByTestId("exec-selected-count")).toHaveTextContent("1 seleccionado (1 en ejecución)");
  });

  it("confirmación de riesgo con el número escrito, petición exacta y resultados por contenedor", async () => {
    const { user } = await boot();
    await user.click(pick("web", "Este equipo"));
    await user.click(pick("api", "Producción"));
    await user.click(screen.getByRole("checkbox", { name: "Solo en ejecución" }));
    await user.click(pick("cache", "Este equipo"));
    await user.click(screen.getByRole("radio", { name: "Secuencia con un intérprete" }));
    await user.type(screen.getByRole("textbox", { name: "Secuencia" }), "rm -rf /tmp/cache");
    await user.selectOptions(screen.getByRole("combobox", { name: "Ejecuciones a la vez" }), "2");
    await screen.findByText("Potencialmente destructivo");
    await user.click(screen.getByTestId("exec-start"));
    const dialog = screen.getByRole("dialog", { name: "Ejecutar el comando en 2 contenedores" });
    expect(within(dialog).getByTestId("exec-confirm-command")).toHaveTextContent("docker exec '<contenedor>' /bin/sh -c rm -rf /tmp/cache");
    expect(within(dialog).getByTestId("exec-confirm-envs")).toHaveTextContent("Este equipo (1) · Producción (1)");
    expect(within(dialog).getByTestId("exec-confirm-skipped")).toHaveTextContent("cache");
    expect(within(dialog).getByText(/puede cambiar o romper el sistema de archivos/)).toBeInTheDocument();
    const run = within(dialog).getByRole("button", { name: "Ejecutar en 2 contenedores" });
    expect(run).toBeDisabled();
    await user.type(within(dialog).getByRole("textbox", { name: "Número de contenedores en los que se ejecutará" }), "2");
    await user.click(run);
    await waitFor(() => expect(api.bulkExecOpen).toHaveBeenCalledTimes(1));
    const { req, emit } = lastExec();
    expect(req).toEqual({
      command: { command: { mode: "script", interpreter: "/bin/sh", flag: "-c", script: "rm -rf /tmp/cache" }, user: null, workdir: null, timeoutSecs: 30 },
      targets: [
        { server: "local", id: fullId("a"), name: "web" },
        { server: "srv-prod", id: fullId("a"), name: "api" },
      ],
      concurrency: 2,
      confirmed: true,
      confirmCount: 2,
    });

    emit({ type: "started", batch: BATCH, total: 2, concurrency: 2 });
    emit({ type: "running", index: 0 });
    const panel = screen.getByTestId("exec-run");
    expect(panel).toHaveTextContent("0 de 2 · 1 en ejecución · 1 pendientes · 2 a la vez");
    expect(within(panel).getAllByTestId("exec-item").map((li) => li.dataset.status)).toEqual(["running", "pending"]);
    const ok = item(0, req, { output: { end: "exited", exitCode: 0, stillRunning: false, stdout: "borrado\n", stderr: "aviso\n", stdoutTruncated: false, stderrTruncated: false, launchFailed: false, startedAt: "2026-11-20T10:00:00Z" } });
    const bad = item(1, req, { status: "failed", output: { end: "exited", exitCode: 127, stillRunning: false, stdout: 'OCI runtime exec failed: exec: "/bin/sh": not found', stderr: "", stdoutTruncated: false, stderrTruncated: false, launchFailed: true, startedAt: "2026-11-20T10:00:00Z" } });
    emit({ type: "item", result: ok });
    emit({ type: "item", result: bad });
    emit({
      type: "finished",
      summary: { batch: BATCH, command: req.command, total: 2, counts: { succeeded: 1, failed: 1, timedOut: 0, interrupted: 0, notRun: 0, unknown: 0 }, startedAt: "2026-11-20T10:00:00Z", finishedAt: "2026-11-20T10:00:01Z", durationMs: 900, results: [ok, bad] },
    });
    expect(within(panel).getByTestId("exec-summary")).toHaveTextContent("1 correctos, 1 con error, 0 con tiempo agotado");
    expect(within(panel).getAllByTestId("exec-item").map((li) => li.dataset.status)).toEqual(["succeeded", "failed"]);
    expect(panel).toHaveTextContent("código 127");
    // Aviso con el resultado en la parte superior.
    expect(screen.getByTestId("bulk-notice")).toHaveTextContent("Comando masivo terminado: 1 de 2 correctos.");

    // Detalle: salida estándar, de error y copia.
    await user.click(within(panel).getAllByTestId("exec-item")[0]!.querySelector("button")!);
    const detail = within(panel).getByTestId("exec-detail");
    expect(within(detail).getByTestId("exec-stdout")).toHaveTextContent("borrado");
    expect(within(detail).getByTestId("exec-stderr")).toHaveTextContent("aviso");
    expect(detail).toHaveTextContent("nginx:1.27");
    await user.click(within(detail).getAllByRole("button", { name: /Copiar/ })[0]!);
    expect(api.copyText).toHaveBeenCalledWith("borrado\n");
    await user.click(within(panel).getAllByTestId("exec-item")[1]!.querySelector("button")!);
    expect(within(panel).getByTestId("exec-detail")).toHaveTextContent("Docker no pudo lanzar el proceso");

    // Exportar: se reutiliza el guardado de informes, con el contenido redactado.
    await user.click(within(panel).getByRole("button", { name: "Exportar (Markdown)" }));
    await waitFor(() => expect(api.auditExportSave).toHaveBeenCalledWith("markdown", expect.stringContaining("## web"), "comando-masivo-1f0e0d0c"));
    expect(await within(panel).findByText(/Guardado en \/home\/u\/comando-masivo-1f0e0d0c/)).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Exportar (JSON)" }));
    await waitFor(() => expect(api.auditExportSave).toHaveBeenLastCalledWith("json", expect.stringContaining('"exitCode": 127'), expect.any(String)));

    await user.click(within(panel).getByRole("button", { name: /Ver en actividad/ }));
    expect(useActivity.getState().filter.batch).toBe(BATCH);
  });

  it("cancelar pendientes e interrumpir activas son acciones distintas", async () => {
    const { user } = await boot();
    await user.click(pick("web", "Este equipo"));
    await user.click(pick("tarea", "Producción"));
    await user.type(screen.getByRole("textbox", { name: "Ejecutable" }), "sleep");
    await user.type(screen.getByRole("textbox", { name: /Argumentos/ }), "30");
    await screen.findByTestId("exec-argv");
    await user.click(screen.getByTestId("exec-start"));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Ejecutar en 2 contenedores" }));
    await waitFor(() => expect(useBulkExec.getState().sid).not.toBeNull());
    const sid = useBulkExec.getState().sid;
    const { emit } = lastExec();
    emit({ type: "started", batch: BATCH, total: 2, concurrency: 1 });
    emit({ type: "running", index: 0 });
    const panel = screen.getByTestId("exec-run");
    expect(panel).toHaveTextContent(/Docker Exec no tiene forma de parar un proceso/);
    await user.click(within(panel).getByRole("button", { name: "Cancelar pendientes" }));
    expect(api.streamClose).toHaveBeenCalledWith(sid);
    expect(api.bulkExecInterrupt).not.toHaveBeenCalled();
    expect(panel).toHaveTextContent("Cancelando las pendientes");
    await user.click(within(panel).getByRole("button", { name: "Interrumpir activas" }));
    expect(api.bulkExecInterrupt).toHaveBeenCalledWith(sid);
    expect(panel).toHaveTextContent("Interrupción solicitada");
    const { req } = lastExec();
    emit({ type: "item", result: item(0, req, { status: "interrupted", output: { end: "interrupted", exitCode: null, stillRunning: true, stdout: "", stderr: "", stdoutTruncated: false, stderrTruncated: false, launchFailed: false, startedAt: "2026-11-20T10:00:00Z" } }) });
    emit({ type: "item", result: item(1, req, { status: "notRun", output: null, cancelled: true, durationMs: 0 }) });
    expect(within(panel).getAllByTestId("exec-item").map((li) => li.dataset.status)).toEqual(["interrupted", "notRun"]);
    expect(panel).toHaveTextContent("Cancelación solicitada");
    expect(panel).toHaveTextContent("No ejecutado");
    await user.click(within(panel).getAllByTestId("exec-item")[0]!.querySelector("button")!);
    expect(within(panel).getByTestId("exec-detail")).toHaveTextContent("El proceso sigue ejecutándose dentro del contenedor.");
  });

  it("si el backend rechaza el comando no empieza y lo explica", async () => {
    vi.mocked(api.bulkExecOpen).mockRejectedValueOnce({ code: "bulk.busy", message: "Ya hay una operación en curso sobre: web", httpStatus: null, issues: null });
    const { user } = await boot();
    await user.click(pick("web", "Este equipo"));
    await user.type(screen.getByRole("textbox", { name: "Ejecutable" }), "id");
    await screen.findByTestId("exec-argv");
    await user.click(screen.getByTestId("exec-start"));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Ejecutar en 1 contenedor" }));
    expect(await screen.findByText("El comando masivo no se inició y no se envió nada a Docker: Ya hay una operación en curso sobre: web")).toBeInTheDocument();
    expect(screen.queryByTestId("exec-run")).not.toBeInTheDocument(); // sin panel de resultados
  });

  it("comandos frecuentes: ejemplos, guardar, rechazo de secretos, usar y borrar", async () => {
    const { user } = await boot();
    await user.selectOptions(screen.getByRole("combobox", { name: "Comandos guardados y ejemplos" }), "example:disk");
    expect(screen.getByRole("textbox", { name: "Ejecutable" })).toHaveValue("df");
    expect(screen.getByRole("textbox", { name: /Argumentos/ })).toHaveValue("-h");
    await user.selectOptions(screen.getByRole("combobox", { name: "Comandos guardados y ejemplos" }), "example:environment");
    expect(await screen.findByText(/puede mostrar variables de entorno, contraseñas/)).toBeInTheDocument();

    await user.selectOptions(screen.getByRole("combobox", { name: "Comandos guardados y ejemplos" }), "example:disk");
    await screen.findByTestId("exec-argv");
    await user.click(screen.getByRole("button", { name: /Guardar como comando frecuente/ }));
    const form = screen.getByTestId("exec-save");
    await user.type(within(form).getByRole("textbox", { name: "Nombre" }), "Disco");
    vi.mocked(api.execTemplatesSave).mockRejectedValueOnce({ code: "exec.templateSecret", message: "Disco: parece contener una contraseña", httpStatus: null, issues: null });
    await user.click(within(form).getByRole("button", { name: "Guardar" }));
    expect(await within(form).findByText("Disco: parece contener una contraseña")).toBeInTheDocument();
    await user.click(within(form).getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(useApp.getState().config.exec.templates).toHaveLength(1));
    const saved = vi.mocked(api.execTemplatesSave).mock.calls.at(-1)![0][0]!;
    expect(saved).toMatchObject({ name: "Disco", mode: "direct", program: "df", args: ["-h"], timeoutSecs: 30 });

    // Usar uno guardado desde la lista.
    await user.clear(screen.getByRole("textbox", { name: "Ejecutable" }));
    await user.click(screen.getByText("Comandos guardados (1)"));
    await user.click(within(screen.getByTestId("exec-saved")).getByRole("button", { name: "Usar" }));
    expect(screen.getByRole("textbox", { name: "Ejecutable" })).toHaveValue("df");
    await user.click(within(screen.getByTestId("exec-saved")).getByRole("button", { name: "Borrar Disco" }));
    await waitFor(() => expect(useApp.getState().config.exec.templates).toHaveLength(0));
  });
});
