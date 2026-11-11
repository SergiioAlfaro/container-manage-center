import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, imageDetails, images, streams } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { ImagesView, matchesImage } from "./ImagesView";
import { usePulls } from "./pull/pulls";
import { useBuildStore } from "./build/buildStore";
import { useImagesNav } from "./store";
import { formatInstruction } from "./detail/ImageDetail";
import { relativeTo } from "./build/BuildPage";

function reset() {
  vi.clearAllMocks();
  streams.reset();
  setServerState(connected);
  useImagesNav.setState({ page: { kind: "list" } });
  usePulls.setState({ items: [] });
  useBuildStore.setState({ byServer: {} });
}

const row = (text: string) => screen.getByText(text).closest("tr")!;

describe("utilidades", () => {
  it("filtra por uso, sin etiqueta y texto", () => {
    expect(matchesImage(images[0]!, "used", "")).toBe(true);
    expect(matchesImage(images[0]!, "unused", "")).toBe(false);
    expect(matchesImage(images[1]!, "dangling", "9999")).toBe(true);
  });
  it("muestra instrucciones legibles del historial", () => {
    expect(formatInstruction('/bin/sh -c #(nop)  CMD ["nginx"]')).toBe('CMD ["nginx"]');
    expect(formatInstruction("/bin/sh -c apt-get update")).toBe("RUN apt-get update");
    expect(formatInstruction("RUN /bin/sh -c make # buildkit")).toBe("RUN /bin/sh -c make");
  });
  it("calcula la ruta del Dockerfile dentro del contexto", () => {
    expect(relativeTo("/p", "/p/docker/Dockerfile")).toBe("docker/Dockerfile");
    expect(relativeTo("/p/", "/p/Dockerfile")).toBe("Dockerfile");
    expect(relativeTo("/p", "/otro/Dockerfile")).toBeNull();
    expect(relativeTo("/p", "/pp/Dockerfile")).toBeNull();
  });
});

