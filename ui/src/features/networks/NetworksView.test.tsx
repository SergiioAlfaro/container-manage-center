import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import i18n from "@/shared/i18n";
import { api, connected, network, networks } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { useEngine } from "@/features/engine/store";
import { NetworksView } from "./NetworksView";
import { useNetworksNav } from "./store";

function reset() {
  vi.clearAllMocks();
  useEngine.setState({ state: connected });
  useNetworksNav.setState({ page: { kind: "list" } });
}

const row = (name: string) => screen.getByRole("button", { name }).closest("tr")!;

describe("Lista de redes", () => {
  beforeEach(reset);

  it("lista las redes reales con driver, subred, distintivos y contenedores", async () => {
    renderWithQuery(<NetworksView />);
    expect(await screen.findByRole("button", { name: "backend" })).toBeInTheDocument();
    const backend = row("backend");
    expect(within(backend).getByText("172.30.0.0/24")).toBeInTheDocument();
    expect(within(backend).getByText("2 contenedores (1 en marcha)")).toBeInTheDocument();
    expect(within(backend).getByText("web, db")).toBeInTheDocument();
    expect(within(row("bridge")).getByText("Predefinida")).toBeInTheDocument();
    expect(within(row("aislada")).getByText("Interna")).toBeInTheDocument();
    expect(within(row("aislada")).getByText("Compose: blog")).toBeInTheDocument();
    expect(screen.getByText("6 redes")).toBeInTheDocument();
    // Las predefinidas no se pueden eliminar; host y none tampoco admiten conexiones.
    expect(within(row("bridge")).getByRole("button", { name: "Eliminar bridge" })).toBeDisabled();
    expect(within(row("host")).getByRole("button", { name: "Conectar un contenedor a host" })).toBeDisabled();
    expect(within(row("bridge")).getByRole("button", { name: "Conectar un contenedor a bridge" })).toBeEnabled();
  });

  it("filtra y busca", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await screen.findByRole("button", { name: "backend" });
    await user.click(screen.getByRole("radio", { name: "Creadas por el usuario" }));
    expect(screen.queryByRole("button", { name: "bridge" })).not.toBeInTheDocument();
    expect(screen.getByText("3 redes")).toBeInTheDocument();
    await user.type(screen.getByPlaceholderText("Buscar por nombre, ID, subred o contenedor"), "172.31");
    expect(screen.getByText("1 red")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "vieja" })).toBeInTheDocument();
  });

  it("eliminar con un contenedor en marcha: avisa de que Docker lo rechazará y no deja continuar", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Eliminar backend" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/Hay 1 contenedor en marcha conectado/)).toBeInTheDocument();
    expect(within(dialog).getByText(/1 contenedor detenido tiene configurada esta red/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Eliminar red" })).toBeDisabled();
  });

  it("eliminar una red que solo usan contenedores detenidos exige confirmar que no podrán arrancar", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Eliminar vieja" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("docker network rm vieja")).toBeInTheDocument();
    const go = within(dialog).getByRole("button", { name: "Eliminar red" });
    expect(go).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox", { name: /Entiendo que esos contenedores no podrán arrancar/ }));
    await user.click(go);
    await waitFor(() => expect(api.networkRemove).toHaveBeenCalledWith("v1v1v1v1v1v1v1v1", "vieja"));
    expect(await screen.findByText("Red vieja eliminada.")).toBeInTheDocument();
  });

  it("los errores de Docker se muestran en el diálogo", async () => {
    const user = userEvent.setup();
    api.networkRemove.mockRejectedValueOnce({ code: "engine.docker", message: "Docker respondió 403: error while removing network", httpStatus: 403, issues: null });
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Eliminar aislada" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Eliminar red" }));
    expect(await screen.findByText(/Docker rechazó la operación: Docker respondió 403/)).toBeInTheDocument();
  });

  it("limpiar: estima las redes afectadas y exige confirmar si las usan contenedores detenidos", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Limpiar" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Según la lista actual se eliminarán 2 redes:")).toBeInTheDocument();
    expect(within(dialog).getByText(/la usan contenedores detenidos: db/)).toBeInTheDocument();
    const go = within(dialog).getByRole("button", { name: "Limpiar redes" });
    expect(go).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox"));
    await user.click(go);
    await waitFor(() => expect(api.networksPrune).toHaveBeenCalled());
    expect(await screen.findByText("2 redes eliminadas: vieja, aislada")).toBeInTheDocument();
  });

  it("los diálogos consultan a Docker al abrirse: la estimación no usa una lista desactualizada", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await screen.findByRole("button", { name: "backend" });
    expect(api.networksList).toHaveBeenCalledTimes(1);
    // Mientras tanto, desde la terminal se crea una red y "vieja" pasa a tener un contenedor en marcha.
    const now = [
      ...networks.filter((n) => n.name !== "vieja"),
      network({ id: "v1v1v1v1v1v1v1v1", name: "vieja", containers: [{ id: "z", name: "nuevo", state: "running", ipv4: null, ipv6: null, mac: null }] }),
      network({ id: "t1t1t1t1t1t1t1t1", name: "creada-fuera", containers: [], labels: {} }),
    ];
    let release: () => void = () => {};
    api.networksList.mockImplementationOnce(() => new Promise((r) => (release = () => r(now))));
    await user.click(screen.getByRole("button", { name: "Limpiar" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Consultando a Docker el estado actual…")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Limpiar redes" })).toBeDisabled();
    release();
    expect(await within(dialog).findByText("Según la lista actual se eliminarán 2 redes:")).toBeInTheDocument();
    expect(within(dialog).getByText("creada-fuera")).toBeInTheDocument();
    expect(within(dialog).queryByText(/vieja/)).not.toBeInTheDocument();
    // Sin contenedores detenidos afectados no hace falta la confirmación adicional.
    expect(within(dialog).getByRole("button", { name: "Limpiar redes" })).toBeEnabled();
  });

  it("eliminar usa el estado actual: si la red ya no existe lo dice y no deja continuar", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Eliminar aislada" }));
    // (primera apertura con datos actuales: se puede eliminar)
    expect(await within(screen.getByRole("dialog")).findByRole("button", { name: "Eliminar red" })).toBeEnabled();
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancelar" }));
    api.networksList.mockResolvedValueOnce(networks.filter((n) => n.name !== "aislada"));
    await user.click(screen.getByRole("button", { name: "Eliminar aislada" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText("La red aislada ya no existe.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Eliminar red" })).toBeDisabled();
  });

  it("conectar: ofrece solo los contenedores posibles, con alias e IP, y muestra el comando", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Conectar un contenedor a backend" }));
    const dialog = screen.getByRole("dialog");
    const select = await within(dialog).findByRole("combobox", { name: "Contenedor" });
    // web y db ya están conectados; cache es el único disponible.
    expect(within(select).getByRole("option", { name: /web.*ya conectado/ })).toBeDisabled();
    expect((select as HTMLSelectElement).value).toBe("c3c3c3c3c3c3");
    await user.type(within(dialog).getByLabelText("Alias en esta red"), "cache redis");
    await user.type(within(dialog).getByLabelText("IPv4 fija"), "172.30.0.50");
    expect(within(dialog).getByText("docker network connect --alias cache --alias redis --ip 172.30.0.50 backend cache")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Conectar" }));
    await waitFor(() =>
      expect(api.networkConnect).toHaveBeenCalledWith("n1n1n1n1n1n1n1n1", "backend", "c3c3c3c3c3c3", "cache", { aliases: ["cache", "redis"], ipv4: "172.30.0.50", ipv6: "" }, ["172.30.0.0/24"]),
    );
    expect(await screen.findByText("cache conectado a backend.")).toBeInTheDocument();
  });

  it("en la red bridge no se ofrecen alias (Docker no los admite)", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Conectar un contenedor a bridge" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("La red predefinida bridge no admite alias.")).toBeInTheDocument();
    expect(within(dialog).queryByLabelText("Alias en esta red")).not.toBeInTheDocument();
  });
});

