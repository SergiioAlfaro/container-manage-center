// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, boundServers, connected, dockerApi, localServer, remoteServer, testReportOk, CHECK_IDS } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { useServers } from "@/app/server";
import type { ConnectionState } from "@/shared/ipc/bindings/ConnectionState";
import type { RemoteProblem } from "@/shared/ipc/bindings/RemoteProblem";
import { ServersView } from "./ServersView";
import { SecretDialog } from "./SecretDialog";
import { ServerSelector } from "./ServerSelector";
import { RemotePanel } from "./RemotePanel";
import { useServersNav } from "./nav";

const PROD = "srv-0123abcd";
const TEST = "srv-89abcdef";

function down(remote: RemoteProblem): ConnectionState {
  return {
    state: "disconnected",
    since: "2026-11-11T10:00:00Z",
    diagnosis: {
      problem: { kind: "remote", remote, steps: [] },
      candidates: [],
      installation: { cliPath: null, daemonPath: null, rootlessTools: false },
      user: { name: "sergio", uid: 1000, groups: [], groupsActive: [] },
      mode: { mode: "auto" },
    } as never,
  };
}

function setServers(list = [localServer(), remoteServer({ id: PROD, name: "Producción" }, connected), remoteServer({ id: TEST, name: "Pruebas", host: "10.0.0.7", port: 2222, user: "ops", auth: "password" })]) {
  api.serversList.mockResolvedValue(list);
  useServers.setState({ servers: list, states: Object.fromEntries(list.map((s) => [s.id, s.state])), loaded: true });
}

beforeEach(() => {
  vi.clearAllMocks();
  boundServers.length = 0;
  useServersNav.setState({ page: { kind: "list" }, tab: "servers" });
  setServerState(connected);
});

