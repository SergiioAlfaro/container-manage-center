import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, container, containers, streams } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { FakeTerminal } from "@/test/xtermMock";
import type { TerminalEvent } from "@/shared/ipc/bindings/TerminalEvent";
import { layoutFor, pairs } from "./layout";
import { defaultName, MAX_SESSIONS, nameProblem, openSession, resetTerminals, useTerminals } from "./sessions";
import { TerminalsView } from "./TerminalsView";

const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));
const WEB = "a1b2c3d4e5f6a7b8c9d0";
const API = "d4d4d4d4d4d4";
const withApi = [...containers, container({ id: API, name: "api", image: "node:22", state: "running", status: "Up 1 minute" })];

function open(name: string, id = WEB, containerName = "web") {
  let key: number | null = null;
  act(() => {
    key = openSession({ server: "local", serverName: "", remote: false, containerId: id, containerName, shell: "sh", name });
  });
  return key!;
}
const panes = () => screen.queryAllByTestId("terminal-pane");
const pane = (name: string) => screen.getByRole("region", { name: new RegExp(`^Terminal ${name} en`) });
const end = (container: "stopped" | "removed" | null, summary = "El proceso terminó con código 0"): TerminalEvent => ({
  event: "end",
  closed: { end: { reason: "finished", summary, exitCode: 0, container }, error: null },
});

describe("Terminales: distribución y nombres", () => {
  it("elige la distribución según el número de paneles", () => {
    expect([0, 1, 2, 3, 4, 5, 9].map(layoutFor)).toEqual(["empty", "single", "columns", "three", "grid", "scroll", "scroll"]);
    expect(pairs([1, 2, 3, 4, 5])).toEqual([[1, 2], [3, 4], [5]]);
  });

  it("valida el nombre de la sesión y numera los nombres por omisión", () => {
    expect(nameProblem("  ")).toBe("empty");
    expect(nameProblem("x".repeat(61))).toBe("long");
    expect(nameProblem("a\u001bb")).toBe("control");
    expect(nameProblem(" depuración ")).toBeNull();
    const s = (name: string) => ({ key: 0, server: "local", serverName: "", remote: false, containerId: "", containerName: "web", shell: "sh" as const, name, status: "open" as const, summary: null, error: null, after: null });
    expect(defaultName("web", [])).toBe("web");
    expect(defaultName("web", [s("web"), s("web (2)")])).toBe("web (3)");
  });
});

