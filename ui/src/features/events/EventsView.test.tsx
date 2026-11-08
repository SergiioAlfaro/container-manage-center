import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, dockerEvent } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { useEngine } from "@/features/engine/store";
import type { DockerEvent } from "@/shared/ipc/bindings/DockerEvent";
import { EventsView } from "./EventsView";
import { REFRESH_DELAY_MS, REFRESH_MIN_INTERVAL_MS, refreshDelay, useLiveRefresh } from "./liveRefresh";
import { useEvents } from "./store";

let emit: (e: DockerEvent) => void = () => {};

function reset() {
  vi.clearAllMocks();
  useEngine.setState({ state: connected });
  useEvents.setState({ live: [], pending: [], paused: false, loaded: false, tab: "live" });
  api.onDockerEvent.mockImplementation(async (h: (e: DockerEvent) => void) => {
    emit = h;
    return () => {};
  });
}

function Live() {
  useLiveRefresh();
  return <EventsView />;
}

describe("Eventos: en directo", () => {
  beforeEach(reset);

  it("muestra los eventos al llegar, los filtra y abre el recurso", async () => {
    const user = userEvent.setup();
    renderWithQuery(<Live />);
    expect(await screen.findByText(/Todavía no ha ocurrido nada en Docker/)).toBeInTheDocument();
    await waitFor(() => expect(api.onDockerEvent).toHaveBeenCalled());
    act(() => {
      emit(dockerEvent({ action: "create", timeNano: 1e18 }));
      emit(dockerEvent({ action: "start", timeNano: 1e18 + 1 }));
      emit(dockerEvent({ kind: "volume", action: "create", actorId: "datos", name: null, attributes: {}, timeNano: 1e18 + 2 }));
    });
    const list = screen.getByTestId("event-list");
    expect(within(list).getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      expect.stringContaining("Volumencreatedatos"),
      expect.stringContaining("Contenedorstartweb"),
      expect.stringContaining("Contenedorcreateweb"),
    ]);
    expect(screen.getByTestId("events-live-status")).toHaveTextContent("En directo");
    await user.click(screen.getByRole("button", { name: "Volumen" }));
    expect(within(screen.getByTestId("event-list")).getAllByRole("listitem")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Todos" }));
    await user.type(screen.getByPlaceholderText("Buscar por nombre, ID, acción o imagen"), "start");
    expect(within(screen.getByTestId("event-list")).getAllByRole("listitem")).toHaveLength(1);
    // El nombre lleva al detalle del contenedor.
    await user.click(screen.getByRole("button", { name: "web" }));
    expect(useApp.getState().section).toBe("containers");
    expect(useContainersNav.getState().page).toMatchObject({ kind: "detail", id: "c0ffee0123456789", name: "web" });
  });

  it("un evento desplegado sigue desplegado al llegar otros (Fase 9)", async () => {
    const user = userEvent.setup();
    renderWithQuery(<Live />);
    await waitFor(() => expect(api.onDockerEvent).toHaveBeenCalled());
    act(() => emit(dockerEvent({ action: "die", timeNano: 1e18, attributes: { name: "web", exitCode: "137", image: "nginx:1.27" } })));
    await user.click(screen.getByRole("button", { name: "Detalles del evento" }));
    expect(screen.getByText("exitCode")).toBeInTheDocument();
    act(() => {
      emit(dockerEvent({ action: "destroy", timeNano: 1e18 + 5 }));
      emit(dockerEvent({ action: "create", timeNano: 1e18 + 9 }));
    });
    expect(within(screen.getByTestId("event-list")).getAllByRole("listitem")).toHaveLength(3);
    // El detalle abierto es el del evento `die`, que ahora está el tercero.
    expect(screen.getByText("exitCode")).toBeInTheDocument();
    const expanded = screen.getAllByRole("button", { name: "Detalles del evento" }).map((b) => b.getAttribute("aria-expanded"));
    expect(expanded).toEqual(["false", "false", "true"]);
  });

  it("en pausa guarda los nuevos y los muestra al reanudar; los detalles muestran los atributos", async () => {
    const user = userEvent.setup();
    renderWithQuery(<Live />);
    await waitFor(() => expect(api.onDockerEvent).toHaveBeenCalled());
    act(() => emit(dockerEvent({ action: "create" })));
    await user.click(screen.getByRole("button", { name: "Pausar" }));
    act(() => {
      emit(dockerEvent({ action: "start" }));
      emit(dockerEvent({ action: "die", attributes: { name: "web", exitCode: "137", image: "nginx:1.27" } }));
    });
    expect(within(screen.getByTestId("event-list")).getAllByRole("listitem")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Reanudar (2 nuevos)" }));
    expect(within(screen.getByTestId("event-list")).getAllByRole("listitem")).toHaveLength(3);
    await user.click(within(screen.getByTestId("event-list")).getAllByRole("button", { name: "Detalles del evento" })[0]!);
    expect(screen.getByText("exitCode")).toBeInTheDocument();
    expect(screen.getByText("137")).toBeInTheDocument();
  });

  it("cada evento recarga los datos afectados de toda la aplicación, agrupando ráfagas", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const qc = new QueryClient();
    const spy = vi.spyOn(qc, "invalidateQueries");
    render(
      <QueryClientProvider client={qc}>
        <Live />
      </QueryClientProvider>,
    );
    await waitFor(() => expect(api.onDockerEvent).toHaveBeenCalled());
    act(() => {
      for (let i = 0; i < 20; i++) emit(dockerEvent({ action: i % 2 ? "start" : "create" }));
      emit(dockerEvent({ action: "exec_create: sh" }));
    });
    expect(spy).not.toHaveBeenCalled();
    act(() => void vi.advanceTimersByTime(REFRESH_DELAY_MS + 10));
    const keys = spy.mock.calls.map((c) => JSON.stringify(c[0]!.queryKey));
    expect(keys).toEqual(expect.arrayContaining(['["containers"]', '["overview"]', '["composeProjects"]']));
    expect(new Set(keys).size).toBe(keys.length);
    spy.mockClear();
    act(() => emit(dockerEvent({ action: "exec_die" })));
    act(() => void vi.advanceTimersByTime(REFRESH_DELAY_MS + 10));
    expect(spy).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("en una ráfaga larga recarga como mucho una vez por segundo y nunca pierde la última", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: false });
    const qc = new QueryClient();
    const spy = vi.spyOn(qc, "invalidateQueries");
    render(
      <QueryClientProvider client={qc}>
        <Live />
      </QueryClientProvider>,
    );
    await vi.waitFor(() => expect(api.onDockerEvent).toHaveBeenCalled());
    const flushes = () => spy.mock.calls.filter((c) => JSON.stringify(c[0]!.queryKey) === '["containers"]').length;
    // 100 eventos, uno cada 370 ms (lo que tarda un `docker start`): 37 s.
    for (let i = 0; i < 100; i++) {
      act(() => emit(dockerEvent({ action: "start", timeNano: 1e18 + i })));
      act(() => void vi.advanceTimersByTime(370));
    }
    act(() => void vi.advanceTimersByTime(REFRESH_MIN_INTERVAL_MS));
    // Antes: una recarga por evento (100). Ahora: ~1 por segundo.
    expect(flushes()).toBeGreaterThanOrEqual(36);
    expect(flushes()).toBeLessThanOrEqual(39);
    // El último evento llega a la interfaz: tras él hay siempre una recarga.
    spy.mockClear();
    act(() => emit(dockerEvent({ action: "die", timeNano: 2e18 })));
    act(() => void vi.advanceTimersByTime(REFRESH_DELAY_MS - 10));
    expect(flushes()).toBe(0);
    act(() => void vi.advanceTimersByTime(REFRESH_MIN_INTERVAL_MS));
    expect(flushes()).toBe(1);
    vi.useRealTimers();
  });

  it("la primera recarga tras un rato tranquilo llega a los 250 ms", () => {
    expect(refreshDelay(10_000, -Infinity)).toBe(REFRESH_DELAY_MS);
    expect(refreshDelay(10_000, 5_000)).toBe(REFRESH_DELAY_MS);
    expect(refreshDelay(10_000, 9_900)).toBe(900);
  });
});

