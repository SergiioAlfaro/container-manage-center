import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, streams, TIENDA_YAML } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { ComposeView } from "./ComposeView";
import { useComposeRuns } from "./runs";
import { useComposeNav } from "./store";

function reset() {
  vi.clearAllMocks();
  streams.reset();
  setServerState(connected);
  useComposeNav.setState({ page: { kind: "list" } });
  useComposeRuns.setState({ runs: {} });
}

const row = (name: string) => screen.getByRole("button", { name }).closest("tr")!;

describe("Docker Compose: lista", () => {
  beforeEach(reset);

  it("muestra los proyectos desplegados y los de la lista, con estado y carpeta", async () => {
    renderWithQuery(<ComposeView />);
    expect(await screen.findByRole("button", { name: "tienda" })).toBeInTheDocument();
    expect(within(row("tienda")).getByText("En marcha · 2/2")).toBeInTheDocument();
    expect(within(row("tienda")).getByText("En la lista")).toBeInTheDocument();
    expect(within(row("blog")).getByText("Detenido · 0/1")).toBeInTheDocument();
    expect(within(row("blog")).getByText("Faltan archivos")).toBeInTheDocument();
    expect(within(row("nuevo")).getByText("Sin desplegar")).toBeInTheDocument();
    expect(within(row("nuevo")).getByText("/home/u/nuevo")).toBeInTheDocument();
    expect(screen.getByText("3 proyectos")).toBeInTheDocument();
  });

  it("filtra por estado y busca", async () => {
    const user = userEvent.setup();
    renderWithQuery(<ComposeView />);
    await screen.findByRole("button", { name: "tienda" });
    await user.click(screen.getByRole("radio", { name: "Detenidos" }));
    expect(screen.getByText("1 proyecto")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "tienda" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Todos" }));
    await user.type(screen.getByPlaceholderText("Buscar por nombre, carpeta o servicio"), "web");
    expect(screen.getByText("1 proyecto")).toBeInTheDocument();
  });

  it("detener desde la lista ejecuta stop con la salida en vivo y deja constancia en la fila", async () => {
    const user = userEvent.setup();
    renderWithQuery(<ComposeView />);
    await user.click(await screen.findByRole("button", { name: "Detener tienda" }));
    await waitFor(() => expect(api.composeRunOpen).toHaveBeenCalledTimes(1));
    const [p, a, confirmed] = api.composeRunOpen.mock.calls[0]!;
    expect(p.name).toBe("tienda");
    expect(a).toEqual({ action: "stop", services: [] });
    expect(confirmed).toBe(false);
    expect(await within(row("tienda")).findByText("Detención en curso…")).toBeInTheDocument();
    act(() => streams.emit(1, { event: "end", closed: { end: { end: "exited", code: 0 }, error: null, stderrTail: "" } }));
    expect(await within(row("tienda")).findByText("Detención completada")).toBeInTheDocument();
    await waitFor(() => expect(api.composeProjects).toHaveBeenCalledTimes(2));
  });

  it("si Compose no está instalado lo explica con las instrucciones de instalación", async () => {
    api.composeStatus.mockResolvedValueOnce({ status: "notInstalled", cliPath: "/usr/bin/docker", searched: [] });
    renderWithQuery(<ComposeView />);
    expect(await screen.findByText("Docker Compose no está disponible")).toBeInTheDocument();
    expect(screen.getByText("sudo apt install docker-compose")).toBeInTheDocument();
    expect(api.composeProjects).not.toHaveBeenCalled();
  });

  it("abrir un proyecto: elige archivos, nombre opcional y muestra el error de Compose si los rechaza", async () => {
    const user = userEvent.setup();
    api.composeRegister.mockResolvedValueOnce({ project: null, error: "services.web.ports must be a array", command: "x" });
    renderWithQuery(<ComposeView />);
    await user.click(await screen.findByRole("button", { name: "Abrir proyecto…" }));
    const d = screen.getByRole("dialog");
    expect(within(d).getByRole("button", { name: "Añadir" })).toBeDisabled();
    await user.click(within(d).getByRole("button", { name: "Elegir archivo…" }));
    expect(within(d).getByText("/home/u/tienda/compose.yaml")).toBeInTheDocument();
    await user.type(within(d).getByRole("textbox"), "Mal");
    expect(within(d).getByText(/Solo minúsculas/)).toBeInTheDocument();
    expect(within(d).getByRole("button", { name: "Añadir" })).toBeDisabled();
    await user.clear(within(d).getByRole("textbox"));
    await user.click(within(d).getByRole("button", { name: "Añadir" }));
    expect(await within(d).findByText(/Docker Compose rechaza los archivos: services.web.ports/)).toBeInTheDocument();
    expect(api.composeRegister).toHaveBeenCalledWith(["/home/u/tienda/compose.yaml"], null);
    await user.click(within(d).getByRole("button", { name: "Añadir" }));
    await waitFor(() => expect(useComposeNav.getState().page).toEqual({ kind: "detail", name: "proyecto", tab: "services" }));
  });
});

describe("Docker Compose: proyecto", () => {
  beforeEach(reset);

  async function open(name = "tienda") {
    useComposeNav.setState({ page: { kind: "detail", name, tab: "services" } });
    const r = renderWithQuery(<ComposeView />);
    await screen.findByRole("heading", { name });
    return r;
  }

  it("servicios: archivo y contenedores juntos, perfiles y servicios sin crear", async () => {
    await open();
    expect(await screen.findByText("perfil debug")).toBeInTheDocument();
    const web = screen.getByText("web", { selector: "span" }).closest("tr")!;
    expect(within(web).getByText("nginx:1.27")).toBeInTheDocument();
    expect(within(web).getByText("8080→80")).toBeInTheDocument();
    expect(within(web).getByRole("button", { name: "tienda-web-1" })).toBeInTheDocument();
    const extra = screen.getByText("extra", { selector: "span" }).closest("tr")!;
    expect(within(extra).getByText("Sin crear")).toBeInTheDocument();
    expect(within(screen.getByText("db", { selector: "span" }).closest("tr")!).getByText("Se construye (./db)")).toBeInTheDocument();
    expect(api.composeConfig).toHaveBeenCalledWith({ name: "tienda", files: ["/home/u/tienda/compose.yaml"], workingDir: "/home/u/tienda", envFiles: [] });
  });

  it("desplegar con riesgos para el equipo exige aceptarlos y los envía al motor (E1, D-086)", async () => {
    const original = api.composePreview.getMockImplementation()!;
    const risk = { code: "risk.runtimeSocket", level: "critical" as const, field: "services.web.volumes.0", key: "risk.runtimeSocket:/var/run/docker.sock@web", params: { level: "critical", path: "/var/run/docker.sock", readOnly: "true", service: "web" } };
    api.composePreview.mockImplementation(async (p, a) => ({ command: `docker compose -p ${p.name} ${a.action}`, risks: a.action === "up" ? [risk] : [] }));
    const user = userEvent.setup();
    await open();
    await screen.findByText("perfil debug");
    await user.click(screen.getByRole("button", { name: "Desplegar" }));
    const d = screen.getByRole("dialog");
    const box = await within(d).findByTestId("host-risks");
    expect(within(box).getByText(/web: \/var\/run\/docker\.sock: el socket del motor/)).toBeInTheDocument();
    const deploy = within(d).getByRole("button", { name: "Desplegar" });
    expect(deploy).toBeDisabled();
    await user.click(within(box).getByRole("checkbox"));
    expect(deploy).toBeEnabled();
    await user.click(deploy);
    await waitFor(() => expect(api.composeRunOpen).toHaveBeenCalledTimes(1));
    expect(api.composeRunOpen.mock.calls[0]![4]).toEqual([risk.key]);
    api.composePreview.mockImplementation(original);
  });

  it("desplegar: opciones, comando exacto y salida en vivo hasta el final", async () => {
    const user = userEvent.setup();
    await open();
    await screen.findByText("perfil debug");
    await user.click(screen.getByRole("button", { name: "Desplegar" }));
    const d = screen.getByRole("dialog");
    await user.click(within(d).getByRole("button", { name: "debug" }));
    await user.click(within(d).getByRole("checkbox", { name: /Construir las imágenes antes/ }));
    await user.click(within(d).getByRole("radio", { name: "Siempre" }));
    await waitFor(() => expect(within(d).getByTestId("compose-dialog-command")).toHaveTextContent("--profile debug up --detach --build --pull always"));
    await user.click(within(d).getByRole("button", { name: "Desplegar" }));
    await waitFor(() => expect(api.composeRunOpen).toHaveBeenCalledTimes(1));
    expect(api.composeRunOpen.mock.calls[0]![1]).toEqual({ action: "up", services: [], build: true, pull: "always", forceRecreate: false, removeOrphans: false, profiles: ["debug"] });
    // Se pasa a la pestaña de salida y llegan las líneas.
    expect(await screen.findByTestId("compose-run-command")).toHaveTextContent("up --detach --build");
    act(() => streams.emit(1, { event: "lines", lines: [{ stream: "stderr", text: " Container tienda-web-1  Started" }] }));
    expect(screen.getByTestId("compose-run-output")).toHaveTextContent("Container tienda-web-1 Started");
    // El estado se ve en la cabecera y en la pestaña de salida.
    expect(screen.getAllByText("Despliegue en curso…")).toHaveLength(2);
    // Detener: cierra el flujo (el backend termina el proceso).
    await user.click(screen.getByRole("button", { name: "Interrumpir" }));
    expect(streams.closed).toEqual([1]);
    expect(await screen.findAllByText("Despliegue interrumpido por ti")).toHaveLength(2);
    expect(screen.getByText(/Se interrumpió Docker Compose/)).toBeInTheDocument();
  });

  it("eliminar exige confirmar; con volúmenes avisa de la pérdida de datos y envía confirmed", async () => {
    const user = userEvent.setup();
    await open();
    await screen.findByText("perfil debug");
    await user.click(screen.getByRole("button", { name: "Eliminar" }));
    const d = screen.getByRole("dialog");
    expect(within(d).getByText(/Se detendrán y eliminarán los 2 contenedores/)).toBeInTheDocument();
    const go = within(d).getByRole("button", { name: "Eliminar" });
    expect(go).toBeDisabled();
    await user.click(within(d).getByRole("checkbox", { name: /Eliminar también los volúmenes/ }));
    expect(within(d).getByText("tienda_datos")).toBeInTheDocument();
    expect(within(d).getByText(/se perderán definitivamente/)).toBeInTheDocument();
    await waitFor(() => expect(within(d).getByTestId("compose-dialog-command")).toHaveTextContent("--profile '*' down --volumes"));
    await user.click(within(d).getByRole("checkbox", { name: /Entiendo que se eliminarán/ }));
    await user.click(go);
    await waitFor(() => expect(api.composeRunOpen).toHaveBeenCalledTimes(1));
    const [, a, confirmed] = api.composeRunOpen.mock.calls[0]!;
    expect(a).toEqual({ action: "down", volumes: true, images: "none", removeOrphans: false });
    expect(confirmed).toBe(true);
  });

  it("un error al abrir (p. ej. operación en curso) se muestra y no queda nada en marcha", async () => {
    const user = userEvent.setup();
    api.composeRunOpen.mockRejectedValueOnce({ code: "engine.busy", message: "ya hay una operación en curso sobre compose:tienda" });
    await open();
    await screen.findByText("perfil debug");
    await user.click(screen.getByRole("button", { name: "Reiniciar" }));
    expect(await screen.findByText("ya hay una operación en curso sobre compose:tienda")).toBeInTheDocument();
    expect(screen.getAllByText("Reinicio: no se pudo ejecutar")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Reiniciar" })).toBeEnabled();
  });

  it("proyecto sin archivos: se puede iniciar o eliminar, pero no desplegar ni editar", async () => {
    await open("blog");
    expect(await screen.findByText(/No se encuentran estos archivos: \/srv\/blog\/compose.yaml/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Desplegar" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Iniciar" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Eliminar" })).toBeEnabled();
    expect(screen.queryByRole("tab", { name: "Archivo" })).not.toBeInTheDocument();
    expect(api.composeConfig).not.toHaveBeenCalled();
    // Un proyecto desplegado pero no añadido no ofrece guardarlo si faltan sus archivos.
    expect(screen.queryByRole("button", { name: "Añadir a la lista" })).not.toBeInTheDocument();
  });

  it("configuración resuelta: oculta hasta pedirla, porque puede contener secretos", async () => {
    const user = userEvent.setup();
    await open();
    await user.click(screen.getByRole("tab", { name: "Configuración" }));
    expect(await screen.findByText(/incluye los valores de las variables/)).toBeInTheDocument();
    expect(screen.queryByTestId("compose-resolved")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Mostrar configuración resuelta" }));
    expect(screen.getByTestId("compose-resolved")).toHaveTextContent("s3creto");
  });

  it("registros: abre logs --follow del servicio elegido y los muestra en vivo", async () => {
    const user = userEvent.setup();
    await open();
    await screen.findByText("perfil debug");
    const web = screen.getByText("web", { selector: "span" }).closest("tr")!;
    await user.click(within(web).getByRole("button", { name: "Registros de web" }));
    await waitFor(() => expect(api.composeLogsOpen).toHaveBeenCalledTimes(1));
    expect(api.composeLogsOpen.mock.calls[0]!.slice(1, 4)).toEqual([["web"], 100, false]);
    act(() => streams.emit(1, { event: "lines", lines: [{ stream: "stdout", text: "web-1  | GET / 200" }] }));
    expect(screen.getByTestId("compose-logs")).toHaveTextContent("GET / 200");
    expect(screen.getByTestId("compose-logs-status")).toHaveTextContent("En directo");
    await user.click(screen.getByRole("button", { name: "Pausar" }));
    expect(streams.closed).toEqual([1]);
    expect(screen.getByTestId("compose-logs-status")).toHaveTextContent("En pausa");
  });

  it("editor: valida con Compose sin guardar, se niega a guardar algo inválido salvo que se insista y detecta cambios externos", async () => {
    const user = userEvent.setup();
    await open();
    await user.click(screen.getByRole("tab", { name: "Archivo" }));
    const area = await screen.findByRole("textbox", { name: "Contenido de compose.yaml" });
    expect(area).toHaveValue(TIENDA_YAML);
    expect(screen.getByRole("button", { name: "Guardar" })).toBeDisabled();
    await user.clear(area);
    await user.type(area, "services:{enter}  web:{enter}    image: x{enter}    ports: 12");
    expect(screen.getByText("Cambios sin guardar")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    expect(await screen.findByText("validating -: services.web.ports must be a array")).toBeInTheDocument();
    expect(api.composeConfig).toHaveBeenLastCalledWith(expect.objectContaining({ name: "tienda" }), { index: 0, content: expect.stringContaining("ports: 12") });
    expect(api.composeFileSave).not.toHaveBeenCalled();
    // Corregido: se guarda con la versión leída.
    await user.clear(area);
    await user.type(area, "services:{enter}  web:{enter}    image: nginx");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(api.composeFileSave).toHaveBeenCalledWith("/home/u/tienda/compose.yaml", expect.stringContaining("image: nginx"), "v1"));
    expect(await screen.findByText("Guardado")).toBeInTheDocument();
    // Otro programa cambia el archivo: no se sobrescribe.
    api.composeFileSave.mockRejectedValueOnce({ code: "engine.fileChanged", message: "ha cambiado" });
    await user.type(area, ":1.27");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    expect(await screen.findByText(/El archivo ha cambiado en el disco/)).toBeInTheDocument();
    expect(api.composeFileSave).toHaveBeenLastCalledWith("/home/u/tienda/compose.yaml", expect.any(String), "v2");
  });

  it("nuevo proyecto: carpeta, nombre en el archivo, validación y alta en la lista", async () => {
    const user = userEvent.setup();
    useComposeNav.setState({ page: { kind: "new" } });
    renderWithQuery(<ComposeView />);
    await user.click(await screen.findByRole("button", { name: "Elegir carpeta…" }));
    expect(await screen.findByText("/home/u/datos")).toBeInTheDocument();
    await user.type(screen.getByPlaceholderText("datos"), "demo");
    const area = screen.getByRole("textbox", { name: "Contenido de compose.yaml" });
    expect((area as HTMLTextAreaElement).value).toMatch(/^name: demo\n/);
    await user.click(screen.getByRole("button", { name: "Crear proyecto" }));
    await waitFor(() => expect(api.composeFileCreate).toHaveBeenCalledWith("/home/u/datos", expect.stringMatching(/^name: demo\n/)));
    expect(api.composeConfig).toHaveBeenCalledWith({ name: "demo", files: ["/home/u/datos/compose.yaml"], workingDir: null, envFiles: [] }, { index: 0, content: expect.any(String) });
    expect(api.composeRegister).toHaveBeenCalledWith(["/home/u/datos/compose.yaml"], "demo");
    await waitFor(() => expect(useComposeNav.getState().page).toEqual({ kind: "detail", name: "demo", tab: "services" }));
  });

  it("nuevo proyecto en una carpeta que ya tiene compose.yaml: no la sobrescribe y ofrece abrirlo", async () => {
    const user = userEvent.setup();
    api.composeFindFile.mockResolvedValueOnce("/home/u/datos/docker-compose.yml");
    useComposeNav.setState({ page: { kind: "new" } });
    renderWithQuery(<ComposeView />);
    await user.click(await screen.findByRole("button", { name: "Elegir carpeta…" }));
    expect(await screen.findByText(/ya tiene un archivo de Docker Compose/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Crear proyecto" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Abrir ese proyecto" }));
    await waitFor(() => expect(api.composeRegister).toHaveBeenCalledWith(["/home/u/datos/docker-compose.yml"], null));
    expect(api.composeFileCreate).not.toHaveBeenCalled();
  });
});