describe("Servidores: lista y gestión", () => {
  it("muestra cada servidor con dirección, método, estado y versión", () => {
    setServers();
    renderWithQuery(<ServersView />);
    const prod = screen.getByTestId(`server-row-${PROD}`);
    expect(within(prod).getByText("Producción")).toBeInTheDocument();
    expect(within(prod).getByText("deploy@docker-a.example.com")).toBeInTheDocument();
    expect(within(prod).getByText("Agente SSH")).toBeInTheDocument();
    expect(within(prod).getByText("Conectado")).toBeInTheDocument();
    expect(within(prod).getByText("29.9.2 · API 1.56")).toBeInTheDocument();
    const t = screen.getByTestId(`server-row-${TEST}`);
    expect(within(t).getByText("ops@10.0.0.7:2222")).toBeInTheDocument();
    expect(within(t).getByText("Contraseña")).toBeInTheDocument();
    expect(within(t).getByText("Desconectado")).toBeInTheDocument();
    expect(within(t).getByText("Nunca")).toBeInTheDocument();
    expect(within(screen.getByTestId("server-row-local")).getByText("Este equipo")).toBeInTheDocument();
  });

  it("conectar y desconectar actúan sobre ese servidor y ningún otro", async () => {
    setServers();
    renderWithQuery(<ServersView />);
    await userEvent.click(screen.getByRole("button", { name: "Conectar con Pruebas" }));
    expect(api.serverConnect).toHaveBeenCalledExactlyOnceWith(TEST, null, false);
    await userEvent.click(screen.getByRole("button", { name: "Desconectar de Producción" }));
    expect(api.serverDisconnect).toHaveBeenCalledExactlyOnceWith(PROD);
  });

  it("eliminar el perfil muestra nombre y dirección y deja claro que no se toca el servidor", async () => {
    setServers();
    renderWithQuery(<ServersView />);
    await userEvent.click(screen.getByRole("button", { name: "Más acciones de Pruebas" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Eliminar perfil" }));
    const dialog = screen.getByRole("dialog", { name: "¿Eliminar el perfil de Pruebas?" });
    expect(within(dialog).getByText("ops@10.0.0.7:2222")).toBeInTheDocument();
    expect(within(dialog).getByText(/No se elimina nada del servidor/)).toBeInTheDocument();
    expect(api.serverDelete).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Eliminar perfil" }));
    expect(api.serverDelete).toHaveBeenCalledExactlyOnceWith(TEST);
  });

  it("los estados se distinguen: autenticación, conexión, Docker e identidad", () => {
    const list = [
      localServer(),
      remoteServer({ id: "srv-aaaaaaaa", name: "A" }, down({ kind: "authFailed", methods: "publickey" })),
      remoteServer({ id: "srv-bbbbbbbb", name: "B" }, down({ kind: "sshRefused", port: 22 })),
      remoteServer({ id: "srv-cccccccc", name: "C" }, down({ kind: "dockerNotRunning", message: "x" })),
      remoteServer({ id: "srv-dddddddd", name: "D" }, down({ kind: "hostKeyChanged", presented: null, file: null, inUserFile: false })),
    ];
    setServers(list);
    renderWithQuery(<ServersView />);
    expect(within(screen.getByTestId("server-row-srv-aaaaaaaa")).getByText("Error de autenticación")).toBeInTheDocument();
    expect(within(screen.getByTestId("server-row-srv-bbbbbbbb")).getByText("Error de conexión")).toBeInTheDocument();
    expect(within(screen.getByTestId("server-row-srv-cccccccc")).getByText("Docker no disponible")).toBeInTheDocument();
    expect(within(screen.getByTestId("server-row-srv-dddddddd")).getByText("Identidad sin confirmar")).toBeInTheDocument();
  });

  it("la vista general muestra cada servidor por separado y permite abrirlo", async () => {
    setServers();
    api.serversOverview.mockResolvedValue([
      { server: localServer(), overview: (await api.engineOverview()) as never, error: null, updatedAt: "2026-11-11T10:00:00Z" },
      { server: remoteServer({ id: TEST, name: "Pruebas" }), overview: null, error: null, updatedAt: "2026-11-11T10:00:00Z" },
    ]);
    useServersNav.setState({ tab: "overview" });
    renderWithQuery(<ServersView />);
    const remote = await screen.findByTestId(`overview-${TEST}`);
    expect(within(remote).getByText("Sin conexión: no hay datos.")).toBeInTheDocument();
    expect(within(screen.getByTestId("overview-local")).getByText(/en ejecución/)).toBeInTheDocument();
    await userEvent.click(within(remote).getByRole("button", { name: "Abrir" }));
    expect(useServers.getState().selected).toBe(TEST);
  });
});

describe("Asistente «Añadir servidor»", () => {
  it("valida con el backend, prueba los siete pasos y guarda sin secretos", async () => {
    setServers([localServer()]);
    api.serverValidate.mockImplementation(async (c) => (c.host.includes(" ") ? [{ field: "host", code: "server.hostInvalid" }] : c.name ? [] : [{ field: "name", code: "server.nameRequired" }]));
    useServersNav.setState({ page: { kind: "edit", id: null } });
    renderWithQuery(<ServersView />);
    const user = userEvent.setup();
    await user.type(screen.getByRole("textbox", { name: /^Nombre/ }), "Producción");
    await user.type(screen.getByRole("textbox", { name: /^Host/ }), "mal host");
    expect(await screen.findByText(/Host no válido/)).toBeInTheDocument();
    await user.clear(screen.getByRole("textbox", { name: /^Host/ }));
    await user.type(screen.getByRole("textbox", { name: /^Host/ }), "docker-a.example.com");
    await user.type(screen.getByRole("textbox", { name: /^Usuario/ }), "deploy");
    await waitFor(() => expect(screen.queryByText(/Host no válido/)).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: /Autenticación/ }));
    await user.click(screen.getByRole("radio", { name: "Contraseña" }));
    expect(screen.getAllByText(/desaconsejada/).length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /Probar conexión/ }));
    await user.type(screen.getByLabelText(/Contraseña SSH/), "Secreto-UI-1");
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Probar conexión" }).at(-1)).toBeEnabled());
    await user.click(screen.getAllByRole("button", { name: "Probar conexión" }).at(-1)!);
    const call = api.serverTest.mock.calls.at(-1)!;
    expect(call[1]).toBe("Secreto-UI-1");
    for (const id of CHECK_IDS) expect(await screen.findByTestId(`check-${id}`)).toHaveAttribute("data-status", "ok");
    expect(screen.getByRole("status")).toHaveTextContent("Docker 29.0.1");

    await user.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(api.serverSave).toHaveBeenCalledOnce());
    const saved = api.serverSave.mock.calls[0]![0];
    expect(saved).toMatchObject({ name: "Producción", host: "docker-a.example.com", user: "deploy", auth: "password", port: 22 });
    expect(JSON.stringify(saved)).not.toContain("Secreto-UI-1");
  });

  it("primera conexión: muestra la huella y solo confía al confirmarla; luego vuelve a probar", async () => {
    setServers([localServer()]);
    const presented = { keyType: "ssh-ed25519", fingerprint: "SHA256:Nueva0123456789abcdefghijklmnopqrstuvwxyzAB" };
    api.serverTest.mockResolvedValueOnce({ ...testReportOk(), problem: { kind: "hostKeyUnknown", presented }, engine: null, steps: CHECK_IDS.map((id, i) => ({ id, status: i < 2 ? "ok" : i === 2 ? "failed" : "notRun", detail: null })) });
    useServersNav.setState({ page: { kind: "edit", id: PROD } });
    setServers([localServer(), remoteServer({ id: PROD, name: "Producción" })]);
    renderWithQuery(<ServersView />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Probar conexión/ }));
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Probar conexión" }).at(-1)).toBeEnabled());
    await user.click(screen.getAllByRole("button", { name: "Probar conexión" }).at(-1)!);
    const review = await screen.findByTestId("identity-review");
    expect(within(review).getByTestId("fingerprint")).toHaveTextContent(presented.fingerprint);
    expect(api.serverTrust).not.toHaveBeenCalled();
    await user.click(within(review).getByRole("button", { name: "La huella coincide: confiar" }));
    expect(api.serverTrust).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ id: PROD }), presented, false, false);
    await waitFor(() => expect(api.serverTest).toHaveBeenCalledTimes(2));
  });
});

