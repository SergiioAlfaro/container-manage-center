import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import i18n from "@/shared/i18n";
import type { Alert } from "@/shared/ipc/bindings/Alert";
import type { MonitorMinute } from "@/shared/ipc/bindings/MonitorMinute";
import { api, connected, localServer, MONITOR_T, monitorLive, monitorSettings, monitorStatus } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { useApp } from "@/app/store";
import { useServers } from "@/app/server";
import { LineChart } from "@/shared/ui/LineChart";
import { MonitorView } from "./MonitorView";
import { useMonitorNav } from "./store";
import { alertText, liveGaps, liveSeries, minuteGaps, minuteSeries, minuteValue, serverLive, uptime } from "./model";

const t = i18n.t.bind(i18n) as unknown as (k: string, o?: Record<string, unknown>) => string;
const pct = (v: number) => `${v.toFixed(1)} %`;

function alert(over: Partial<Alert> = {}): Alert {
  return { server: "local", kind: "cpu", state: "fired", containerId: "c1c1c1c1c1c1", container: "web", value: 97.25, at: MONITOR_T, notified: true, ...over };
}

function minute(t: number, over: Partial<MonitorMinute> = {}): MonitorMinute {
  return {
    t,
    samples: 12,
    gaps: 0,
    cpuTotalAvg: 10,
    memTotalAvg: 1024,
    containers: [{ id: "c1", name: "web", samples: 12, cpuAvg: 10, cpuMax: 20, memAvg: 512, memMax: 600, memLimit: 1024, netRx: 6000, netTx: 600, blkRead: 0, blkWrite: 60, pidsMax: 4 }],
    ...over,
  };
}

describe("modelo de la monitorización", () => {
  it("las series en vivo cortan la línea en los huecos y los marcan", () => {
    const live = monitorLive(["c1c1c1c1c1c1"]);
    const [s] = liveSeries(live, ["c1c1c1c1c1c1"], { c1c1c1c1c1c1: "web" }, "cpu");
    expect(s!.name).toBe("web");
    expect(s!.points.map((p) => p[1])).toEqual([10, null, 12]);
    expect(serverLive(live.server, "cpuTotal").map((p) => p[1])).toEqual([10, null, 12]);
    expect(liveGaps(live.server)).toEqual([[MONITOR_T - 10_000, MONITOR_T]]);
  });

  it("los minutos guardados: medias, bytes por segundo y minutos que faltan como hueco", () => {
    const recs = [minute(0), minute(60_000), minute(300_000), minute(360_000, { samples: 0, gaps: 12, cpuTotalAvg: null, containers: [] })];
    expect(minuteValue(recs[0]!.containers[0], "netRx")).toBe(100);
    expect(minuteValue(recs[0]!.containers[0], "memPercent")).toBe(50);
    expect(minuteValue(recs[0]!.containers[0], "pids")).toBe(4);
    const [s] = minuteSeries(recs, ["c1"], {}, "cpu");
    expect(s!.name).toBe("web");
    // Del minuto 1 al 5 no hay registros: se inserta un `null`.
    expect(s!.points).toEqual([[0, 10], [60_000, 10], [120_000, null], [300_000, 10], [360_000, null]]);
    expect(minuteGaps(recs)).toEqual([[120_000, 300_000], [360_000, 420_000]]);
  });

  it("tiempo en marcha y textos de las alertas", () => {
    const now = Date.parse("2026-11-14T12:00:00Z");
    expect(uptime(t, "2026-11-12T09:00:00Z", now)).toBe("2 d 3 h");
    expect(uptime(t, "2026-11-14T10:30:00Z", now)).toBe("1 h 30 min");
    expect(uptime(t, "2026-11-14T11:58:00Z", now)).toBe("2 min");
    expect(uptime(t, "2026-11-14T11:59:50Z", now)).toBe("10 s");
    expect(uptime(t, "0001-01-01T00:00:00Z", now)).toBe("—");
    expect(uptime(t, null, now)).toBe("—");
    expect(alertText(t, alert(), "Local", pct)).toEqual({ summary: "CPU alta en web", body: "web en Local: 97.3 % de CPU" });
    expect(alertText(t, alert({ state: "resolved", value: 40 }), "Local", pct).summary).toBe("CPU normal en web");
    expect(alertText(t, alert({ kind: "exitError", value: 137 }), "Local", pct).body).toBe("web en Local terminó con el código 137.");
    expect(alertText(t, alert({ kind: "disconnected", container: null, containerId: null, value: null }), "Producción", pct).summary).toBe("Sin conexión con Producción");
  });
});