describe("Terminales: sesiones reales e independientes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    streams.reset();
    FakeTerminal.all = [];
    resetTerminals();
    setServerState(connected);
  });
  afterEach(() => resetTerminals());

  it("sin sesiones explica el apartado; el diálogo abre una terminal en el contenedor elegido", async () => {
    api.containersList.mockResolvedValue(withApi);
    renderWithQuery(<TerminalsView />);
    expect(screen.getByText("No hay terminales abiertas")).toBeInTheDocument();
    await userEvent.click(screen.getAllByRole("button", { name: "Nueva terminal" })[0]!);
    const dialog = await screen.findByRole("dialog", { name: "Nueva terminal" });
    // Solo los contenedores en ejecución.
    const select = await within(dialog).findByLabelText("Contenedor en ejecución");
    expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["api — node:22", "web — nginx:1.27"]);
    await userEvent.selectOptions(select, API);
    await userEvent.click(within(dialog).getByRole("radio", { name: "bash" }));
    const name = within(dialog).getByLabelText("Nombre de la sesión");
    expect(name).toHaveAttribute("placeholder", "api");
    await userEvent.type(name, "x".repeat(61));
    expect(within(dialog).getByText("Usa como mucho 60 caracteres.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Abrir terminal" })).toBeDisabled();
    await userEvent.clear(name);
    await userEvent.type(name, "migraciones");
    await userEvent.click(within(dialog).getByRole("button", { name: "Abrir terminal" }));

    expect(api.terminalOpen).toHaveBeenCalledWith(API, "api", "bash", 80, 24, expect.any(Function));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    const p = pane("migraciones");
    await waitFor(() => expect(within(p).getByTestId("session-state")).toHaveTextContent("Conectada"));
    expect(within(p).getByText("api")).toBeInTheDocument();
  });

  it("sin contenedores en ejecución no se puede abrir ninguna", async () => {
    api.containersList.mockResolvedValue(containers.filter((c) => c.state !== "running"));
    renderWithQuery(<TerminalsView />);
    await userEvent.click(screen.getAllByRole("button", { name: "Nueva terminal" })[0]!);
    expect(await screen.findByText(/No hay contenedores en ejecución/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Abrir terminal" })).toBeDisabled();
  });

  it("varias sesiones a la vez: cada una con su salida, su entrada y su tamaño", async () => {
    renderWithQuery(<TerminalsView />);
    open("uno");
    open("dos", API, "api");
    open("tres");
    await waitFor(() => expect(screen.getAllByText("Conectada")).toHaveLength(3));
    const [t1, t2, t3] = FakeTerminal.all;
    expect(t1!.opened && t2!.opened && t3!.opened).toBe(true);

    act(() => {
      streams.emit(1, { event: "output", data: b64("uno$ ") });
      streams.emit(2, { event: "output", data: b64("dos$ ") });
    });
    expect(t1!.text()).toBe("uno$ ");
    expect(t2!.text()).toBe("dos$ ");
    expect(t3!.text()).toBe("");

    act(() => t2!.type("ls\r"));
    expect(api.terminalInput).toHaveBeenCalledWith(2, "ls\r");
    expect(api.terminalInput).toHaveBeenCalledTimes(1);
    act(() => t3!.doResize(100, 30));
    expect(api.terminalResize).toHaveBeenLastCalledWith(3, 100, 30);
    expect(screen.getByText("3 sesiones · 3 conectadas")).toBeInTheDocument();
  });

  it("distribución: 1 ocupa todo, 2 columnas, 3 adaptable, 4 en 2×2 y más de 4 con desplazamiento", async () => {
    renderWithQuery(<TerminalsView />);
    const seps = () => screen.queryAllByRole("separator");
    open("a");
    expect(seps()).toHaveLength(0);
    open("b");
    expect(seps().map((s) => s.getAttribute("aria-orientation"))).toEqual(["vertical"]);
    open("c");
    expect(seps().map((s) => s.getAttribute("aria-orientation"))).toEqual(["vertical", "horizontal"]);
    open("d");
    expect(seps().map((s) => s.getAttribute("aria-orientation")).sort()).toEqual(["horizontal", "vertical", "vertical"]);
    expect(screen.queryByTestId("terminals-scroll")).not.toBeInTheDocument();
    open("e");
    expect(screen.getByTestId("terminals-scroll")).toBeInTheDocument();
    expect(panes()).toHaveLength(5);
    // Filas de dos con altura propia (la última, con una sola sesión, a todo el ancho).
    const rows = screen.getAllByTestId("terminals-row");
    expect(rows).toHaveLength(3);
    for (const r of rows) expect(r.style.height).toContain("calc(");
    expect(within(rows[2]!).getAllByTestId("terminal-pane")).toHaveLength(1);
    // Recolocar paneles no cierra ni recrea las sesiones.
    expect(FakeTerminal.all).toHaveLength(5);
    expect(api.terminalOpen).toHaveBeenCalledTimes(5);
    expect(api.streamClose).not.toHaveBeenCalled();
  });

  it("los separadores se mueven con el teclado y con el ratón, dentro de sus límites", async () => {
    renderWithQuery(<TerminalsView />);
    open("a");
    open("b");
    const sep = screen.getByRole("separator", { name: "Redimensionar columnas" });
    expect(sep).toHaveAttribute("aria-valuenow", "50");
    sep.focus();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    expect(sep).toHaveAttribute("aria-valuenow", "60");
    await userEvent.keyboard("{End}{ArrowRight}");
    expect(sep).toHaveAttribute("aria-valuenow", "80");
    await userEvent.keyboard("{Home}{ArrowLeft}");
    expect(sep).toHaveAttribute("aria-valuenow", "20");
    // Arrastre: posición relativa al contenedor de los dos paneles.
    vi.spyOn(sep.parentElement!, "getBoundingClientRect").mockReturnValue({ left: 100, top: 0, width: 1000, height: 500, right: 1100, bottom: 500, x: 100, y: 0, toJSON: () => ({}) });
    fireEvent.pointerDown(sep, { button: 0, pointerId: 1, clientX: 300 });
    fireEvent.pointerMove(sep, { pointerId: 1, clientX: 400 });
    expect(sep).toHaveAttribute("aria-valuenow", "30");
    fireEvent.pointerUp(sep, { pointerId: 1 });
    fireEvent.pointerMove(sep, { pointerId: 1, clientX: 900 });
    expect(sep).toHaveAttribute("aria-valuenow", "30");
    fireEvent.doubleClick(sep);
    expect(sep).toHaveAttribute("aria-valuenow", "50");
  });

  it("cerrar una sesión cierra solo su flujo; las demás siguen", async () => {
    renderWithQuery(<TerminalsView />);
    open("uno");
    open("dos");
    await waitFor(() => expect(screen.getAllByText("Conectada")).toHaveLength(2));
    await userEvent.click(within(pane("uno")).getByRole("button", { name: "Cerrar sesión" }));
    expect(api.streamClose).toHaveBeenCalledWith(1);
    expect(api.streamClose).toHaveBeenCalledTimes(1);
    expect(FakeTerminal.all[0]!.disposed).toBe(true);
    expect(FakeTerminal.all[1]!.disposed).toBe(false);
    expect(panes()).toHaveLength(1);
    act(() => FakeTerminal.all[1]!.type("pwd\r"));
    expect(api.terminalInput).toHaveBeenCalledWith(2, "pwd\r");
  });

  it("cerrar mientras se abre no deja la sesión huérfana", async () => {
    let resolve!: (id: number) => void;
    api.terminalOpen.mockImplementationOnce(() => new Promise<number>((r) => (resolve = r)));
    renderWithQuery(<TerminalsView />);
    open("lenta");
    expect(within(pane("lenta")).getByTestId("session-state")).toHaveTextContent("Conectando…");
    await userEvent.click(within(pane("lenta")).getByRole("button", { name: "Cerrar sesión" }));
    expect(api.streamClose).not.toHaveBeenCalled();
    await act(async () => resolve(7));
    expect(api.streamClose).toHaveBeenCalledWith(7);
  });

  it("si el contenedor se detiene o se elimina lo indica; solo se puede reabrir si sigue existiendo", async () => {
    renderWithQuery(<TerminalsView />);
    open("parada");
    open("borrada");
    await waitFor(() => expect(screen.getAllByText("Conectada")).toHaveLength(2));
    act(() => {
      streams.emit(1, end("stopped", "El contenedor se detuvo: el proceso terminó con código 137"));
      streams.emit(2, end("removed", "El contenedor se eliminó: el proceso terminó con código 137"));
    });
    const p1 = pane("parada");
    const p2 = pane("borrada");
    expect(within(p1).getByText("Contenedor detenido")).toBeInTheDocument();
    expect(within(p1).getByRole("status")).toHaveTextContent("El contenedor se detuvo");
    expect(within(p1).getByTestId("session-state")).toHaveTextContent("Finalizada");
    expect(within(p2).getByText("Contenedor eliminado")).toBeInTheDocument();
    expect(within(p2).getByRole("button", { name: /No se puede volver a abrir/ })).toBeDisabled();
    expect(FakeTerminal.all[0]!.text()).toContain("El contenedor se detuvo");

    await userEvent.click(within(p1).getByRole("button", { name: "Volver a abrir la sesión" }));
    expect(api.terminalOpen).toHaveBeenCalledTimes(3);
    await waitFor(() => expect(within(pane("parada")).getByTestId("session-state")).toHaveTextContent("Conectada"));
    // La sesión reabierta usa su flujo nuevo.
    act(() => FakeTerminal.all[0]!.type("x"));
    expect(api.terminalInput).toHaveBeenCalledWith(3, "x");
  });

  it("un error al abrir se muestra en el panel", async () => {
    api.terminalOpen.mockRejectedValueOnce({ code: "docker.conflict", message: "El contenedor no está en ejecución" });
    renderWithQuery(<TerminalsView />);
    open("fallida");
    const p = pane("fallida");
    await waitFor(() => expect(within(p).getByTestId("session-state")).toHaveTextContent("Error"));
    expect(within(p).getByRole("status")).toHaveTextContent("El contenedor no está en ejecución");
  });

  it("renombrar: Intro guarda, Escape cancela y un nombre vacío no se acepta", async () => {
    renderWithQuery(<TerminalsView />);
    open("web");
    await userEvent.click(within(pane("web")).getByRole("button", { name: "Renombrar sesión" }));
    const input = screen.getByLabelText("Nombre de la sesión");
    await userEvent.clear(input);
    await userEvent.type(input, "  logs nginx {Enter}");
    expect(pane("logs nginx")).toBeInTheDocument();
    await userEvent.click(within(pane("logs nginx")).getByRole("button", { name: "Renombrar sesión" }));
    await userEvent.clear(screen.getByLabelText("Nombre de la sesión"));
    await userEvent.type(screen.getByLabelText("Nombre de la sesión"), "{Enter}");
    expect(screen.getByLabelText("Nombre de la sesión")).toHaveAttribute("aria-invalid", "true");
    await userEvent.type(screen.getByLabelText("Nombre de la sesión"), "otro{Escape}");
    expect(pane("logs nginx")).toBeInTheDocument();
    expect(useTerminals.getState().sessions[0]!.name).toBe("logs nginx");
  });

  it("maximizar muestra solo esa sesión sin cerrar las demás", async () => {
    renderWithQuery(<TerminalsView />);
    open("a");
    open("b");
    open("c");
    await userEvent.click(within(pane("b")).getByRole("button", { name: "Maximizar" }));
    expect(panes()).toHaveLength(1);
    expect(screen.queryAllByRole("separator")).toHaveLength(0);
    expect(within(pane("b")).getByRole("button", { name: "Restaurar la cuadrícula" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(within(pane("b")).getByRole("button", { name: "Restaurar la cuadrícula" }));
    expect(panes()).toHaveLength(3);
    expect(api.streamClose).not.toHaveBeenCalled();
  });

  it("cerrar todas pide confirmación", async () => {
    renderWithQuery(<TerminalsView />);
    open("a");
    open("b");
    await waitFor(() => expect(screen.getAllByText("Conectada")).toHaveLength(2));
    await userEvent.click(screen.getByRole("button", { name: "Cerrar todas" }));
    const dialog = screen.getByRole("dialog", { name: "¿Cerrar todas las terminales?" });
    expect(dialog).toHaveTextContent("Se cerrarán 2 sesiones");
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(api.streamClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Cerrar todas" }));
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cerrar todas" }));
    expect(api.streamClose.mock.calls.map((c) => c[0]).sort()).toEqual([1, 2]);
    expect(screen.getByText("No hay terminales abiertas")).toBeInTheDocument();
  });

  it("cambiar de apartado no cierra las sesiones ni pierde su contenido", async () => {
    const first = renderWithQuery(<TerminalsView />);
    open("persistente");
    await waitFor(() => expect(screen.getByText("Conectada")).toBeInTheDocument());
    act(() => streams.emit(1, { event: "output", data: b64("hola") }));
    first.unmount();
    // Sigue recibiendo mientras no se ve.
    act(() => streams.emit(1, { event: "output", data: b64(" mundo") }));
    expect(api.streamClose).not.toHaveBeenCalled();
    renderWithQuery(<TerminalsView />);
    expect(pane("persistente")).toBeInTheDocument();
    expect(FakeTerminal.all).toHaveLength(1);
    expect(FakeTerminal.all[0]!.disposed).toBe(false);
    expect(FakeTerminal.all[0]!.text()).toBe("hola mundo");
  });

  it("hay un máximo de sesiones simultáneas", async () => {
    renderWithQuery(<TerminalsView />);
    for (let i = 0; i < MAX_SESSIONS; i++) open(`s${i}`);
    expect(openSession({ server: "local", serverName: "", remote: false, containerId: WEB, containerName: "web", shell: "sh" })).toBeNull();
    expect(screen.getAllByRole("button", { name: "Nueva terminal" })[0]).toBeDisabled();
    expect(api.terminalOpen).toHaveBeenCalledTimes(MAX_SESSIONS);
  });

  it("los botones de solo icono tienen nombre y descripción", async () => {
    renderWithQuery(<TerminalsView />);
    open("a");
    open("b");
    for (const b of within(pane("a")).getAllByRole("button")) {
      expect(b).toHaveAccessibleName();
      expect(b).toHaveAttribute("title");
    }
  });
});