describe("Servidor intermedio (E1, D-087)", () => {
  it("la huella del intermedio se confirma aparte y se guarda como la del intermedio", async () => {
    setServers([localServer()]);
    const presented = { keyType: "ssh-ed25519", fingerprint: "SHA256:Salto0123456789abcdefghijklmnopqrstuvwxyzABC" };
    api.serverTest.mockResolvedValueOnce({ ...testReportOk(), problem: { kind: "jumpHostKeyUnknown", jump: "ops@bastion:2222", presented }, engine: null, steps: CHECK_IDS.map((id, i) => ({ id, status: i < 2 ? "skipped" : i === 2 ? "failed" : "notRun", detail: null })) });
    useServersNav.setState({ page: { kind: "edit", id: PROD } });
    setServers([localServer(), remoteServer({ id: PROD, name: "Producción", proxyJump: "ops@bastion:2222" })]);
    renderWithQuery(<ServersView />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Probar conexión/ }));
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Probar conexión" }).at(-1)).toBeEnabled());
    await user.click(screen.getAllByRole("button", { name: "Probar conexión" }).at(-1)!);
    const review = await screen.findByTestId("identity-review");
    expect(within(review).getByTestId("identity-jump")).toHaveTextContent("Servidor intermedio: ops@bastion:2222");
    expect(within(review).getByTestId("fingerprint")).toHaveTextContent(presented.fingerprint);
    await user.click(within(review).getByRole("button", { name: "La huella coincide: confiar" }));
    expect(api.serverTrust).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ id: PROD }), presented, false, true);
  });
});

