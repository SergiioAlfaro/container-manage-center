import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import i18n from "@/shared/i18n";
import { api, connected, volume, volumes } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { useEngine } from "@/features/engine/store";
import { VolumesView } from "./VolumesView";
import { useVolumesNav } from "./store";

function reset() {
  vi.clearAllMocks();
  useEngine.setState({ state: connected });
  useVolumesNav.setState({ page: { kind: "list" } });
}

const row = (name: string) => screen.getByRole("button", { name }).closest("tr")!;

describe("Lista de volúmenes", () => {
  beforeEach(reset);

  it("lista los volúmenes con driver, contenedores y distintivos; el tamaño se calcula bajo demanda", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    expect(await screen.findByRole("button", { name: "datos" })).toBeInTheDocument();
    expect(within(row("datos")).getByText("2 montajes")).toBeInTheDocument();
    expect(within(row("datos")).getByText("db, web")).toBeInTheDocument();
    expect(within(row("huerfano")).getByText("Sin uso")).toBeInTheDocument();
    expect(within(row("huerfano")).getByText("Compose: blog")).toBeInTheDocument();
    expect(within(row("3f1c9e2b7a6d")).getByText("Anónimo")).toBeInTheDocument();
    expect(api.volumeSizes).not.toHaveBeenCalled();
    expect(within(row("datos")).getByText("—")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Calcular tamaños" }));
    await waitFor(() => expect(api.volumeSizes).toHaveBeenCalledTimes(1));
    expect(await within(row("datos")).findByText("50 MiB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Volver a calcular" })).toBeInTheDocument();
  });

  it("filtra y busca", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await screen.findByRole("button", { name: "datos" });
    await user.click(screen.getByRole("radio", { name: "Sin uso" }));
    expect(screen.getByText("2 volúmenes")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "datos" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Todos" }));
    await user.type(screen.getByPlaceholderText("Buscar por nombre, driver, contenedor o metadato"), "copias");
    expect(screen.getByText("1 volumen")).toBeInTheDocument();
  });

  it("eliminar un volumen en uso (también por un contenedor detenido): avisa y no deja continuar", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await user.click(await screen.findByRole("button", { name: "Eliminar copias" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText(/Lo monta 1 contenedor: Docker no elimina un volumen en uso, ni siquiera de un contenedor detenido/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Eliminar volumen" })).toBeDisabled();
    expect(within(dialog).queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("eliminar un volumen sin uso exige confirmar el borrado de datos y consulta el estado actual", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await user.click(await screen.findByRole("button", { name: "Eliminar huerfano" }));
    const dialog = screen.getByRole("dialog");
    const go = within(dialog).getByRole("button", { name: "Eliminar volumen" });
    expect(within(dialog).getByText("docker volume rm huerfano")).toBeInTheDocument();
    await user.click(await within(dialog).findByRole("checkbox", { name: "Entiendo que se borrarán sus datos" }));
    expect(api.volumesList).toHaveBeenCalledTimes(2);
    await user.click(go);
    await waitFor(() => expect(api.volumeRemove).toHaveBeenCalledWith("huerfano"));
    expect(await screen.findByText("Volumen huerfano eliminado.")).toBeInTheDocument();
  });

  it("si mientras tanto un contenedor empezó a usarlo, el diálogo lo dice", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await screen.findByRole("button", { name: "huerfano" });
    api.volumesList.mockResolvedValueOnce([...volumes.filter((v) => v.name !== "huerfano"), volume({ name: "huerfano", containers: [{ id: "n", name: "nuevo", state: "running", destination: "/d", rw: true }] })]);
    await user.click(screen.getByRole("button", { name: "Eliminar huerfano" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText(/Lo monta 1 contenedor/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Eliminar volumen" })).toBeDisabled();
  });

  it("limpiar: por defecto solo anónimos; con todos exige confirmar los que tienen nombre", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await user.click(await screen.findByRole("button", { name: "Limpiar" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText("Según el estado actual se eliminará 1 volumen:")).toBeInTheDocument();
    const go = within(dialog).getByRole("button", { name: "Limpiar volúmenes" });
    expect(go).toBeEnabled();
    await user.click(within(dialog).getByRole("radio", { name: /Todos los volúmenes que no monta ningún contenedor/ }));
    expect(within(dialog).getByText("Según el estado actual se eliminarán 2 volúmenes:")).toBeInTheDocument();
    expect(within(dialog).getByText("huerfano")).toBeInTheDocument();
    expect(go).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox", { name: /1 volumen con nombre/ }));
    await user.click(go);
    await waitFor(() => expect(api.volumesPrune).toHaveBeenCalledWith(true));
    expect(await screen.findByText("2 volúmenes eliminados; 4 KiB recuperados.")).toBeInTheDocument();
  });

  it("con Docker antiguo (API < 1.42) avisa de que también elimina los que tienen nombre", async () => {
    const user = userEvent.setup();
    useEngine.setState({ state: { ...connected, engine: { ...(connected as Extract<typeof connected, { state: "connected" }>).engine, apiVersion: "1.41" } } as typeof connected });
    renderWithQuery(<VolumesView />);
    await user.click(await screen.findByRole("button", { name: "Limpiar" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/API 1.41\) elimina todos los volúmenes sin uso/)).toBeInTheDocument();
    expect(await within(dialog).findByText("Según el estado actual se eliminarán 2 volúmenes:")).toBeInTheDocument();
    expect(within(dialog).queryByRole("radio")).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("checkbox"));
    await user.click(within(dialog).getByRole("button", { name: "Limpiar volúmenes" }));
    // No se envía --all (esa API no lo admite).
    await waitFor(() => expect(api.volumesPrune).toHaveBeenCalledWith(false));
  });

  it("crear un contenedor con el volumen abre el asistente con el montaje puesto", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await user.click(await screen.findByRole("button", { name: "Crear un contenedor con huerfano" }));
    expect(useApp.getState().section).toBe("containers");
    const page = useContainersNav.getState().page;
    expect(page.kind).toBe("create");
    if (page.kind === "create") expect(page.spec?.mounts).toEqual([{ kind: "volume", source: "huerfano", target: "/data", readOnly: false }]);
  });
});

describe("Detalle de volumen", () => {
  beforeEach(reset);

  it("muestra resumen, contenedores con su ruta y acceso, e inspección", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await user.click(await screen.findByRole("button", { name: "datos" }));
    await waitFor(() => expect(api.volumeDetails).toHaveBeenCalledWith("datos"));
    expect(await screen.findByText("/var/lib/docker/volumes/datos/_data")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Calcular" }));
    expect(await screen.findByText("50 MiB")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /Contenedores/ }));
    const web = screen.getByRole("button", { name: "web" }).closest("tr")!;
    expect(within(web).getByText("/var/lib/app")).toBeInTheDocument();
    expect(within(web).getByText("Lectura y escritura")).toBeInTheDocument();
    const db = screen.getByRole("button", { name: "db" }).closest("tr")!;
    expect(within(db).getByText("Solo lectura")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Inspección JSON" }));
    expect(screen.getByTestId("volume-json").textContent).toContain('"Name": "datos"');
  });

  it("si el volumen ya no existe lo indica", async () => {
    api.volumeDetails.mockRejectedValueOnce({ code: "engine.docker", message: "get x: no such volume", httpStatus: 404, issues: null });
    useVolumesNav.setState({ page: { kind: "detail", name: "x", tab: "summary" } });
    renderWithQuery(<VolumesView />);
    expect(await screen.findByText("El volumen x ya no existe.")).toBeInTheDocument();
  });
});

describe("Crear volumen", () => {
  beforeEach(reset);

  it("valida, ofrece tipos habituales y muestra el comando; un nombre existente se marca en el formulario", async () => {
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />);
    await user.click(await screen.findByRole("button", { name: "Crear volumen" }));
    expect(await screen.findByText("Listo para crear un volumen anónimo.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Nombre"), "a");
    expect(await screen.findAllByText("El nombre debe tener al menos dos caracteres.")).not.toHaveLength(0);
    const submit = screen.getByRole("button", { name: "Crear volumen" });
    expect(submit).toBeDisabled();
    await user.type(screen.getByLabelText("Nombre"), "pp");
    await user.click(screen.getByRole("radio", { name: "tmpfs" }));
    await waitFor(() => expect(screen.getByTestId("volume-command").textContent).toBe("docker volume create --driver local --opt type=tmpfs --opt device=tmpfs --opt o=size=100m app"));
    // El backend detecta al crear que ya existe: se muestra en el formulario.
    api.volumeCreate.mockRejectedValueOnce({ code: "engine.invalidSpec", message: "la configuración tiene 1 error(es)", httpStatus: null, issues: [{ severity: "error", field: "name", code: "volumeName.exists", params: { value: "app" } }] });
    await user.click(submit);
    expect(await screen.findAllByText("Ya existe un volumen llamado app. Docker devolvería el existente sin aplicar estos cambios.")).not.toHaveLength(0);
    expect(submit).toBeDisabled();
    // Al cambiar el nombre el aviso desaparece y se crea.
    await user.type(screen.getByLabelText("Nombre"), "2");
    await waitFor(() => expect(submit).toBeEnabled());
    await user.click(submit);
    await waitFor(() => expect(api.volumeCreate).toHaveBeenCalledTimes(2));
    expect(api.volumeCreate.mock.calls[1]![0]).toMatchObject({ name: "app2", driver: "local", driverOpts: [{ key: "type", value: "tmpfs" }, { key: "device", value: "tmpfs" }, { key: "o", value: "size=100m" }] });
    await waitFor(() => expect(useVolumesNav.getState().page).toMatchObject({ kind: "detail", name: "app2" }));
  });

  it("avisa mientras se escribe si el nombre ya existe en Docker", async () => {
    const user = userEvent.setup();
    useVolumesNav.setState({ page: { kind: "create" } });
    renderWithQuery(<VolumesView />);
    await user.type(await screen.findByLabelText("Nombre"), "datos");
    expect(await screen.findAllByText("Ya existe un volumen llamado datos. Docker devolvería el existente sin aplicar estos cambios.")).not.toHaveLength(0);
    expect(screen.getByRole("button", { name: "Crear volumen" })).toBeDisabled();
    expect(api.volumeCreate).not.toHaveBeenCalled();
  });

  it("en inglés el apartado aparece en inglés", async () => {
    await i18n.changeLanguage("en");
    try {
      renderWithQuery(<VolumesView />);
      expect(await screen.findByRole("button", { name: "datos" })).toBeInTheDocument();
      expect(screen.getByText("2 mounts")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Calculate sizes" })).toBeInTheDocument();
      expect(within(row("3f1c9e2b7a6d")).getByText("Anonymous")).toBeInTheDocument();
    } finally {
      await i18n.changeLanguage("es");
    }
  });
});