describe("Lista de imágenes", () => {
  beforeEach(reset);

  it("lista imágenes con etiquetas, tamaño, ID corto y uso real", async () => {
    renderWithQuery(<ImagesView />);
    expect(await screen.findByText("nginx:1.27")).toBeInTheDocument();
    const nginx = row("nginx:1.27");
    expect(within(nginx).getByText("1111aaaabbbb")).toBeInTheDocument();
    expect(within(nginx).getByText("183 MiB")).toBeInTheDocument();
    expect(within(nginx).getByText("1 contenedor")).toBeInTheDocument();
    expect(within(screen.getByRole("table")).getByText("Sin etiqueta")).toBeInTheDocument();
  });

  it("ofrece crear un contenedor solo para imágenes con etiqueta y abre el asistente", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    expect(within(row("nginx:1.27")).getByRole("button", { name: "Crear contenedor con nginx:1.27" })).toBeInTheDocument();
    const dangling = within(screen.getByRole("table")).getByText("Sin etiqueta").closest("tr")!;
    expect(within(dangling).queryByRole("button", { name: /Crear contenedor/ })).not.toBeInTheDocument();
    await userEvent.click(within(row("nginx:1.27")).getByRole("button", { name: "Crear contenedor con nginx:1.27" }));
    expect(useApp.getState().section).toBe("containers");
    expect(useContainersNav.getState().page).toMatchObject({ kind: "create", spec: { image: "nginx:1.27" }, sourceName: null });
  });

  it("descargar: valida la referencia, elige credenciales y muestra el progreso real", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(screen.getByRole("button", { name: "Descargar imagen" }));
    const dialog = screen.getByRole("dialog");
    const input = within(dialog).getByLabelText("Imagen");
    await userEvent.type(input, "Nginx");
    expect(await within(dialog).findByText(/Nombre no válido/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Descargar" })).toBeDisabled();
    await userEvent.clear(input);
    await userEvent.type(input, "registro.local:5000/equipo/app:v2");
    expect(await within(dialog).findByText("Se descargará registro.local:5000/equipo/app con la etiqueta v2.")).toBeInTheDocument();
    expect(within(dialog).getByText("docker pull registro.local:5000/equipo/app:v2")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("radio", { name: "Usuario y contraseña" }));
    expect(within(dialog).getByRole("button", { name: "Descargar" })).toBeDisabled();
    await userEvent.type(within(dialog).getByLabelText("Usuario"), "ana");
    const pw = within(dialog).getByLabelText("Contraseña o token");
    expect(pw).toHaveAttribute("type", "password");
    await userEvent.type(pw, "s3creto");
    await userEvent.click(within(dialog).getByRole("button", { name: "Descargar" }));
    expect(api.imagePullOpen).toHaveBeenCalledWith("registro.local:5000/equipo/app:v2", { mode: "manual", username: "ana", password: "s3creto" }, expect.any(Function));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    act(() => streams.emit(1, { event: "progress", update: { layers: [{ id: "a1b2c3d4e5f6", status: "Downloading", current: 50, total: 100 }], messages: ["v2: Pulling from equipo/app"] } }));
    const panel = screen.getByRole("region", { name: "Descargas" });
    expect(within(panel).getByText("Descargando")).toBeInTheDocument();
    expect(within(panel).getByRole("progressbar", { name: /Progreso de/ })).toHaveAttribute("aria-valuenow", "25");
    act(() => streams.emit(1, { event: "end", closed: { end: { reason: "finished", summary: "registro.local:5000/equipo/app:v2: Downloaded newer image", exitCode: null }, error: null } }));
    expect(within(panel).getByText("Descargada")).toBeInTheDocument();
    expect(within(panel).getByText(/Downloaded newer image/)).toBeInTheDocument();
    await waitFor(() => expect(api.imagesList).toHaveBeenCalledTimes(2));
  });

  it("una descarga en curso se puede cancelar", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(screen.getByRole("button", { name: "Descargar imagen" }));
    await userEvent.type(within(screen.getByRole("dialog")).getByLabelText("Imagen"), "alpine");
    await waitFor(() => expect(within(screen.getByRole("dialog")).getByRole("button", { name: "Descargar" })).toBeEnabled());
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Descargar" }));
    expect(api.imagePullOpen).toHaveBeenCalledWith("alpine", { mode: "dockerConfig" }, expect.any(Function));
    await userEvent.click(within(screen.getByRole("region", { name: "Descargas" })).getByRole("button", { name: "Cancelar" }));
    expect(api.streamClose).toHaveBeenCalledWith(1);
    expect(screen.getByText("Cancelada")).toBeInTheDocument();
  });

  it("etiquetar valida el destino y muestra el comando", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(within(row("nginx:1.27")).getByRole("button", { name: "Etiquetar nginx:1.27" }));
    const dialog = screen.getByRole("dialog");
    await userEvent.type(within(dialog).getByLabelText("Repositorio"), "miusuario/web");
    expect(await within(dialog).findByText("docker tag nginx:1.27 miusuario/web:latest")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Etiquetar" }));
    expect(api.imageTag).toHaveBeenCalledWith("nginx:1.27", "miusuario/web", "latest");
    expect(await screen.findByText("Etiquetada como miusuario/web:latest.")).toBeInTheDocument();
  });

  it("eliminar: con varias etiquetas, quitar una no exige forzar; la imagen completa sí", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(within(row("nginx:1.27")).getByRole("button", { name: "Eliminar nginx:1.27" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("La usa 1 contenedor:")).toBeInTheDocument();
    expect(within(dialog).getByText("docker image rm nginx:1.27")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Quitar etiqueta" })).toBeEnabled();
    await userEvent.click(within(dialog).getByRole("radio", { name: /La imagen completa/ }));
    expect(within(dialog).getByText(/no elimina una imagen que usa un contenedor en ejecución/)).toBeInTheDocument();
    const btn = within(dialog).getByRole("button", { name: "Eliminar imagen" });
    expect(btn).toBeDisabled();
    await userEvent.click(within(dialog).getByRole("checkbox", { name: /Forzar la eliminación/ }));
    expect(within(dialog).getByText("docker image rm --force sha256:1111aaaabbbbcccc")).toBeInTheDocument();
    await userEvent.click(btn);
    expect(api.imageRemove).toHaveBeenCalledWith("sha256:1111aaaabbbbcccc", true);
  });

  it("los errores de Docker al eliminar se muestran en el diálogo", async () => {
    api.imageRemove.mockRejectedValueOnce({ code: "engine.docker", message: "conflict: unable to delete 1111aaaabbbb (cannot be forced) - image is being used by running container a1b2c3d4e5f6", httpStatus: 409, issues: null });
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(within(row("nginx:1.27")).getByRole("button", { name: "Eliminar nginx:1.27" }));
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Quitar etiqueta" }));
    expect(await within(screen.getByRole("dialog")).findByText(/cannot be forced/)).toBeInTheDocument();
  });

  it("limpiar: estima lo que se eliminará y exige confirmación para todas", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(screen.getByRole("button", { name: "Limpiar" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Según la lista actual se eliminaría 1 imagen (4,8 MiB).")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("radio", { name: /Todas las imágenes/ }));
    expect(within(dialog).getByRole("button", { name: "Limpiar" })).toBeDisabled();
    await userEvent.click(within(dialog).getByLabelText(/Entiendo que tendré/));
    await userEvent.click(within(dialog).getByRole("button", { name: "Limpiar" }));
    expect(api.imagesPrune).toHaveBeenCalledWith(true);
    expect(await screen.findByText("Limpieza terminada: 1 elementos eliminados, 4,8 MiB recuperados.")).toBeInTheDocument();
  });
});

describe("Limpieza y descargas", () => {
  beforeEach(reset);

  it("no deja limpiar mientras hay una descarga en curso (Docker la haría fallar, D-050)", async () => {
    usePulls.setState({ items: [{ key: 1, server: "local", reference: "python:3.13", stream: 1, status: "running", layers: [], messages: [], summary: null, error: null, cancelRequested: false }] });
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(screen.getByRole("button", { name: "Limpiar" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/Hay 1 descarga en curso. Espera a que termine/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Limpiar" })).toBeDisabled();
    // Al terminar la descarga se puede limpiar.
    act(() => usePulls.setState({ items: [{ key: 1, server: "local", reference: "python:3.13", stream: null, status: "done", layers: [], messages: [], summary: "ok", error: null, cancelRequested: false }] }));
    expect(within(dialog).queryByText(/descarga en curso/)).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Limpiar" })).toBeEnabled();
  });
});

describe("Búsqueda", () => {
  beforeEach(reset);

  it("busca en Docker Hub y descarga un resultado", async () => {
    useImagesNav.getState().showSearch();
    renderWithQuery(<ImagesView />);
    await userEvent.type(screen.getByLabelText("Término de búsqueda"), "redis");
    await userEvent.click(screen.getByLabelText("Solo imágenes oficiales"));
    await userEvent.click(screen.getByRole("button", { name: "Buscar" }));
    expect(api.imagesSearch).toHaveBeenCalledWith("redis", 25, true);
    expect(await screen.findByText("Official build of redis")).toBeInTheDocument();
    expect(screen.getByText("21.000")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Descargar redis" }));
    expect(within(screen.getByRole("dialog")).getByLabelText("Imagen")).toHaveValue("redis");
  });

  it("sin Internet en el daemon muestra el error real", async () => {
    api.imagesSearch.mockRejectedValueOnce({ code: "engine.docker", message: "Get \"https://index.docker.io/v1/search\": dial tcp: lookup index.docker.io: no such host", httpStatus: 500, issues: null });
    useImagesNav.getState().showSearch();
    renderWithQuery(<ImagesView />);
    await userEvent.type(screen.getByLabelText("Término de búsqueda"), "redis");
    await userEvent.click(screen.getByRole("button", { name: "Buscar" }));
    expect(await screen.findByText(/no such host/)).toBeInTheDocument();
    expect(screen.getByText(/Si Docker no tiene acceso a Internet/)).toBeInTheDocument();
  });
});

describe("Detalle de imagen", () => {
  beforeEach(reset);

  it("se abre desde la lista y muestra la configuración, con secretos ocultos", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(screen.getByTitle("Abrir nginx:1.27"));
    expect(await screen.findByRole("tab", { name: "Resumen" })).toHaveAttribute("aria-selected", "true");
    expect(api.imageDetails).toHaveBeenCalledWith("sha256:1111aaaabbbbcccc");
    expect(screen.getByText("linux/amd64")).toBeInTheDocument();
    expect(screen.getByText("80/tcp")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("t0ken");
    await userEvent.click(screen.getByRole("button", { name: "Mostrar 1 valor oculto" }));
    expect(document.body.textContent).toContain("t0ken");
  });

  it("capas, contenedores e inspección", async () => {
    useImagesNav.getState().showDetail("sha256:1111aaaabbbbcccc", "nginx:1.27");
    renderWithQuery(<ImagesView />);
    await userEvent.click(await screen.findByRole("tab", { name: "Capas" }));
    expect(await screen.findByText("RUN apt-get update && apt-get install -y nginx")).toBeInTheDocument();
    expect(screen.getByText('CMD ["nginx" "-g" "daemon off;"]')).toBeInTheDocument();
    expect(screen.getByText(/^3 entradas en el historial, 2 con datos, 12[78] MiB en total\.$/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("tab", { name: /Contenedores/ }));
    await userEvent.click(screen.getByRole("button", { name: "web" }));
    expect(useApp.getState().section).toBe("containers");
    expect(useContainersNav.getState().page).toMatchObject({ kind: "detail", id: "a1b2c3d4e5f6a7b8c9d0" });
  });

  it("una imagen sin nombre no se puede descargar de nuevo ni usar para crear contenedores", async () => {
    api.imageDetails.mockResolvedValueOnce(imageDetails("sha256:9999ddddeeeeffff", { id: "sha256:9999ddddeeeeffff", repoTags: [], repoDigests: [] }));
    useImagesNav.getState().showDetail("sha256:9999ddddeeeeffff", "9999ddddeeee");
    renderWithQuery(<ImagesView />);
    await screen.findByRole("tab", { name: "Resumen" });
    expect(screen.queryByRole("button", { name: "Descargar de nuevo" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Crear contenedor" })).not.toBeInTheDocument();
    expect(screen.getByText(/imagen construida o importada en este equipo/)).toBeInTheDocument();
  });

  it("eliminar desde el detalle vuelve a la lista sin volver a pedir la imagen borrada", async () => {
    api.imageRemove.mockResolvedValueOnce({ untagged: ["nginx:1.27", "nginx:latest"], deleted: ["sha256:1111aaaabbbbcccc"] });
    useImagesNav.getState().showDetail("sha256:1111aaaabbbbcccc", "nginx:1.27");
    renderWithQuery(<ImagesView />);
    await screen.findByRole("tab", { name: "Resumen" });
    expect(api.imageDetails).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole("button", { name: "Eliminar" }));
    const dialog = screen.getByRole("dialog");
    await userEvent.click(within(dialog).getByRole("radio", { name: /La imagen completa/ }));
    await userEvent.click(within(dialog).getByRole("checkbox", { name: /Forzar/ }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Eliminar imagen" }));
    await waitFor(() => expect(useImagesNav.getState().page.kind).toBe("list"));
    expect(api.imageDetails).toHaveBeenCalledTimes(1);
  });
});

describe("Construcción", () => {
  beforeEach(reset);

  it("valida, resume el contexto y muestra el comando con los secretos ocultos", async () => {
    useImagesNav.getState().showBuild();
    renderWithQuery(<ImagesView />);
    await waitFor(() => expect(api.buildPreview).toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "Construir" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Elegir carpeta de contexto" }));
    await userEvent.click(screen.getByRole("button", { name: "Añadir nombre" }));
    await userEvent.type(screen.getByLabelText("Nombre 1"), "miapp:1.0");
    await userEvent.click(screen.getByRole("button", { name: "Añadir argumento" }));
    await userEvent.type(screen.getByLabelText("Clave 1"), "NPM_TOKEN");
    const v = screen.getByLabelText("Valor 1");
    expect(v).toHaveAttribute("type", "password");
    await userEvent.type(v, "abc");
    await waitFor(() => expect(screen.getByTestId("build-command")).toHaveTextContent("docker build --tag miapp:1.0 --build-arg NPM_TOKEN=******** /home/u/datos"));
    expect(screen.getByTestId("context-summary")).toHaveTextContent("12 archivos en 3 carpetas");
    expect(screen.getByText(".dockerignore excluye 4 entradas.")).toBeInTheDocument();
    expect(screen.getByText(/1 valor sensible aparece como/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "Clásico" }));
    await waitFor(() => expect(screen.getByTestId("build-command")).toHaveTextContent(/^DOCKER_BUILDKIT=0 docker build/));
  });

  it("el Dockerfile se elige dentro del contexto", async () => {
    useImagesNav.getState().showBuild({ context: "/home/u/proyecto", dockerfile: "Dockerfile", tags: [], buildArgs: [], labels: [], target: "", noCache: false, pull: false, builder: "buildKit" });
    renderWithQuery(<ImagesView />);
    await userEvent.click(await screen.findByRole("button", { name: "Elegir Dockerfile" }));
    expect(screen.getByLabelText("Dockerfile")).toHaveValue("docker/Dockerfile.prod");
    api.pickFile.mockResolvedValueOnce("/tmp/otro/Dockerfile");
    await userEvent.click(screen.getByRole("button", { name: "Elegir Dockerfile" }));
    expect(screen.getByText("El Dockerfile debe estar dentro de la carpeta de contexto.")).toBeInTheDocument();
  });

  it("construye mostrando pasos y salida, y termina con la imagen", async () => {
    useImagesNav.getState().showBuild({ context: "/home/u/proyecto", dockerfile: "Dockerfile", tags: ["miapp:1.0"], buildArgs: [], labels: [], target: "", noCache: false, pull: false, builder: "buildKit" });
    renderWithQuery(<ImagesView />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Construir" })).toBeEnabled());
    await userEvent.click(screen.getByRole("button", { name: "Construir" }));
    expect(api.buildOpen).toHaveBeenCalledWith(expect.objectContaining({ context: "/home/u/proyecto", tags: ["miapp:1.0"] }), expect.any(Function));
    act(() =>
      streams.emit(1, {
        event: "progress",
        update: {
          steps: [
            { number: 1, name: "[1/2] FROM node:22", state: "cached", durationMs: null, error: null },
            { number: 2, name: "[2/2] RUN npm ci", state: "running", durationMs: null, error: null },
          ],
          lines: ["#1 [1/2] FROM node:22", "#1 CACHED", "#2 [2/2] RUN npm ci", "#2 added 120 packages"],
        },
      }),
    );
    expect(screen.getByText("Construyendo…")).toBeInTheDocument();
    expect(screen.getByTestId("build-log")).toHaveTextContent("#2 added 120 packages");
    expect(within(screen.getByRole("list", { name: "Pasos" })).getByText("caché")).toBeInTheDocument();
    act(() =>
      streams.emit(1, {
        event: "progress",
        update: { steps: [{ number: 2, name: "[2/2] RUN npm ci", state: "done", durationMs: 4200, error: null }], lines: ["#2 DONE 4.2s"] },
      }),
    );
    expect(screen.getByText("4.2 s")).toBeInTheDocument();
    act(() => streams.emit(1, { event: "end", closed: { result: { end: { reason: "finished", summary: "Imagen construida sha256:abc (miapp:1.0): 2 pasos, 1 de la caché", exitCode: null }, imageId: "sha256:abcdef0123456789", tags: ["miapp:1.0"] }, error: null } }));
    expect(screen.getByText("Imagen construida")).toBeInTheDocument();
    expect(screen.getByText(/2 pasos, 1 de la caché/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Ver imagen" }));
    expect(useImagesNav.getState().page).toMatchObject({ kind: "detail", id: "sha256:abcdef0123456789", name: "miapp:1.0" });
  });

  it("cancelar cierra el flujo y queda como cancelada", async () => {
    useImagesNav.getState().showBuild({ context: "/home/u/proyecto", dockerfile: "Dockerfile", tags: ["miapp:1.0"], buildArgs: [], labels: [], target: "", noCache: false, pull: false, builder: "buildKit" });
    renderWithQuery(<ImagesView />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Construir" })).toBeEnabled());
    await userEvent.click(screen.getByRole("button", { name: "Construir" }));
    await waitFor(() => expect(useBuildStore.getState().byServer.local?.stream).toBe(1));
    await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(api.streamClose).toHaveBeenCalledWith(1);
    expect(await screen.findByText("Construcción cancelada")).toBeInTheDocument();
    expect(screen.getByText("Construcción cancelada por el usuario (2 pasos iniciados)")).toBeInTheDocument();
  });

  it("un fallo muestra el error de Docker y permite volver al formulario con los datos", async () => {
    useImagesNav.getState().showBuild({ context: "/home/u/proyecto", dockerfile: "Dockerfile", tags: ["miapp:1.0"], buildArgs: [], labels: [], target: "", noCache: false, pull: false, builder: "buildKit" });
    renderWithQuery(<ImagesView />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Construir" })).toBeEnabled());
    await userEvent.click(screen.getByRole("button", { name: "Construir" }));
    act(() =>
      streams.emit(1, {
        event: "end",
        closed: { result: null, error: { code: "engine.docker", message: "process \"/bin/sh -c npm ci\" did not complete successfully: exit code: 1", httpStatus: 500, issues: null } },
      }),
    );
    expect(screen.getByText("La construcción falló")).toBeInTheDocument();
    expect(screen.getByText(/did not complete successfully: exit code: 1/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Volver al formulario" }));
    expect(screen.getByLabelText("Carpeta de contexto")).toHaveValue("/home/u/proyecto");
    expect(screen.getByLabelText("Nombre 1")).toHaveValue("miapp:1.0");
  });
});