describe("Identidad cambiada", () => {
  it("avisa del posible ataque y exige confirmar antes de sustituirla", async () => {
    const presented = { keyType: "ssh-ed25519", fingerprint: "SHA256:Cambiada0123456789abcdefghijklmnopqrstuvwx" };
    setServers([localServer(), remoteServer({ id: PROD, name: "Producción" }, down({ kind: "hostKeyChanged", presented, file: "/x/known_hosts", inUserFile: false }))]);
    renderWithQuery(<RemotePanel />, PROD);
    const review = screen.getByTestId("identity-review");
    expect(within(review).getByText("La identidad del servidor ha cambiado")).toBeInTheDocument();
    expect(within(review).getByText(/ataque de intermediario/)).toBeInTheDocument();
    const replace = within(review).getByRole("button", { name: "Sustituir la identidad guardada" });
    expect(replace).toBeDisabled();
    await userEvent.click(within(review).getByRole("checkbox"));
    await userEvent.click(replace);
    expect(api.serverTrust).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ id: PROD }), presented, true, false);
  });

  it("si la clave anterior está en ~/.ssh/known_hosts no la toca: muestra la orden", () => {
    setServers([localServer(), remoteServer({ id: PROD, name: "Producción", port: 2222 }, down({ kind: "hostKeyChanged", presented: { keyType: "ssh-ed25519", fingerprint: "SHA256:x" }, file: "/home/u/.ssh/known_hosts", inUserFile: true }))]);
    renderWithQuery(<RemotePanel />, PROD);
    const review = screen.getByTestId("identity-review");
    expect(within(review).queryByRole("button", { name: "Sustituir la identidad guardada" })).not.toBeInTheDocument();
    expect(within(review).getByText("ssh-keygen -R '[docker-a.example.com]:2222'")).toBeInTheDocument();
  });
});

describe("Frase de paso y contraseña", () => {
  it("se envía solo al conectar; recordar usa el keyring", async () => {
    setServers();
    useServers.setState({ secretPrompt: { server: TEST, kind: "password", retry: false } });
    renderWithQuery(<SecretDialog />);
    const dialog = screen.getByRole("dialog", { name: "Contraseña para Pruebas" });
    expect(within(dialog).getByText("ops@10.0.0.7:2222")).toBeInTheDocument();
    await userEvent.type(within(dialog).getByLabelText("Contraseña SSH"), "Clave-123");
    await waitFor(() => expect(within(dialog).getByRole("checkbox")).toBeEnabled());
    await userEvent.click(within(dialog).getByRole("checkbox"));
    await userEvent.click(within(dialog).getByRole("button", { name: "Conectar" }));
    expect(api.serverConnect).toHaveBeenCalledExactlyOnceWith(TEST, "Clave-123", true);
  });

  it("sin almacén seguro no se ofrece recordar y se explica", async () => {
    setServers();
    api.keyringStatus.mockResolvedValue({ state: "unavailable", reason: "sin bus de sesión" });
    useServers.setState({ secretPrompt: { server: TEST, kind: "password", retry: true } });
    renderWithQuery(<SecretDialog />);
    expect(await screen.findByText(/No hay un almacén seguro disponible/)).toBeInTheDocument();
    expect(screen.getByRole("checkbox")).toBeDisabled();
    expect(screen.getByText("La contraseña no es correcta.")).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Contraseña SSH"), "x");
    await userEvent.click(screen.getByRole("button", { name: "Conectar" }));
    expect(api.serverConnect).toHaveBeenCalledExactlyOnceWith(TEST, "x", false);
  });
});

