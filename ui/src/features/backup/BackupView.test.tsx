import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import i18n from "@/shared/i18n";
import { api, backupEntry, backupJob, connected, container, containers, streams, timerStatus } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { useApp } from "@/app/store";
import { BackupView } from "./BackupView";
import { useBackups } from "./store";
import { codeText, databaseContainers, dbEngineOf, defaultTarget, retentionText, scheduleText, timerText } from "./model";

const t = i18n.t.bind(i18n) as unknown as (k: string, o?: Record<string, unknown>) => string;
const exists = (k: string) => i18n.exists(k);

function reset() {
  vi.clearAllMocks();
  streams.reset();
  useBackups.setState({ runs: {} });
  setServerState(connected);
  useApp.setState({ section: "backup" });
}

describe("modelo de las copias", () => {
  it("reconoce los motores, describe la programación y traduce los códigos", () => {
    expect(dbEngineOf("docker.io/library/mariadb:11")).toBe("mariadb");
    expect(dbEngineOf("postgis/postgis:16")).toBe("postgres");
    expect(dbEngineOf("valkey/valkey:8")).toBe("redis");
    expect(dbEngineOf("nginx:1.27")).toBeNull();
    const list = [container({ name: "pg", image: "postgres:17", state: "running" }), container({ name: "pg2", image: "postgres:17", state: "exited" })];
    expect(databaseContainers(list).map((c) => [c.name, c.engine])).toEqual([["pg", "postgres"]]);
    expect(scheduleText(t, { kind: "daily", hour: 3, minute: 5 })).toBe("Cada día a las 03:05");
    expect(scheduleText(t, { kind: "weekly", weekday: 7, hour: 22, minute: 0 })).toBe("Cada domingo a las 22:00");
    expect(scheduleText(t, { kind: "monthly", day: 1, hour: 0, minute: 30 })).toBe("El día 1 de cada mes a las 00:30");
    expect(retentionText(t, { daily: 7, weekly: 4, monthly: 3 })).toBe("conserva 7 diarias, 4 semanales y 3 mensuales");
    expect(codeText(t, exists, "restore.volumeNotEmpty")).toBe("El volumen de destino tiene datos: elige uno nuevo o vacío.");
    expect(codeText(t, exists, "restore.warn.secretsMissing:API_TOKEN")).toBe("Faltan las variables secretas (API_TOKEN): añádelas antes de arrancarlo.");
    expect(codeText(t, exists, "desconocido.algo")).toBe("desconocido.algo");
    expect(timerText(t, timerStatus()).text).toBe("Sin programar");
    expect(timerText(t, timerStatus({ installed: true, nextRun: "mañana" })).text).toBe("Próxima ejecución: mañana");
    expect(timerText(t, timerStatus({ installed: true, outdated: true })).tone).toBe("warn");
    expect(timerText(t, timerStatus({ foreign: true })).tone).toBe("warn");
    // Restaurar siempre en algo nuevo, salvo las bases de datos.
    const e = backupEntry();
    expect(defaultTarget(e.manifest!.items[0]!, null)).toBe("datos-restaurado");
    expect(defaultTarget(e.manifest!.items[1]!, null)).toBe("db");
    expect(defaultTarget({ ...e.manifest!.items[0]!, kind: "compose", name: "tienda" }, "/home/u/")).toBe("/home/u/tienda-restaurado");
  });
});