describe("gráfico de líneas", () => {
  it("dibuja cada serie con su trazo, resume los últimos valores y sombrea los huecos", () => {
    const { container } = render(
      <LineChart
        label="CPU"
        series={[
          { name: "web", points: [[0, 10], [1000, null], [2000, 30]] },
          { name: "db", points: [[0, 5], [2000, 6]] },
        ]}
        from={0}
        to={2000}
        gaps={[[1000, 1500]]}
        format={pct}
        timeFormat={(x) => String(x)}
        emptyText="vacío"
      />,
    );
    expect(screen.getByRole("img", { name: "CPU: web 30.0 %, db 6.0 %" })).toBeInTheDocument();
    const paths = container.querySelectorAll("path");
    // El `null` corta la línea: dos tramos «M» en la primera serie.
    expect(paths[0]!.getAttribute("d")!.match(/M/g)).toHaveLength(2);
    expect(paths[1]!.getAttribute("stroke-dasharray")).toBe("6 3");
    // Un punto suelto entre huecos se ve (tramo de longitud mínima).
    expect(paths[0]!.getAttribute("d")).toContain("l0.01,0");
    expect(container.querySelectorAll("rect")).toHaveLength(1);
    const legend = screen.getByRole("list", { name: "CPU" });
    expect(within(legend).getByText("web")).toBeInTheDocument();
  });

  it("sin datos muestra el texto vacío", () => {
    render(<LineChart label="CPU" series={[{ name: "web", points: [[0, null]] }]} from={0} to={1} format={pct} timeFormat={String} emptyText="Sin datos" />);
    expect(screen.getByText("Sin datos")).toBeInTheDocument();
    expect(screen.queryByRole("img")).toBeNull();
  });
});