describe("Selector de servidor", () => {
  it("muestra el servidor actual con su estado y marca los remotos", async () => {
    setServers();
    useServers.setState({ selected: PROD });
    renderWithQuery(<ServerSelector />, PROD);
    const button = screen.getByRole("button", { name: /Servidor: Producción/ });
    expect(within(button).getByText("Remoto")).toBeInTheDocument();
    await userEvent.click(button);
    await userEvent.click(screen.getByRole("menuitem", { name: /^Pruebas — Desconectado/ }));
    expect(useServers.getState().selected).toBe(TEST);
  });

  it("un servidor remoto sin conexión no muestra datos de otro: explica y ofrece conectar", async () => {
    setServers();
    renderWithQuery(<RemotePanel />, TEST);
    expect(screen.getByText("Pruebas")).toBeInTheDocument();
    expect(screen.getByText(/no está conectado/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Conectar" }));
    expect(api.serverConnect).toHaveBeenCalledExactlyOnceWith(TEST, null, false);
  });
});

describe("Aislamiento en la interfaz", () => {
  it("cada ámbito crea su cliente con su servidor", async () => {
    setServers();
    const { DashboardView } = await import("@/features/dashboard/DashboardView");
    setServerState(connected, PROD);
    renderWithQuery(<DashboardView />, PROD);
    await screen.findByText("29.9.2");
    expect(dockerApi).toHaveBeenCalledWith(PROD);
    expect(boundServers).not.toContain("local");
  });

  it("dos ámbitos a la vez: cada uno con su servidor y su caché", async () => {
    setServers();
    const { ServerScope, queryClientFor } = await import("@/app/server");
    const { useApi } = await import("@/app/server");
    const { render } = await import("@testing-library/react");
    const seen: string[] = [];
    function Probe() {
      seen.push(useApi().server);
      return null;
    }
    render(
      <>
        <ServerScope server={PROD}>
          <Probe />
        </ServerScope>
        <ServerScope server={TEST}>
          <Probe />
        </ServerScope>
      </>,
    );
    expect(new Set(seen)).toEqual(new Set([PROD, TEST]));
    expect(queryClientFor(PROD)).not.toBe(queryClientFor(TEST));
    expect(queryClientFor(PROD)).toBe(queryClientFor(PROD));
  });
});

describe("Confirmaciones con el servidor de destino (§7)", () => {
  it("eliminar un volumen en un servidor remoto dice en qué servidor y dirección se hace", async () => {
    setServers();
    setServerState(connected, PROD);
    const { VolumesView } = await import("@/features/volumes/VolumesView");
    const user = userEvent.setup();
    renderWithQuery(<VolumesView />, PROD);
    await user.click(await screen.findByRole("button", { name: "Eliminar huerfano" }));
    const dialog = screen.getByRole("dialog");
    const target = within(dialog).getByTestId("dialog-server");
    expect(target).toHaveTextContent("Servidor: Producción");
    expect(target).toHaveTextContent("deploy@docker-a.example.com");
    await user.click(await within(dialog).findByRole("checkbox", { name: "Entiendo que se borrarán sus datos" }));
    await user.click(within(dialog).getByRole("button", { name: "Eliminar volumen" }));
    await waitFor(() => expect(api.volumeRemove).toHaveBeenCalledWith("huerfano"));
    // La orden va al cliente de ese servidor, y a ningún otro.
    expect(dockerApi).toHaveBeenCalledWith(PROD);
    expect(boundServers.every((s) => s === PROD)).toBe(true);
  });

  it("con un único equipo no se añade nada; y los diálogos de la aplicación no lo muestran", async () => {
    setServers([localServer()]);
    const { Dialog } = await import("@/shared/ui/feedback");
    const first = renderWithQuery(
      <Dialog title="x" onClose={() => {}} footer={null}>
        cuerpo
      </Dialog>,
    );
    expect(screen.queryByTestId("dialog-server")).not.toBeInTheDocument();
    first.unmount();
    setServers();
    renderWithQuery(
      <Dialog title="y" target={false} onClose={() => {}} footer={null}>
        cuerpo
      </Dialog>,
    );
    expect(screen.queryByTestId("dialog-server")).not.toBeInTheDocument();
  });
});

describe("Archivos del servidor remoto (§5)", () => {
  it("añadir un proyecto Compose usa una ruta del servidor, nunca el selector de este equipo", async () => {
    setServers();
    setServerState(connected, PROD);
    const { AddProjectDialog } = await import("@/features/compose/dialogs");
    const user = userEvent.setup();
    renderWithQuery(<AddProjectDialog onClose={() => {}} onAdded={() => {}} />, PROD);
    expect(screen.queryByRole("button", { name: /Elegir archivos|Añadir archivos/ })).not.toBeInTheDocument();
    const input = screen.getByRole("textbox", { name: "Archivo Compose en el servidor" });
    await user.type(input, "relativo.yaml{Enter}");
    expect(screen.getByText(/ruta absoluta del servidor/)).toBeInTheDocument();
    await user.clear(input);
    await user.type(input, "/srv/tienda/compose.yaml{Enter}");
    await user.click(screen.getByRole("button", { name: "Añadir" }));
    await waitFor(() => expect(api.composeRegister).toHaveBeenCalledWith(["/srv/tienda/compose.yaml"], null));
    expect(api.pickComposeFiles).not.toHaveBeenCalled();
    expect(boundServers.every((s) => s === PROD)).toBe(true);
  });

  it("guardar un archivo en el servidor confirma antes el destino", async () => {
    setServers();
    setServerState(connected, PROD);
    const { FileEditor } = await import("@/features/compose/FileEditor");
    const user = userEvent.setup();
    const project = { name: "tienda", files: ["/srv/tienda/compose.yaml"], workingDir: null, envFiles: [] };
    renderWithQuery(<FileEditor project={project} onSaved={() => {}} />, PROD);
    const area = await screen.findByRole("textbox");
    await user.type(area, "#");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    const dialog = await screen.findByRole("dialog", { name: "¿Guardar el archivo en el servidor?" });
    expect(within(dialog).getByTestId("write-destination")).toHaveTextContent("/srv/tienda/compose.yaml");
    expect(within(dialog).getByTestId("dialog-server")).toHaveTextContent("Producción");
    expect(api.composeFileSave).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Guardar en el servidor" }));
    await waitFor(() => expect(api.composeFileSave).toHaveBeenCalledOnce());
    expect(api.composeFileSave.mock.calls[0]![0]).toBe("/srv/tienda/compose.yaml");
  });

  it("en el equipo local el guardado no pide esa confirmación", async () => {
    setServers();
    const { FileEditor } = await import("@/features/compose/FileEditor");
    const user = userEvent.setup();
    const project = { name: "tienda", files: ["/home/u/tienda/compose.yaml"], workingDir: null, envFiles: [] };
    renderWithQuery(<FileEditor project={project} onSaved={() => {}} />);
    const area = await screen.findByRole("textbox");
    await user.type(area, "#");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(api.composeFileSave).toHaveBeenCalledOnce());
    expect(screen.queryByRole("dialog", { name: "¿Guardar el archivo en el servidor?" })).not.toBeInTheDocument();
  });

  it("vincular una carpeta en un servidor remoto pide la ruta del servidor", async () => {
    setServers();
    setServerState(connected, PROD);
    const { BindsView } = await import("@/features/binds/BindsView");
    const user = userEvent.setup();
    renderWithQuery(<BindsView />, PROD);
    await user.click(await screen.findByRole("button", { name: "Vincular una carpeta…" }));
    expect(api.pickFolder).not.toHaveBeenCalled();
    expect(screen.getByRole("textbox", { name: "Carpeta del servidor" })).toBeInTheDocument();
  });
});

describe("Terminal SSH del servidor (§6)", () => {
  it("es un tipo de sesión aparte, solo en servidores remotos, y va a ese servidor", async () => {
    setServers();
    setServerState(connected, PROD);
    const { NewTerminalDialog } = await import("@/features/terminals/NewTerminalDialog");
    const { useTerminals, closeAllSessions } = await import("@/features/terminals/sessions");
    const user = userEvent.setup();
    renderWithQuery(<NewTerminalDialog onClose={() => {}} />, PROD);
    await user.click(screen.getByRole("radio", { name: "Servidor (shell SSH)" }));
    expect(screen.getByText(/se ejecuta en el servidor, no dentro de un contenedor/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Contenedor")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Abrir terminal" }));
    await waitFor(() => expect(api.serverShellOpen).toHaveBeenCalledOnce());
    expect(api.terminalOpen).not.toHaveBeenCalled();
    expect(dockerApi).toHaveBeenLastCalledWith(PROD);
    const s = useTerminals.getState().sessions.at(-1)!;
    expect(s).toMatchObject({ kind: "server", server: PROD, serverName: "Producción", remote: true, name: "SSH Producción" });
    closeAllSessions();
  });

  it("en el equipo local no se ofrece", async () => {
    setServers();
    const { NewTerminalDialog } = await import("@/features/terminals/NewTerminalDialog");
    renderWithQuery(<NewTerminalDialog onClose={() => {}} />);
    expect(screen.queryByRole("radio", { name: "Servidor (shell SSH)" })).not.toBeInTheDocument();
  });
});