describe("Eventos: guardados, Docker e historial", () => {
  beforeEach(reset);

  it("guardados: filtros por fecha, tipo, acciones y texto; borrar pide confirmación", async () => {
    const user = userEvent.setup();
    useEvents.setState({ tab: "journal" });
    renderWithQuery(<EventsView />);
    expect(await screen.findByTestId("journal-count")).toHaveTextContent("Se muestran 4 de 4 que cumplen los filtros (4 guardados)");
    const first = api.eventsJournal.mock.calls[0]![0]!;
    expect(first.since).toBeGreaterThan(Math.floor(Date.now() / 1000) - 24 * 3600 - 120);
    expect(first.until).toBeNull();
    await user.click(screen.getByRole("button", { name: "Volumen" }));
    await waitFor(() => expect(api.eventsJournal).toHaveBeenLastCalledWith(expect.objectContaining({ types: ["volume"] })));
    await user.type(screen.getByPlaceholderText("start, die"), "start, die");
    await waitFor(() => expect(api.eventsJournal).toHaveBeenLastCalledWith(expect.objectContaining({ actions: ["start", "die"] })));
    await user.click(screen.getByRole("button", { name: "Borrar eventos guardados" }));
    const d = screen.getByRole("dialog");
    expect(within(d).getByText(/Se borrarán del disco los 4 eventos guardados/)).toBeInTheDocument();
    await user.click(within(d).getByRole("button", { name: "Borrar eventos guardados" }));
    await waitFor(() => expect(api.eventsJournalClear).toHaveBeenCalledTimes(1));
  });

  it("consultar a Docker: explica el límite de 256, envía la consulta y muestra el comando", async () => {
    const user = userEvent.setup();
    useEvents.setState({ tab: "docker" });
    renderWithQuery(<EventsView />);
    expect(screen.getByText(/últimos 256 eventos/)).toBeInTheDocument();
    const resource = screen.getByPlaceholderText("elige un tipo de recurso");
    expect(resource).toBeDisabled();
    await user.selectOptions(screen.getByRole("combobox"), "container");
    await user.type(screen.getByPlaceholderText("nombre o ID"), "web");
    await user.click(screen.getByRole("button", { name: "Consultar" }));
    await waitFor(() => expect(api.eventsQuery).toHaveBeenCalledTimes(1));
    const q = api.eventsQuery.mock.calls[0]![0]!;
    expect(q).toMatchObject({ types: ["container"], resource: "web", actions: [] });
    expect(q.until).toBeGreaterThanOrEqual(q.since + 3600 - 120);
    expect(await screen.findByTestId("events-query-command")).toHaveTextContent("--filter type=container");
  });

  it("historial: sesiones anteriores, filtros, detalle sin secretos y borrado con confirmación", async () => {
    const user = userEvent.setup();
    useEvents.setState({ tab: "history" });
    renderWithQuery(<EventsView />);
    const list = await screen.findByTestId("history-list");
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("Desplegar proyecto");
    expect(rows[0]).toHaveTextContent("Error");
    expect(within(rows[0]!).queryByText("Sesión anterior")).not.toBeInTheDocument();
    expect(within(rows[2]!).getByText("Sesión anterior")).toBeInTheDocument();
    await user.click(within(rows[0]!).getByRole("button"));
    expect(screen.getByText("/usr/bin/docker compose -p tienda up --detach")).toBeInTheDocument();
    expect(screen.getByText("El proceso terminó con código 1")).toBeInTheDocument();
    expect(screen.getByText(/solo están en el panel de actividad durante la sesión/)).toBeInTheDocument();
    await user.click(within(screen.getByRole("group", { name: "Estado" })).getByRole("button", { name: "Error" }));
    await waitFor(() => expect(api.historyList).toHaveBeenLastCalledWith(expect.objectContaining({ statuses: ["failed"] })));
    expect(await screen.findByTestId("history-count")).toHaveTextContent("Se muestran 1 de 1");
    await user.click(screen.getByRole("button", { name: "Borrar historial" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Borrar historial" }));
    await waitFor(() => expect(api.historyClear).toHaveBeenCalledTimes(1));
  });

  it("si el guardado está desactivado lo dice", async () => {
    api.historyList.mockResolvedValueOnce({ entries: [], total: 0, stored: 0, persistent: false, sessionStart: "2026-11-07T10:00:00Z", location: null });
    useEvents.setState({ tab: "history" });
    renderWithQuery(<EventsView />);
    expect(await screen.findByText(/El historial está desactivado en Configuración/)).toBeInTheDocument();
    expect(screen.getByText("Todavía no hay operaciones guardadas.")).toBeInTheDocument();
  });
});
