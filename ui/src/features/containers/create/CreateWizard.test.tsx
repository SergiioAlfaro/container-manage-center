import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { ContainersView } from "../ContainersView";
import { useContainersNav } from "../store";
import { effectiveSpec, normalizeRef } from "./CreateWizard";
import { emptySpec } from "../specModel";

const preview = () => screen.getByTestId("cli-preview");
const lastSpec = () => api.containerPreview.mock.calls.at(-1)![0];

async function openWizard() {
  renderWithQuery(<ContainersView />);
  await userEvent.click(await screen.findByRole("button", { name: "Crear contenedor" }));
  await screen.findByRole("tab", { name: /General/ });
}

describe("normalizeRef / effectiveSpec", () => {
  it("completa la etiqueta latest como Docker", () => {
    expect(normalizeRef("nginx")).toBe("nginx:latest");
    expect(normalizeRef("registry:5000/app")).toBe("registry:5000/app:latest");
    expect(normalizeRef("nginx:1.27")).toBe("nginx:1.27");
    expect(normalizeRef("nginx@sha256:abc")).toBe("nginx@sha256:abc");
  });
  it("traduce los campos de texto y detecta formatos incorrectos", () => {
    const base = { command: `sh -c 'echo hola'`, entrypoint: "", cpus: "1,5", mem: { value: "512", unit: "m" as const }, memRes: { value: "x", unit: "m" as const }, capAdd: "net_admin, SYS_TIME", capDrop: "" };
    const r = effectiveSpec(emptySpec(), base);
    expect(r.spec.command).toEqual(["sh", "-c", "echo hola"]);
    expect(r.spec.cpus).toBe(1.5);
    expect(r.spec.memoryBytes).toBe(512 * 1024 ** 2);
    expect(r.spec.memoryReservationBytes).toBeNull();
    expect(r.spec.capAdd).toEqual(["NET_ADMIN", "SYS_TIME"]);
    expect(r.issues.map((i) => i.code)).toEqual(["memory.invalid"]);
  });
});