describe("Detalle de red", () => {
  beforeEach(reset);

  it("muestra resumen, direcciones, opciones y contenedores con su IP real", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "backend" }));
    await waitFor(() => expect(api.networkDetails).toHaveBeenCalledWith("n1n1n1n1n1n1n1n1", "backend"));
    expect(await screen.findByText("172.30.0.1")).toBeInTheDocument();
    expect(screen.getByText("com.docker.network.driver.mtu")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /Contenedores/ }));
    const web = screen.getByRole("button", { name: "web" }).closest("tr")!;
    expect(within(web).getByText("172.30.0.2/24")).toBeInTheDocument();
    const db = screen.getByRole("button", { name: "db" }).closest("tr")!;
    expect(within(db).getByText("Se asigna al arrancar")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Inspección JSON" }));
    expect(screen.getByTestId("network-json").textContent).toContain('"Id": "n1n1n1n1n1n1n1n1"');
  });

  it("desconectar pide confirmación y avisa si el contenedor está en marcha", async () => {
    const user = userEvent.setup();
    useNetworksNav.setState({ page: { kind: "detail", id: "n1n1n1n1n1n1n1n1", name: "backend", tab: "containers" } });
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Desconectar web" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/pierde al instante la conexión/)).toBeInTheDocument();
    expect(within(dialog).getByText("docker network disconnect backend web")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Desconectar" }));
    await waitFor(() => expect(api.networkDisconnect).toHaveBeenCalledWith("n1n1n1n1n1n1n1n1", "backend", "a1b2c3d4e5f6a7b8c9d0", "web", false));
    expect(await screen.findByText("web desconectado de backend.")).toBeInTheDocument();
  });

  it("un contenedor de la red abre su detalle en Contenedores", async () => {
    const user = userEvent.setup();
    useNetworksNav.setState({ page: { kind: "detail", id: "n1n1n1n1n1n1n1n1", name: "backend", tab: "containers" } });
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "db" }));
    expect(useApp.getState().section).toBe("containers");
    expect(useContainersNav.getState().page).toMatchObject({ kind: "detail", id: "b2b2b2b2b2b2" });
  });

  it("si la red ya no existe lo indica", async () => {
    api.networkDetails.mockRejectedValueOnce({ code: "engine.docker", message: "network x not found", httpStatus: 404, issues: null });
    useNetworksNav.setState({ page: { kind: "detail", id: "x", name: "x", tab: "summary" } });
    renderWithQuery(<NetworksView />);
    expect(await screen.findByText("La red x ya no existe.")).toBeInTheDocument();
  });
});

