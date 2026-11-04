import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, diagnosis, disconnected } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useEngine } from "@/features/engine/store";
import { useApp } from "@/app/store";
import { ConnectionBadge } from "@/features/engine/ConnectionBadge";
import { DashboardView } from "./DashboardView";

describe("Dashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useApp.setState({ section: "dashboard" });
  });

  it("con conexión muestra los datos del motor que devuelve el backend", async () => {
    useEngine.setState({ state: connected });
    renderWithQuery(<DashboardView />);
    expect(await screen.findByText("29.9.2")).toBeInTheDocument();
    expect(screen.getByText("/var/run/docker.sock")).toBeInTheDocument();
    expect(screen.getByText("Socket del sistema")).toBeInTheDocument();
    expect(screen.getByText("Debian GNU/Linux 13 (trixie)")).toBeInTheDocument();
    expect(screen.getByText("overlayfs")).toBeInTheDocument();
    expect(screen.getByText("WARNING: bridge-nf-call-iptables is disabled")).toBeInTheDocument();
    const containers = screen.getByRole("heading", { name: "Contenedores" }).closest("section")!;
    expect(within(containers).getByText("3")).toBeInTheDocument();
    expect(api.engineOverview).toHaveBeenCalledTimes(1);
  });

  it("Actualizar vuelve a pedir los datos a Docker", async () => {
    useEngine.setState({ state: connected });
    renderWithQuery(<DashboardView />);
    await userEvent.click(await screen.findByRole("button", { name: "Actualizar" }));
    expect(api.engineOverview).toHaveBeenCalledTimes(2);
  });

  it("sin conexión no pide datos y muestra el diagnóstico con instrucciones concretas", async () => {
    useEngine.setState({
      state: disconnected(diagnosis({ kind: "permissionDenied", permissions: { socketPath: "/var/run/docker.sock", owner: "root", group: "docker", mode: "660", userInGroup: false, groupActiveInSession: false } })),
    });
    renderWithQuery(<DashboardView />);
    expect(screen.getByRole("heading", { name: "Sin permiso para usar el socket de Docker" })).toBeInTheDocument();
    expect(screen.getByText(/root:docker con permisos 660/)).toBeInTheDocument();
    expect(screen.getByText("sudo usermod -aG docker sergio")).toBeInTheDocument();
    expect(screen.getByText(/equivale a tener privilegios de administrador/)).toBeInTheDocument();
    expect(screen.getByText("/run/user/1000/docker.sock")).toBeInTheDocument();
    expect(screen.getByText("Permiso denegado")).toBeInTheDocument();
    expect(api.engineOverview).not.toHaveBeenCalled();
  });

  it("Volver a conectar y Configurar conexión funcionan desde el diagnóstico", async () => {
    const down = disconnected(diagnosis({ kind: "daemonNotRunning", socketPath: "/var/run/docker.sock" }));
    useEngine.setState({ state: down });
    // El daemon sigue caído: el backend vuelve a informar del mismo problema.
    api.engineReconnect.mockResolvedValueOnce(down);
    renderWithQuery(<DashboardView />);
    expect(screen.getByText("sudo systemctl enable --now docker")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Volver a conectar" }));
    expect(api.engineReconnect).toHaveBeenCalledOnce();
    await userEvent.click(screen.getByRole("button", { name: "Configurar conexión" }));
    expect(useApp.getState().section).toBe("settings");
  });

  it("el error de una consulta se muestra con el mensaje de Docker", async () => {
    api.engineOverview.mockRejectedValueOnce({ code: "engine.docker", message: "Error response from daemon: boom", httpStatus: 500 });
    useEngine.setState({ state: connected });
    renderWithQuery(<DashboardView />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Error response from daemon: boom");
  });
});

describe("ConnectionBadge", () => {
  it("refleja conectado, desconectado y conectando", () => {
    const onOpen = vi.fn();
    const c = connected as Extract<typeof connected, { state: "connected" }>;
    useEngine.setState({ state: { ...c, engine: { ...c.engine, rootless: true } } });
    const { rerender } = renderWithQuery(<ConnectionBadge onOpen={onOpen} />);
    expect(screen.getByRole("button", { name: "Conectado a /var/run/docker.sock" })).toHaveTextContent("Docker 29.9.2rootless");
    useEngine.setState({ state: disconnected(diagnosis({ kind: "noSocket" })) });
    rerender(<ConnectionBadge onOpen={onOpen} />);
    expect(screen.getByRole("button")).toHaveTextContent("Sin conexión con Docker");
    useEngine.setState({ state: { state: "connecting" } });
    rerender(<ConnectionBadge onOpen={onOpen} />);
    expect(screen.getByRole("button")).toHaveTextContent("Conectando con Docker…");
  });
});
