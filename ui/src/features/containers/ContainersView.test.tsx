import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, diagnosis, disconnected } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useEngine } from "@/features/engine/store";
import { ContainersView, allowedActions } from "./ContainersView";

const row = (name: string) => screen.getByText(name, { selector: "span" }).closest("tr")!;

describe("allowedActions", () => {
  it("solo ofrece acciones válidas para cada estado", () => {
    expect(allowedActions("running")).toEqual(["stop", "restart", "pause"]);
    expect(allowedActions("paused")).toEqual(["unpause", "stop"]);
    expect(allowedActions("exited")).toEqual(["start"]);
    expect(allowedActions("removing")).toEqual([]);
  });
});

describe("Contenedores", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEngine.setState({ state: connected });
  });

  it("lista los contenedores que devuelve Docker con estado, puertos y proyecto", async () => {
    renderWithQuery(<ContainersView />);
    expect(await screen.findByText("web", { selector: "span" })).toBeInTheDocument();
    expect(within(row("web")).getByText("En ejecución")).toBeInTheDocument();
    // IPv4 e IPv6 del mismo puerto se muestran una vez.
    expect(within(row("web")).getAllByText("8080→80/tcp")).toHaveLength(1);
    expect(within(row("cache")).getByText("compose: blog")).toBeInTheDocument();
    expect(screen.getByText("3 contenedores")).toBeInTheDocument();
  });

  it("filtra por estado y por texto", async () => {
    renderWithQuery(<ContainersView />);
    await screen.findByText("web", { selector: "span" });
    await userEvent.click(screen.getByRole("radio", { name: "Detenidos" }));
    expect(screen.queryByText("web", { selector: "span" })).not.toBeInTheDocument();
    expect(screen.getByText("db", { selector: "span" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "Todos" }));
    await userEvent.type(screen.getByRole("searchbox"), "redis");
    expect(screen.getByText("1 contenedor")).toBeInTheDocument();
  });

  it("una acción llama al backend con el ID real y después recarga la lista", async () => {
    renderWithQuery(<ContainersView />);
    await screen.findByText("db", { selector: "span" });
    expect(api.containersList).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole("button", { name: "Iniciar db" }));
    expect(api.containerAction).toHaveBeenCalledWith("b2b2b2b2b2b2", "db", "start");
    await waitFor(() => expect(api.containersList).toHaveBeenCalledTimes(2));
    // Un contenedor detenido no ofrece detener ni pausar.
    expect(screen.queryByRole("button", { name: "Detener db" })).not.toBeInTheDocument();
  });

  it("la interfaz sigue respondiendo mientras una operación está en curso", async () => {
    let release!: () => void;
    api.containerAction.mockImplementationOnce(() => new Promise<void>((r) => (release = r)));
    renderWithQuery(<ContainersView />);
    await screen.findByText("web", { selector: "span" });
    await userEvent.click(screen.getByRole("button", { name: "Detener web" }));
    // La fila ocupada muestra la operación y no admite otra acción…
    expect(within(row("web")).getByText("Detener")).toBeInTheDocument();
    expect(within(row("web")).queryByRole("button", { name: "Reiniciar web" })).not.toBeInTheDocument();
    // …pero el resto de la interfaz funciona: buscar y actuar sobre otro contenedor.
    await userEvent.click(screen.getByRole("button", { name: "Iniciar db" }));
    expect(api.containerAction).toHaveBeenLastCalledWith("b2b2b2b2b2b2", "db", "start");
    await userEvent.type(screen.getByRole("searchbox"), "web");
    expect(screen.getByText("1 contenedor")).toBeInTheDocument();
    await act(async () => release());
  });

  it("muestra el error real de Docker si la acción falla", async () => {
    api.containerAction.mockRejectedValueOnce({ code: "engine.docker", message: "Container is not running", httpStatus: 409 });
    renderWithQuery(<ContainersView />);
    await screen.findByText("web", { selector: "span" });
    await userEvent.click(screen.getByRole("button", { name: "Pausar web" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Pausar web: Container is not running");
  });

  it("eliminar en ejecución exige forzar; cancelar no hace nada", async () => {
    renderWithQuery(<ContainersView />);
    await screen.findByText("web", { selector: "span" });
    await userEvent.click(screen.getByRole("button", { name: "Eliminar web" }));
    const dialog = screen.getByRole("dialog", { name: "Eliminar web" });
    const confirm = within(dialog).getByRole("button", { name: "Eliminar contenedor" });
    expect(confirm).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.containerRemove).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Eliminar web" }));
    const d2 = screen.getByRole("dialog");
    await userEvent.click(within(d2).getByLabelText("Detenerlo a la fuerza y eliminarlo"));
    await userEvent.click(within(d2).getByRole("button", { name: "Eliminar contenedor" }));
    expect(api.containerRemove).toHaveBeenCalledWith("a1b2c3d4e5f6a7b8c9d0", "web", true, false);
  });

  it("eliminar uno detenido no pide forzar y permite borrar volúmenes anónimos", async () => {
    renderWithQuery(<ContainersView />);
    await screen.findByText("db", { selector: "span" });
    await userEvent.click(screen.getByRole("button", { name: "Eliminar db" }));
    const d = screen.getByRole("dialog");
    expect(within(d).queryByLabelText("Detenerlo a la fuerza y eliminarlo")).not.toBeInTheDocument();
    await userEvent.click(within(d).getByLabelText(/Eliminar también sus volúmenes anónimos/));
    await userEvent.click(within(d).getByRole("button", { name: "Eliminar contenedor" }));
    expect(api.containerRemove).toHaveBeenCalledWith("b2b2b2b2b2b2", "db", false, true);
  });

  it("sin conexión no consulta Docker y remite al diagnóstico", () => {
    useEngine.setState({ state: disconnected(diagnosis({ kind: "noSocket" })) });
    renderWithQuery(<ContainersView />);
    expect(screen.getByText("Este apartado necesita conexión con Docker")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ver diagnóstico" })).toBeInTheDocument();
    expect(api.containersList).not.toHaveBeenCalled();
  });

  it("lista vacía: estado vacío sin acciones", async () => {
    api.containersList.mockResolvedValueOnce([]);
    renderWithQuery(<ContainersView />);
    expect(await screen.findByText("No hay contenedores")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});