describe("Textos del equipo de Docker en un servidor remoto", () => {
  it("en remoto hablan del servidor; en local no cambian", async () => {
    setServers();
    setServerState(connected, PROD);
    const { BindsView } = await import("@/features/binds/BindsView");
    const { useIssueText } = await import("@/features/containers/create/fields");
    const issue = { severity: "error" as const, field: "mounts.0.source", code: "bind.missing", params: { path: "/srv/x" } };
    let text = "";
    function Probe() {
      text = useIssueText()(issue);
      return null;
    }
    const r = renderWithQuery(
      <>
        <BindsView />
        <Probe />
      </>,
      PROD,
    );
    expect(await screen.findByText(/^Carpetas del servidor montadas/)).toBeInTheDocument();
    expect(text).toBe("La carpeta /srv/x no existe en el servidor.");
    r.unmount();
    renderWithQuery(
      <>
        <BindsView />
        <Probe />
      </>,
    );
    expect(await screen.findByText(/^Carpetas de este equipo montadas/)).toBeInTheDocument();
    expect(text).toBe("La carpeta /srv/x no existe en este equipo.");
  });
});

describe("Códigos de validación del motor", () => {
  it("se traducen con su prefijo «server.» en el asistente y en el problema de perfil", async () => {
    // Los códigos reales llevan prefijo (ProfileIssue.code = «server.keyPathInvalid»).
    setServers([localServer()]);
    api.serverValidate.mockImplementation(async (c) => (c.auth === "key" && !c.keyPath ? [{ field: "keyPath", code: "server.keyPathInvalid" }] : []));
    useServersNav.setState({ page: { kind: "edit", id: null } });
    renderWithQuery(<ServersView />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Autenticación/ }));
    await user.click(screen.getByRole("radio", { name: "Clave privada" }));
    // Sin tocar el campo no se muestra todavía como error.
    expect(screen.queryByText("Indica la ruta de la clave privada.")).not.toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: /Ruta de la clave privada/ }), "x");
    await user.clear(screen.getByRole("textbox", { name: /Ruta de la clave privada/ }));
    expect(await screen.findByText("Indica la ruta de la clave privada.")).toBeInTheDocument();
    expect(screen.queryByText(/servers\.issues/)).not.toBeInTheDocument();
  });

  it("un perfil no válido muestra los problemas traducidos, no los códigos", () => {
    setServers([localServer(), remoteServer({ id: PROD, name: "Producción" }, down({ kind: "invalidProfile", codes: ["server.hostInvalid"] }))]);
    renderWithQuery(<RemotePanel />, PROD);
    const box = screen.getByTestId("remote-problem");
    expect(within(box).getByText(/Host no válido/)).toBeInTheDocument();
    expect(within(box).queryByText(/server\.hostInvalid/)).not.toBeInTheDocument();
  });
});