describe("apartado Monitorización", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
    useApp.setState({ section: "monitor" });
  });

  it("desactivada: se activa para este servidor", async () => {
    api.monitorStatus.mockResolvedValueOnce(monitorStatus({ enabled: false, running: false, last: null }));
    renderWithQuery(<MonitorView />);
    expect(await screen.findByText("La monitorización de este servidor está desactivada.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("checkbox", { name: "Monitorizar este servidor" }));
    expect(api.monitorSetServer).toHaveBeenCalledWith(true);
  });

  it("resumen: equipo según Docker, recuentos y tabla de contenedores", async () => {
    renderWithQuery(<MonitorView />);
    expect(await screen.findByText("Equipo (según Docker)")).toBeInTheDocument();
    expect(screen.getByText("1 en marcha · 0 en pausa · 1 parados")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("2 contenedores en 42 ms");
    const table = screen.getByRole("table", { name: "Contenedores" });
    const web = within(table).getByRole("button", { name: "web" }).closest("tr")!;
    expect(web).toHaveTextContent("12,5 %");
    expect(web).toHaveTextContent("2 reinicios");
    const db = within(table).getByRole("button", { name: "db" }).closest("tr")!;
    expect(db).toHaveTextContent("No está en marcha (exited)");
    // Al elegir un contenedor se piden sus series y se ve su detalle.
    await userEvent.click(within(table).getByRole("button", { name: "web" }));
    expect(await screen.findByRole("region", { name: "Detalle de web" })).toBeInTheDocument();
    await waitFor(() => expect(api.monitorLive).toHaveBeenCalledWith(["c1c1c1c1c1c1"], 15));
  });

  it("al llegar desde la auditoría abre el detalle de ese contenedor (por id), una sola vez", async () => {
    useMonitorNav.getState().show("local", "c1c1c1c1c1c1");
    renderWithQuery(<MonitorView />);
    expect(await screen.findByRole("region", { name: "Detalle de web" })).toBeInTheDocument();
    await waitFor(() => expect(api.monitorLive).toHaveBeenCalledWith(["c1c1c1c1c1c1"], 15));
    expect(useMonitorNav.getState().focus).toBeNull();
    // Para otro servidor no se usa.
    useMonitorNav.getState().show("srv-otro", "c1c1c1c1c1c1");
    expect(useMonitorNav.getState().take("local")).toBeNull();
    useMonitorNav.setState({ focus: null });
  });

  it("pausa, intervalo y periodo guardado en disco", async () => {
    renderWithQuery(<MonitorView />);
    await userEvent.click(await screen.findByRole("button", { name: "Pausar" }));
    expect(api.monitorPause).toHaveBeenCalledWith(true);
    await waitFor(() => expect(screen.getByRole("combobox")).toHaveValue("5"));
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "2" } });
    await waitFor(() => expect(api.monitorSettingsSet).toHaveBeenCalledWith(expect.objectContaining({ intervalSecs: 2 })));
    await userEvent.click(screen.getByRole("radio", { name: "6 h" }));
    await waitFor(() => expect(api.monitorMinutes).toHaveBeenCalledWith(6, []));
  });

  it("sin conexión lo dice y muestra el último error", async () => {
    api.monitorStatus.mockResolvedValue(monitorStatus({ gap: true, lastError: "no hay conexión con Docker Engine" }));
    renderWithQuery(<MonitorView />);
    expect(await screen.findByText("Sin conexión: se registra un hueco.")).toBeInTheDocument();
    expect(screen.getByText("Último error: no hay conexión con Docker Engine")).toBeInTheDocument();
    api.monitorStatus.mockImplementation(async () => monitorStatus());
  });

  it("comparar: como mucho cuatro contenedores", async () => {
    const many = monitorStatus();
    const base = many.last!.containers[0]!;
    many.last!.containers = Array.from({ length: 5 }, (_, i) => ({ ...base, meta: { ...base.meta, id: `id${i}`, name: `c${i}` } }));
    api.monitorStatus.mockResolvedValue(many);
    renderWithQuery(<MonitorView />);
    await userEvent.click(await screen.findByRole("radio", { name: "Comparar" }));
    expect(screen.getByText("Elige algún contenedor para compararlo.")).toBeInTheDocument();
    for (const n of ["c0", "c1", "c2", "c3"]) await userEvent.click(screen.getByRole("checkbox", { name: n }));
    expect(screen.getByRole("checkbox", { name: "c4" })).toBeDisabled();
    await waitFor(() => expect(api.monitorLive).toHaveBeenLastCalledWith(["id0", "id1", "id2", "id3"], 15));
    api.monitorStatus.mockImplementation(async () => monitorStatus());
  });

  it("alertas: reglas, prueba de notificación e historial", async () => {
    useServers.setState({ servers: [localServer()] });
    api.monitorAlerts.mockResolvedValue([alert(), alert({ state: "resolved", value: 40, notified: false, at: MONITOR_T + 1 })]);
    renderWithQuery(<MonitorView />);
    await userEvent.click(await screen.findByRole("radio", { name: "Alertas" }));
    const save = screen.getByRole("button", { name: "Guardar" });
    expect(save).toBeDisabled();
    const cpu = screen.getAllByRole("spinbutton", { name: "Salta por encima de (%)" })[0]!;
    await userEvent.clear(cpu);
    await userEvent.type(cpu, "95");
    await userEvent.click(save);
    expect(api.monitorSettingsSet).toHaveBeenCalledWith({ ...monitorSettings(), rules: { ...monitorSettings().rules, cpu: { ...monitorSettings().rules.cpu, above: 95 } } });
    await userEvent.click(screen.getByRole("button", { name: "Probar notificación" }));
    expect(api.monitorNotify).toHaveBeenCalledWith("Prueba de Docker Control Center", "Si ves este aviso, las notificaciones funcionan.");
    expect(await screen.findByText("Enviada.")).toBeInTheDocument();
    const hist = screen.getByRole("region", { name: "Historial de alertas" });
    expect(within(hist).getByText("CPU alta en web")).toBeInTheDocument();
    // El equipo local tiene el nombre vacío: se muestra traducido.
    expect(within(hist).getAllByText("web en Este equipo: 97,3 % de CPU")).not.toHaveLength(0);
    expect(within(hist).getByText("CPU normal en web")).toBeInTheDocument();
    api.monitorAlerts.mockResolvedValue([]);
  });

  it("cada muestra nueva del servidor refresca el estado", async () => {
    let push: ((e: { server: string; t: number }) => void) | undefined;
    api.onMonitorSample.mockImplementationOnce(async (h) => {
      push = h;
      return () => {};
    });
    renderWithQuery(<MonitorView />);
    await screen.findByText("Equipo (según Docker)");
    const before = api.monitorStatus.mock.calls.length;
    await act(async () => push!({ server: "otro", t: 1 }));
    expect(api.monitorStatus.mock.calls.length).toBe(before);
    await act(async () => push!({ server: "local", t: 1 }));
    await waitFor(() => expect(api.monitorStatus.mock.calls.length).toBeGreaterThan(before));
  });
});