describe("Crear red", () => {
  beforeEach(reset);

  it("valida en el backend, muestra el comando y crea la red", async () => {
    const user = userEvent.setup();
    renderWithQuery(<NetworksView />);
    await user.click(await screen.findByRole("button", { name: "Crear red" }));
    const submit = screen.getByRole("button", { name: "Crear red" });
    expect(await screen.findByText("El nombre es obligatorio.", { selector: "li" })).toBeInTheDocument();
    expect(submit).toBeDisabled();
    await user.type(screen.getByLabelText("Nombre"), "tienda");
    await user.type(screen.getByLabelText("Subred", { selector: "input" }), "10.0.0.5/24");
    expect(await screen.findAllByText("10.0.0.5/24 no es una dirección de red; debería ser 10.0.0.0/24.")).not.toHaveLength(0);
    expect(submit).toBeDisabled();
    await user.clear(screen.getByLabelText("Subred", { selector: "input" }));
    await user.type(screen.getByLabelText("Subred", { selector: "input" }), "10.0.0.0/24");
    await user.click(screen.getByRole("checkbox", { name: /Red interna/ }));
    await waitFor(() => expect(screen.getByTestId("network-command").textContent).toBe("docker network create --driver bridge --subnet 10.0.0.0/24 --internal tienda"));
    expect(screen.getByText("Lista para crear.")).toBeInTheDocument();
    await user.click(submit);
    await waitFor(() => expect(api.networkCreate).toHaveBeenCalled());
    expect(api.networkCreate.mock.calls[0]![0]).toMatchObject({ name: "tienda", driver: "bridge", internal: true, ipv4: { subnet: "10.0.0.0/24" } });
    // Tras crearla se abre su detalle.
    await waitFor(() => expect(useNetworksNav.getState().page).toMatchObject({ kind: "detail", id: "f00df00df00df00d", name: "tienda" }));
  });

  it("overlay muestra el aviso de Swarm sin impedir crearla, y los errores de Docker se muestran", async () => {
    const user = userEvent.setup();
    api.networkCreate.mockRejectedValueOnce({ code: "engine.docker", message: "Docker respondió 403: This node is not a swarm manager.", httpStatus: 403, issues: null });
    useNetworksNav.setState({ page: { kind: "create" } });
    renderWithQuery(<NetworksView />);
    await user.type(screen.getByLabelText("Nombre"), "multi");
    await user.click(screen.getByRole("radio", { name: "overlay" }));
    expect(await screen.findAllByText(/Las redes overlay requieren Docker Swarm/)).not.toHaveLength(0);
    const submit = screen.getByRole("button", { name: "Crear red" });
    await waitFor(() => expect(submit).toBeEnabled());
    await user.click(submit);
    expect(await screen.findByText(/This node is not a swarm manager/)).toBeInTheDocument();
  });

  it("en inglés todo el apartado aparece en inglés", async () => {
    await i18n.changeLanguage("en");
    try {
      renderWithQuery(<NetworksView />);
      expect(await screen.findByRole("button", { name: "backend" })).toBeInTheDocument();
      expect(screen.getByText("2 containers (1 running)")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Create network" })).toBeInTheDocument();
      expect(screen.getAllByText("Predefined").length).toBe(3);
    } finally {
      await i18n.changeLanguage("es");
    }
  });
});
