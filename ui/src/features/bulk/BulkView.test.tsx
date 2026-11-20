// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, bulkInventory, fullId, streams } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useApp } from "@/app/store";
import { useActivity } from "@/features/activity/store";
import { EMPTY_FILTER } from "@/features/activity/logic";
import type { BulkEvent } from "@/shared/ipc/bindings/BulkEvent";
import type { BulkItemResult } from "@/shared/ipc/bindings/BulkItemResult";
import type { BulkRequest } from "@/shared/ipc/bindings/BulkRequest";
import { BulkView } from "./BulkView";
import { useBulkRun } from "./store";

const BATCH = "0f0e0d0c-0b0a-4908-8706-050403020100";

async function boot() {
  const user = userEvent.setup();
  const r = renderWithQuery(<BulkView />);
  await screen.findByRole("table", { name: "Contenedores de todos los servidores" });
  return { user, ...r };
}

const rowNames = () => screen.getAllByTestId("bulk-row").map((tr) => within(tr).getAllByRole("cell")[1]!.textContent?.replace(/[0-9a-f]{12}$/, "").trim());
const select = (name: string, env: string) => screen.getByRole("checkbox", { name: `Seleccionar ${name} (${env})` });
const bar = () => screen.getByRole("toolbar", { name: "Acciones sobre la selección" });

function lastRun(): { req: BulkRequest; emit: (e: BulkEvent) => void } {
  const call = vi.mocked(api.bulkRunOpen).mock.calls.at(-1)!;
  return { req: call[0] as BulkRequest, emit: (e) => act(() => (call[1] as (e: BulkEvent) => void)(e)) };
}

function result(index: number, req: BulkRequest, over: Partial<BulkItemResult>): BulkItemResult {
  return { index, target: req.targets[index]!, status: "succeeded", state: "exited", skip: null, error: null, stage: null, details: null, durationMs: 120, ...over };
}