describe("Asistente de creación", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setServerState(connected);
    useContainersNav.setState({ page: { kind: "list" } });
  });

  it("muestra el comando equivalente en vivo y bloquea la creación sin imagen", async () => {
    await openWizard();
    await waitFor(() => expect(api.containerPreview).toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "Crear e iniciar" })).toBeDisabled();
    expect(screen.getByText("Corrige 1 error para continuar.")).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    await userEvent.type(screen.getByLabelText("Nombre"), "web2");
    await waitFor(() => expect(preview()).toHaveTextContent("docker run --detach --name web2 nginx:1.27"));
    expect(screen.getByRole("button", { name: "Crear e iniciar" })).toBeEnabled();
    // También el docker create que se ejecuta primero.
    await userEvent.click(screen.getByRole("radio", { name: "docker create" }));
    expect(preview()).toHaveTextContent("docker create --name web2 nginx:1.27");
  });

  it("divide el comando en argumentos sin shell y avisa de comillas sin cerrar", async () => {
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    const cmd = screen.getByLabelText("Comando");
    await userEvent.type(cmd, `nginx -g 'daemon off;'`);
    await waitFor(() => expect(lastSpec().command).toEqual(["nginx", "-g", "daemon off;"]));
    expect(within(screen.getByLabelText("Argumentos resultantes")).getByText("daemon off;")).toBeInTheDocument();
    await userEvent.type(cmd, " 'x");
    expect(await screen.findAllByText("Hay una comilla sin cerrar.")).not.toHaveLength(0);
    expect(screen.getByRole("button", { name: "Crear e iniciar" })).toBeDisabled();
  });

  it("usa la configuración real de la imagen local como ayuda", async () => {
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    await waitFor(() => expect(api.imageDefaults).toHaveBeenCalledWith("nginx:1.27"));
    expect(await screen.findByPlaceholderText("nginx -g 'daemon off;'")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("tab", { name: "Red" }));
    await userEvent.click(screen.getByRole("button", { name: "Publicar los expuestos (80/tcp)" }));
    await waitFor(() => expect(lastSpec().ports).toEqual([{ hostIp: "", hostPort: null, containerPort: 80, protocol: "tcp" }]));
  });

  it("no consulta Docker por imágenes que no existen localmente y lo explica", async () => {
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "alpine");
    expect(await screen.findByText(/La imagen alpine:latest no está en este equipo/)).toBeInTheDocument();
    expect(api.imageDefaults).not.toHaveBeenCalled();
  });

  it("oculta los valores de variables sensibles y permite mostrarlos", async () => {
    await openWizard();
    await userEvent.click(screen.getByRole("tab", { name: "Entorno" }));
    await userEvent.click(screen.getByRole("button", { name: "Añadir variable" }));
    await userEvent.type(screen.getByLabelText("Clave 1"), "DB_PASSWORD");
    const value = screen.getByLabelText("Valor 1");
    expect(value).toHaveAttribute("type", "password");
    await userEvent.click(screen.getByRole("button", { name: "Mostrar valor" }));
    expect(value).toHaveAttribute("type", "text");
  });

  it("marca los errores en su pestaña y lleva a ella desde la revisión", async () => {
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    await userEvent.click(screen.getByRole("tab", { name: "Red" }));
    await userEvent.click(screen.getByRole("button", { name: "Añadir puerto" }));
    await userEvent.click(screen.getByRole("tab", { name: "General" }));
    const redTab = await screen.findByRole("tab", { name: /Red\s*1/ });
    expect(redTab).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Red · Puerto no válido/ }));
    expect(screen.getByRole("tab", { name: /Red/ })).toHaveAttribute("aria-selected", "true");
  });

  it("elige la carpeta de un bind mount con el selector del sistema", async () => {
    await openWizard();
    await userEvent.click(screen.getByRole("tab", { name: "Almacenamiento" }));
    await userEvent.click(screen.getByRole("button", { name: "Añadir montaje" }));
    await userEvent.click(screen.getByRole("radio", { name: "Carpeta del equipo" }));
    await userEvent.click(screen.getByRole("button", { name: "Elegir carpeta" }));
    await waitFor(() => expect(lastSpec().mounts[0]).toMatchObject({ kind: "bind", source: "/home/u/datos" }));
  });

  it("crea e inicia, y abre el detalle del contenedor nuevo", async () => {
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    await userEvent.type(screen.getByLabelText("Nombre"), "web2");
    await userEvent.click(screen.getByRole("tab", { name: "Recursos" }));
    await userEvent.type(screen.getByLabelText("Límite de memoria"), "256");
    await waitFor(() => expect(screen.getByRole("button", { name: "Crear e iniciar" })).toBeEnabled());
    await userEvent.click(screen.getByRole("button", { name: "Crear e iniciar" }));
    expect(api.containerCreate).toHaveBeenCalledWith(expect.objectContaining({ name: "web2", image: "nginx:1.27", memoryBytes: 256 * 1024 ** 2 }), true);
    await waitFor(() => expect(useContainersNav.getState().page).toMatchObject({ kind: "detail", id: "f00dfeedf00dfeed" }));
  });

  it("si se creó pero no arrancó, lo dice en el detalle", async () => {
    api.containerCreate.mockResolvedValueOnce({ id: "f00d", name: "web2", warnings: ["Your kernel does not support swap limit"], state: null, startError: { code: "engine.docker", message: "port is already allocated", httpStatus: 500, issues: null } });
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    await waitFor(() => expect(screen.getByRole("button", { name: "Crear e iniciar" })).toBeEnabled());
    await userEvent.click(screen.getByRole("button", { name: "Crear e iniciar" }));
    expect(await screen.findByText("El contenedor se creó, pero no pudo iniciarse: port is already allocated")).toBeInTheDocument();
    expect(screen.getByText("Aviso de Docker: Your kernel does not support swap limit")).toBeInTheDocument();
  });

  it("muestra los problemas que devuelve el backend al crear", async () => {
    api.containerCreate.mockRejectedValueOnce({ code: "engine.invalidSpec", message: "x", httpStatus: null, issues: [{ severity: "error", field: "mounts.0.source", code: "bind.missing", params: { path: "/borrada" } }] });
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    await waitFor(() => expect(screen.getByRole("button", { name: "Crear" })).toBeEnabled());
    await userEvent.click(screen.getByRole("button", { name: "Crear" }));
    expect(api.containerCreate).toHaveBeenCalledWith(expect.anything(), false);
    expect(await screen.findAllByText("La carpeta /borrada no existe en este equipo.")).not.toHaveLength(0);
    expect(screen.getByRole("tab", { name: /Almacenamiento/ })).toHaveAttribute("aria-selected", "true");
  });

  it("los errores de Docker se muestran tal cual", async () => {
    api.containerCreate.mockRejectedValueOnce({ code: "engine.docker", message: "Conflict. The container name \"/web\" is already in use", httpStatus: 409, issues: null });
    await openWizard();
    await userEvent.type(screen.getByLabelText("Imagen"), "nginx:1.27");
    await waitFor(() => expect(screen.getByRole("button", { name: "Crear" })).toBeEnabled());
    await userEvent.click(screen.getByRole("button", { name: "Crear" }));
    expect(await screen.findByText(/No se pudo crear el contenedor: Conflict/)).toBeInTheDocument();
    expect(useContainersNav.getState().page.kind).toBe("create");
  });
});