describe("apartado Copias", () => {
  beforeEach(reset);

  it("sin carpeta de destino pide elegirla con el diálogo nativo", async () => {
    api.backupSettings.mockResolvedValueOnce({ destination: null, jobs: [], systemd: true });
    renderWithQuery(<BackupView />);
    expect(await screen.findByText("Elige primero la carpeta donde se guardan las copias.")).toBeInTheDocument();
    expect(api.backupList).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Elegir carpeta" }));
    expect(api.backupPickFolder).toHaveBeenCalledWith(true);
  });

  it("lista las copias, sus avisos, y comprueba la integridad", async () => {
    api.backupList.mockResolvedValueOnce([backupEntry(), backupEntry({ id: "20261113T030000Z-ffffff", state: "incomplete", manifest: null, size: 10 })]);
    renderWithQuery(<BackupView />);
    const list = await screen.findByRole("list", { name: "Copias" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows[0]).toHaveTextContent("Completa");
    expect(rows[0]).toHaveTextContent("2 elementos");
    expect(rows[0]).toHaveTextContent("1 aviso");
    expect(rows[1]).toHaveTextContent("Incompleta");
    expect(rows[1]).toHaveTextContent("nunca se da por buena");
    // La incompleta no se puede comprobar ni restaurar.
    expect(within(rows[1]!).queryByRole("button", { name: "Restaurar" })).toBeNull();
    await userEvent.click(within(rows[0]!).getByRole("button", { name: "Comprobar" }));
    expect(await within(rows[0]!).findByText("Integridad correcta: 3 archivos con su suma SHA-256.")).toBeInTheDocument();
    // Detalle: el volumen copiado en caliente lleva su aviso.
    await userEvent.click(within(rows[0]!).getByRole("button", { expanded: false }));
    expect(within(rows[0]!).getByText("en caliente: no garantizada")).toBeInTheDocument();
    expect(within(rows[0]!).getByText(/un contenedor lo estaba usando/)).toBeInTheDocument();
    // Eliminar pide confirmación.
    await userEvent.click(within(rows[1]!).getByRole("button", { name: "Eliminar la copia 20261113T030000Z-ffffff" }));
    const dlg = await screen.findByRole("dialog", { name: "Eliminar la copia" });
    await userEvent.click(within(dlg).getByRole("button", { name: "Eliminar" }));
    await waitFor(() => expect(api.backupDelete).toHaveBeenCalledWith("20261113T030000Z-ffffff"));
  });

  it("una copia «deteniendo» avisa de qué contenedores se paran y muestra el avance", async () => {
    renderWithQuery(<BackupView />);
    await userEvent.click(await screen.findByRole("radio", { name: "Nueva copia" }));
    await userEvent.click(await screen.findByRole("checkbox", { name: "datos" }));
    expect(screen.getByText("Un contenedor lo está usando: la copia no está garantizada.")).toBeInTheDocument();
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Modo de copia de datos" }), "stop");
    await userEvent.click(screen.getByRole("button", { name: "Hacer copia ahora" }));
    const dlg = await screen.findByRole("dialog", { name: "Detener contenedores durante la copia" });
    expect(within(dlg).getByText("web")).toBeInTheDocument();
    expect(api.backupRunOpen).not.toHaveBeenCalled();
    await userEvent.click(within(dlg).getByRole("button", { name: "Detener y copiar" }));
    await waitFor(() => expect(api.backupRunOpen).toHaveBeenCalled());
    const [req, job] = api.backupRunOpen.mock.calls[0]!;
    expect(req.volumes).toEqual([{ name: "datos", mode: "stop" }]);
    expect(job).toBeNull();
    act(() => streams.emit(1, { event: "progress", progress: { kind: "volume", name: "datos", done: 0, total: 1, bytes: 4_194_304 } }));
    expect(await screen.findByText(/0 de 1: datos/)).toBeInTheDocument();
    act(() => streams.emit(1, { event: "backupEnd", entry: backupEntry(), error: null }));
    expect(await screen.findByText(/Copia 20261114T030000Z-a1b2c3 completa/)).toBeInTheDocument();
  });

  it("programar una tarea muestra los archivos de systemd y exige la autorización expresa", async () => {
    api.backupSettings.mockResolvedValue({ destination: "/home/u/Copias", jobs: [{ job: backupJob(), timer: timerStatus() }], systemd: true });
    renderWithQuery(<BackupView />);
    await userEvent.click(await screen.findByRole("radio", { name: "Tareas programadas" }));
    expect(await screen.findByText("Diaria")).toBeInTheDocument();
    expect(screen.getByText(/Cada día a las 03:00 · conserva 7 diarias/)).toBeInTheDocument();
    expect(screen.getByText("Sin programar")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Programar" }));
    const dlg = await screen.findByRole("dialog", { name: "Programar «Diaria»" });
    expect(await within(dlg).findByText("/home/u/.config/systemd/user/dcc-backup-job-0a1b2c3d.timer")).toBeInTheDocument();
    expect(within(dlg).getByText(/OnCalendar=\*-\*-\* 03:00:00/)).toBeInTheDocument();
    const create = within(dlg).getByRole("button", { name: "Crear la programación" });
    expect(create).toBeDisabled();
    expect(api.backupTimerInstall).not.toHaveBeenCalled();
    await userEvent.click(within(dlg).getByRole("checkbox", { name: "Autorizo crear estos archivos y ejecutar estas órdenes" }));
    await userEvent.click(create);
    await waitFor(() => expect(api.backupTimerInstall).toHaveBeenCalledWith("job-0a1b2c3d"));
    api.backupSettings.mockReset();
  });

  it("restaurar una base de datos exige vista previa y la palabra de confirmación", async () => {
    api.containersList.mockResolvedValue([...containers, container({ id: "dbdbdbdbdbdb", name: "db2", image: "postgres:17", state: "running" })]);
    renderWithQuery(<BackupView />);
    const list = await screen.findByRole("list", { name: "Copias" });
    await userEvent.click(within(list).getByRole("button", { name: "Restaurar" }));
    const dlg = await screen.findByRole("dialog", { name: /Restaurar la copia/ });
    // Restauración selectiva: solo la base de datos.
    await userEvent.click(within(dlg).getByRole("checkbox", { name: "Volumen · datos" }));
    await userEvent.selectOptions(within(dlg).getByRole("combobox", { name: "Contenedor donde se restaura" }), "db2");
    await userEvent.click(within(dlg).getByRole("button", { name: "Vista previa" }));
    expect(await within(dlg).findByText("Se sustituirán los datos de la base de datos.")).toBeInTheDocument();
    expect(within(dlg).getByText("Integridad comprobada: 3 archivos correctos.")).toBeInTheDocument();
    const run = within(dlg).getByRole("button", { name: "Restaurar" });
    expect(run).toBeDisabled();
    await userEvent.type(within(dlg).getByRole("textbox", { name: "Escribe «restaurar» para confirmar" }), "restaurar");
    expect(run).toBeEnabled();
    await userEvent.click(run);
    await waitFor(() => expect(api.backupRestoreOpen).toHaveBeenCalled());
    const [id, req] = api.backupRestoreOpen.mock.calls[0]!;
    expect(id).toBe("20261114T030000Z-a1b2c3");
    expect(req).toEqual({ items: [{ kind: "database", name: "db", target: "db2" }], confirmed: true, acceptedRisks: [] });
    act(() =>
      streams.emit(1, {
        event: "restoreEnd",
        outcome: { backupId: id, items: [{ kind: "database", name: "db", target: "db2", ok: true, error: null, warnings: ["restore.warn.sqlErrors:2"] }] },
        error: null,
      }),
    );
    expect(await within(dlg).findByText("2 sentencias dieron error al restaurar.")).toBeInTheDocument();
    api.containersList.mockReset();
    api.containersList.mockImplementation(async () => containers);
  });
});