describe("Gestión masiva", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    streams.reset();
    useBulkRun.setState({ phase: "idle", sid: null, batch: null, operation: null, targets: [], results: [], active: [], summary: null, error: null, cancelling: false, onFinish: null });
    useActivity.setState({ filter: EMPTY_FILTER });
  });
  afterEach(() => {
    vi.mocked(api.bulkInventory).mockImplementation(async () => bulkInventory());
    vi.mocked(api.bulkRunOpen).mockImplementation(async (_r, h) => streams.add(h as never));
  });

  it("panel general: indicadores, servidores, última actualización y servidores sin conexión", async () => {
    await boot();
    const k = screen.getByTestId("bulk-kpis");
    const value = (label: string) => within(k).getByRole("button", { name: new RegExp(`^${label}`) }).textContent;
    expect(value("Total")).toBe("Total5");
    expect(value("En ejecución")).toBe("En ejecución3");
    expect(value("Detenidos")).toBe("Detenidos1");
    expect(value("En pausa")).toBe("En pausa1");
    expect(value("Con error")).toBe("Con error1");
    expect(screen.getByTestId("bulk-envs")).toHaveTextContent("3 servidores (2 conectados)");
    expect(screen.getByTestId("bulk-updated")).toHaveTextContent("Actualizado a las");
    expect(screen.getByText(/Sin conexión \(sus contenedores no aparecen\): Pruebas/)).toBeInTheDocument();
    // Primera carga sin volver a pedir los datos de inspect.
    expect(api.bulkInventory).toHaveBeenCalledWith(false);
  });

  it("muestra el estado de carga y los errores comprensibles", async () => {
    let resolve!: (v: ReturnType<typeof bulkInventory>) => void;
    vi.mocked(api.bulkInventory).mockImplementationOnce(() => new Promise((r) => (resolve = r)));
    renderWithQuery(<BulkView />);
    expect(screen.getByText("Cargando…")).toBeInTheDocument();
    const inv = bulkInventory();
    inv.environments[1]!.containers = [];
    inv.environments[1]!.error = { code: "engine.io", message: "conexión SSH cortada", httpStatus: null, issues: null };
    await act(async () => resolve(inv));
    expect(await screen.findByText("No se pudieron listar los contenedores de Producción: conexión SSH cortada")).toBeInTheDocument();
  });

  it("si no se puede cargar la lista lo explica", async () => {
    vi.mocked(api.bulkInventory).mockRejectedValue({ code: "engine.io", message: "sin respuesta", httpStatus: null, issues: null });
    renderWithQuery(<BulkView />);
    expect(await screen.findByText("No se pudo cargar la lista: sin respuesta")).toBeInTheDocument();
  });

  it("actualizar pide de nuevo todos los datos a Docker", async () => {
    const { user } = await boot();
    await user.click(screen.getByRole("button", { name: "Actualizar" }));
    await waitFor(() => expect(api.bulkInventory).toHaveBeenLastCalledWith(true));
  });

  it("la actualización automática se puede desactivar y se guarda en la configuración", async () => {
    const { user } = await boot();
    expect(useApp.getState().config.bulk.autoRefresh).toBe(true);
    await user.click(screen.getByRole("switch", { name: "Actualizar automáticamente" }));
    await waitFor(() => expect(useApp.getState().config.bulk.autoRefresh).toBe(false));
    expect(screen.getByRole("combobox", { name: "Intervalo de actualización" })).toBeDisabled();
  });

  it("busca por nombre o ID y filtra por estado, imagen, etiqueta y servidor", async () => {
    const { user } = await boot();
    expect(rowNames()).toEqual(["api", "cache", "db", "tarea", "web"]);
    await user.type(screen.getByRole("searchbox", { name: "Buscar por nombre o ID" }), "dddd");
    expect(rowNames()).toEqual(["tarea"]);
    await user.click(screen.getByRole("button", { name: "Quitar filtros" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Estado" }), "running");
    expect(rowNames()).toEqual(["api", "tarea", "web"]);
    await user.selectOptions(screen.getByRole("combobox", { name: "Imagen" }), "nginx:1.27");
    expect(rowNames()).toEqual(["api", "web"]);
    await user.selectOptions(screen.getByRole("combobox", { name: "Servidor" }), "srv-prod");
    expect(rowNames()).toEqual(["api"]);
    expect(screen.getByTestId("bulk-count")).toHaveTextContent("1 de 5 contenedor");
    await user.click(screen.getByRole("button", { name: "Quitar filtros" }));
    await user.type(screen.getByRole("textbox", { name: "Filtrar por etiqueta" }), "tier=backend");
    expect(rowNames()).toEqual(["db"]);
    await user.clear(screen.getByRole("textbox", { name: "Filtrar por etiqueta" }));
    // Pulsar un indicador filtra por ese estado.
    await user.click(within(screen.getByTestId("bulk-kpis")).getByRole("button", { name: /^Con error/ }));
    expect(rowNames()).toEqual(["db"]);
    await user.type(screen.getByRole("searchbox", { name: "Buscar por nombre o ID" }), "nada");
    expect(screen.getByText("Ningún contenedor coincide con los filtros.")).toBeInTheDocument();
  });

  it("ordena por columna y muestra el sentido", async () => {
    const { user } = await boot();
    const header = screen.getByRole("columnheader", { name: /Nombre/ });
    expect(header).toHaveAttribute("aria-sort", "ascending");
    await user.click(within(header).getByRole("button"));
    expect(header).toHaveAttribute("aria-sort", "descending");
    expect(rowNames()).toEqual(["web", "tarea", "db", "cache", "api"]);
    await user.click(within(screen.getByRole("columnheader", { name: /Servidor/ })).getByRole("button"));
    expect(rowNames().slice(0, 3).sort()).toEqual(["cache", "db", "web"]);
  });

  it("selección individual, de la página, de todo lo filtrado y quitar la selección", async () => {
    const { user } = await boot();
    expect(screen.queryByTestId("bulk-bar")).not.toBeInTheDocument();
    await user.click(select("web", "Este equipo"));
    expect(screen.getByTestId("bulk-selected")).toHaveTextContent("1 seleccionado");
    // El mismo ID en otro servidor no se selecciona con él.
    expect(select("api", "Producción")).not.toBeChecked();
    await user.click(within(bar()).getByRole("button", { name: "Quitar la selección" }));
    expect(screen.queryByTestId("bulk-bar")).not.toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    expect(screen.getByTestId("bulk-selected")).toHaveTextContent("5 seleccionados");
    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    expect(screen.queryByTestId("bulk-bar")).not.toBeInTheDocument();

    await user.selectOptions(screen.getByRole("combobox", { name: "Servidor" }), "srv-prod");
    await user.click(select("api", "Producción"));
    await user.click(within(bar()).getByRole("button", { name: "Seleccionar los 2 filtrados" }));
    expect(screen.getByTestId("bulk-selected")).toHaveTextContent("2 seleccionados");
  });

  it("la barra indica a cuántos se aplica cada acción y desactiva las imposibles", async () => {
    const { user } = await boot();
    await user.click(select("db", "Este equipo"));
    await user.click(select("cache", "Este equipo"));
    const b = bar();
    const start = within(b).getByRole("button", { name: /^Iniciar/ });
    expect(start).toBeEnabled();
    expect(start).toHaveAttribute("title", "Se aplicará a 1 de 2 (los demás se omiten por su estado).");
    const pause = within(b).getByRole("button", { name: /^Pausar/ });
    expect(pause).toBeDisabled();
    expect(pause).toHaveAttribute("title", "Ningún contenedor seleccionado admite «Pausar» en su estado actual.");
    expect(within(b).getByRole("button", { name: /^Reanudar/ })).toBeEnabled();
    expect(within(b).getByRole("button", { name: /^Eliminar/ })).toBeEnabled();
  });

  it("confirmación: operación, servidores, contenedores, omitidos y la petición exacta", async () => {
    const { user } = await boot();
    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    await user.click(within(bar()).getByRole("button", { name: /^Detener/ }));
    const dialog = screen.getByRole("dialog", { name: "Detener: 4 contenedores" });
    expect(within(dialog).getByTestId("bulk-confirm-envs")).toHaveTextContent("Este equipo (2) · Producción (2)");
    expect(within(dialog).getByText(/Incluye servidores remotos/)).toBeInTheDocument();
    expect(within(dialog).getByTestId("bulk-confirm-list").querySelectorAll("li")).toHaveLength(4);
    const skipped = within(dialog).getByTestId("bulk-confirm-skipped");
    expect(skipped.querySelectorAll("li")).toHaveLength(1);
    expect(skipped).toHaveTextContent("db");
    expect(skipped).toHaveTextContent("Se omite: no se puede en estado «Detenido»");
    expect(within(dialog).getByText(/Los servicios que ofrecen dejarán de estar disponibles/)).toBeInTheDocument();
    expect(within(dialog).getByText(/si alguno falla, los demás siguen/)).toBeInTheDocument();
    // El límite de operaciones a la vez se guarda en la configuración (el backend lo lee al empezar).
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "Operaciones a la vez" }), "2");
    await waitFor(() => expect(api.configSet).toHaveBeenLastCalledWith(expect.objectContaining({ bulk: expect.objectContaining({ concurrency: 2 }) })));
    await user.click(within(dialog).getByRole("button", { name: "Aplicar a 4 contenedores" }));
    await waitFor(() => expect(api.bulkRunOpen).toHaveBeenCalledTimes(1));
    const { req } = lastRun();
    expect(req.operation).toEqual({ kind: "stop" });
    expect(req.confirmed).toBe(true);
    expect(req.confirmCount).toBeNull();
    // Cada destino lleva su servidor y su ID completo; el mismo ID en dos servidores, dos destinos.
    expect(req.targets).toEqual(
      expect.arrayContaining([
        { server: "local", id: fullId("a"), name: "web" },
        { server: "srv-prod", id: fullId("a"), name: "api" },
        { server: "local", id: fullId("c"), name: "cache" },
        { server: "srv-prod", id: fullId("d"), name: "tarea" },
      ]),
    );
    expect(req.targets).toHaveLength(4);
    // La selección se vacía al empezar: no se repite por error.
    expect(screen.queryByTestId("bulk-bar")).not.toBeInTheDocument();
  });

  it("cancelar el diálogo no envía nada", async () => {
    const { user } = await boot();
    await user.click(select("web", "Este equipo"));
    await user.click(within(bar()).getByRole("button", { name: /^Reiniciar/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.bulkRunOpen).not.toHaveBeenCalled();
  });

  it("eliminar exige escribir el número y nunca toca volúmenes ni imágenes", async () => {
    const { user } = await boot();
    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    await user.click(within(bar()).getByRole("button", { name: /^Eliminar/ }));
    let dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAccessibleName("Eliminar: 1 contenedor");
    expect(within(dialog).getByText("Los volúmenes y las imágenes nunca se eliminan: solo los contenedores.")).toBeInTheDocument();
    expect(within(dialog).getByTestId("bulk-confirm-skipped")).toHaveTextContent("marca «Forzar» para eliminarlo");
    await user.click(within(dialog).getByRole("checkbox", { name: /Forzar/ }));
    dialog = screen.getByRole("dialog", { name: "Eliminar (forzando): 5 contenedores" });
    const run = within(dialog).getByRole("button", { name: "Aplicar a 5 contenedores" });
    expect(run).toBeDisabled();
    const input = within(dialog).getByRole("textbox", { name: "Número de contenedores que se eliminarán" });
    await user.type(input, "4");
    expect(run).toBeDisabled();
    await user.clear(input);
    await user.type(input, "5");
    expect(run).toBeEnabled();
    await user.click(run);
    await waitFor(() => expect(api.bulkRunOpen).toHaveBeenCalledTimes(1));
    const { req } = lastRun();
    expect(req.operation).toEqual({ kind: "remove", force: true });
    expect(req.confirmCount).toBe(5);
    expect(JSON.stringify(req)).not.toMatch(/volume/i);
  });

  it("política de reinicio: vista previa del cambio y contenedores que no lo admiten", async () => {
    const { user } = await boot();
    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    await user.click(within(bar()).getByRole("button", { name: "Política de reinicio" }));
    let dialog = screen.getByRole("dialog");
    // Por defecto unless-stopped: web y api ya la tienen; tarea es --rm.
    const skipped = within(dialog).getByTestId("bulk-confirm-skipped");
    expect(skipped).toHaveTextContent("ya tiene esa política");
    expect(skipped).toHaveTextContent("creado con --rm");
    await user.click(within(dialog).getByRole("radio", { name: "Si falla" }));
    dialog = screen.getByRole("dialog");
    const retries = within(dialog).getByRole("spinbutton", { name: /Intentos máximos/ });
    await user.clear(retries);
    await user.type(retries, "5000");
    expect(within(dialog).getByRole("button", { name: /^Aplicar a/ })).toBeDisabled();
    await user.clear(retries);
    await user.type(retries, "3");
    const list = within(dialog).getByTestId("bulk-confirm-list");
    expect(list).toHaveTextContent("always");
    expect(list).toHaveTextContent("on-failure:3");
    expect(within(dialog).getByText(/sin recrear el contenedor/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Aplicar a 4 contenedores" }));
    await waitFor(() => expect(api.bulkRunOpen).toHaveBeenCalledTimes(1));
    expect(lastRun().req.operation).toEqual({ kind: "restartPolicy", policy: { name: "on-failure", maximumRetryCount: 3 } });
    expect(lastRun().req.targets.map((t) => t.name)).not.toContain("tarea");
  });

  it("etiquetas: explica que Docker no permite cambiarlas y no envía nada", async () => {
    const { user } = await boot();
    await user.click(select("web", "Este equipo"));
    await user.click(within(bar()).getByRole("button", { name: "Etiquetas" }));
    const dialog = screen.getByRole("dialog", { name: "Etiquetas de los contenedores seleccionados" });
    expect(within(dialog).getByText(/Docker no permite añadir, cambiar ni quitar etiquetas/)).toBeInTheDocument();
    expect(within(dialog).getByText("tier=frontend")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: /Aplicar/ })).not.toBeInTheDocument();
    expect(api.bulkRunOpen).not.toHaveBeenCalled();
  });

  it("progreso y resultado parcial: correcto, fallido, desconocido y omitido, con detalle en la actividad", async () => {
    const { user } = await boot();
    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    await user.click(within(bar()).getByRole("button", { name: /^Detener/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Aplicar a 4 contenedores" }));
    await waitFor(() => expect(api.bulkRunOpen).toHaveBeenCalled());
    const { req, emit } = lastRun();
    const calls = vi.mocked(api.bulkInventory).mock.calls.length;
    emit({ type: "started", batch: BATCH, total: 4, concurrency: 4 });
    emit({ type: "running", index: 0 });
    const panel = screen.getByTestId("bulk-run");
    expect(within(panel).getByText("0 de 4 · 4 a la vez")).toBeInTheDocument();
    expect(within(panel).getAllByTestId("bulk-run-item")[0]).toHaveAttribute("data-status", "running");
    // Mientras se ejecuta no se puede lanzar otra.
    await user.click(select("db", "Este equipo"));
    expect(within(bar()).getByRole("button", { name: /^Iniciar/ })).toBeDisabled();
    expect(within(bar()).getByText("Hay una operación masiva en curso.")).toBeInTheDocument();

    const results = [
      result(0, req, {}),
      result(1, req, { status: "failed", state: null, error: { code: "engine.docker", message: "Docker rechazó la orden (409): conflicto", httpStatus: 409, issues: null }, stage: "execute" }),
      result(2, req, { status: "unknown", state: null, error: { code: "engine.io", message: "conexión cortada", httpStatus: null, issues: null }, stage: "execute" }),
      result(3, req, { status: "skipped", state: "exited", skip: { reason: "state", state: "exited" }, durationMs: 3 }),
    ];
    emit({ type: "item", result: results[0]! });
    expect(within(panel).getByText("1 de 4 · 4 a la vez")).toBeInTheDocument();
    for (const r of results.slice(1)) emit({ type: "item", result: r });
    emit({
      type: "finished",
      summary: { batch: BATCH, operation: { kind: "stop" }, total: 4, counts: { succeeded: 1, failed: 1, skipped: 1, cancelled: 0, unknown: 1 }, startedAt: "2026-11-20T10:00:00Z", finishedAt: "2026-11-20T10:00:02Z", durationMs: 2100, results },
    });
    expect(within(panel).getByTestId("bulk-run-summary")).toHaveTextContent("Resultado parcial: 1 correctas, 1 fallidas, 1 con resultado desconocido, 1 omitidas, 0 canceladas.");
    expect(within(panel).getByText(/la conexión se cortó antes de la confirmación de Docker/)).toBeInTheDocument();
    expect(within(panel).getAllByTestId("bulk-run-item").map((li) => li.dataset.status)).toEqual(["succeeded", "failed", "unknown", "skipped"]);
    expect(within(panel).getByText("Docker rechazó la orden (409): conflicto")).toBeInTheDocument();
    expect(within(panel).getByText("Ahora: Detenido")).toBeInTheDocument();
    expect(within(panel).getByText("Terminada en 2.10 s")).toBeInTheDocument();
    // Al terminar se recarga la tabla con el estado real.
    await waitFor(() => expect(vi.mocked(api.bulkInventory).mock.calls.length).toBeGreaterThan(calls));

    await user.click(within(panel).getByRole("checkbox", { name: "Mostrar solo los que no fueron correctos" }));
    expect(within(panel).getAllByTestId("bulk-run-item")).toHaveLength(3);

    await user.click(within(panel).getByRole("button", { name: /Ver en actividad/ }));
    expect(useActivity.getState().filter.batch).toBe(BATCH);
    await waitFor(() => expect(useApp.getState().config.activity.panelOpen).toBe(true));

    await user.click(within(panel).getByRole("button", { name: "Cerrar resultado" }));
    expect(screen.queryByTestId("bulk-run")).not.toBeInTheDocument();
  });

  it("todo correcto: lo indica sin alarmas", async () => {
    const { user } = await boot();
    await user.click(select("db", "Este equipo"));
    await user.click(within(bar()).getByRole("button", { name: /^Iniciar/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Aplicar a 1 contenedor" }));
    await waitFor(() => expect(api.bulkRunOpen).toHaveBeenCalled());
    const { req, emit } = lastRun();
    const r = result(0, req, { state: "running" });
    emit({ type: "started", batch: BATCH, total: 1, concurrency: 4 });
    emit({ type: "item", result: r });
    emit({ type: "finished", summary: { batch: BATCH, operation: { kind: "start" }, total: 1, counts: { succeeded: 1, failed: 0, skipped: 0, cancelled: 0, unknown: 0 }, startedAt: "", finishedAt: "", durationMs: 300, results: [r] } });
    expect(screen.getByTestId("bulk-run-summary")).toHaveTextContent("Correcta en 1 contenedor (0 omitidos).");
    expect(screen.getByText("Ahora: En ejecución")).toBeInTheDocument();
  });

  it("cancelar detiene lo pendiente a través del backend", async () => {
    const { user } = await boot();
    await user.click(screen.getByRole("checkbox", { name: "Seleccionar los de esta página" }));
    await user.click(within(bar()).getByRole("button", { name: /^Detener/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Aplicar a 4 contenedores" }));
    await waitFor(() => expect(useBulkRun.getState().sid).not.toBeNull());
    const { emit } = lastRun();
    emit({ type: "started", batch: BATCH, total: 4, concurrency: 1 });
    await user.click(screen.getByRole("button", { name: "Cancelar lo pendiente" }));
    expect(api.streamClose).toHaveBeenCalledWith(useBulkRun.getState().sid);
    expect(screen.getByText(/Cancelando lo pendiente/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancelar lo pendiente" })).toBeDisabled();
  });

  it("si el backend rechaza la operación, lo dice y no muestra progreso", async () => {
    vi.mocked(api.bulkRunOpen).mockRejectedValueOnce({ code: "bulk.busy", message: "Ya hay una operación en curso sobre: web", httpStatus: null, issues: null });
    const { user } = await boot();
    await user.click(select("web", "Este equipo"));
    await user.click(within(bar()).getByRole("button", { name: /^Detener/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Aplicar a 1 contenedor" }));
    expect(await screen.findByText("La operación masiva no se inició y no se envió nada a Docker: Ya hay una operación en curso sobre: web")).toBeInTheDocument();
    expect(screen.queryByTestId("bulk-run")).not.toBeInTheDocument();
    // La selección se conserva para poder reintentarlo a mano.
    expect(screen.getByTestId("bulk-selected")).toHaveTextContent("1 seleccionado");
  });

  it("pagina la tabla", async () => {
    const inv = bulkInventory();
    inv.environments[0]!.containers = Array.from({ length: 60 }, (_, i) => ({ ...inv.environments[0]!.containers[0]!, id: i.toString(16).padStart(64, "0"), name: `c${String(i).padStart(2, "0")}` }));
    vi.mocked(api.bulkInventory).mockResolvedValue(inv);
    const { user } = await boot();
    expect(screen.getAllByTestId("bulk-row")).toHaveLength(50);
    expect(screen.getByTestId("bulk-range")).toHaveTextContent("1–50 de 62");
    await user.click(screen.getByRole("button", { name: "Página siguiente" }));
    expect(screen.getAllByTestId("bulk-row")).toHaveLength(12);
    expect(screen.getByTestId("bulk-range")).toHaveTextContent("51–62 de 62");
  });
});
