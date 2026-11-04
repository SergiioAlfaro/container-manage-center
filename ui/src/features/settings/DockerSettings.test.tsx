import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected, diagnosis } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useEngine } from "@/features/engine/store";
import { DEFAULT_CONFIG, useApp } from "@/app/store";
import { DockerSettings } from "./DockerSettings";

describe("Configuración: conexión con Docker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEngine.setState({ state: connected, reconnecting: false });
    useApp.setState({ config: structuredClone(DEFAULT_CONFIG), saveError: null });
  });

  it("muestra la conexión actual real", () => {
    renderWithQuery(<DockerSettings />);
    expect(screen.getByText("/var/run/docker.sock")).toBeInTheDocument();
    expect(screen.getByText("1.56")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Automático" })).toHaveAttribute("aria-checked", "true");
  });

  it("socket personalizado: exige ruta, prueba sin guardar y guarda al confirmar", async () => {
    renderWithQuery(<DockerSettings />);
    await userEvent.click(screen.getByRole("radio", { name: "Socket personalizado" }));
    expect(screen.getByText("Indica la ruta del socket.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Probar conexión" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Guardar y conectar" })).toBeDisabled();

    await userEvent.type(screen.getByLabelText("Ruta del socket"), "  /run/user/1000/docker.sock ");
    await userEvent.click(screen.getByRole("button", { name: "Probar conexión" }));
    expect(api.engineTest).toHaveBeenCalledWith({ mode: "custom", customSocket: "/run/user/1000/docker.sock" });
    expect(await screen.findByRole("status")).toHaveTextContent("Conexión correcta: Docker 29.9.2");
    expect(api.configSet).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Guardar y conectar" }));
    expect(api.configSet).toHaveBeenCalledWith(expect.objectContaining({ docker: { mode: "custom", customSocket: "/run/user/1000/docker.sock" } }));
  });

  it("una prueba fallida explica la causa", async () => {
    api.engineTest.mockResolvedValueOnce({ result: "failed", diagnosis: diagnosis({ kind: "daemonNotRunning", socketPath: "/tmp/d.sock" }) } as never);
    renderWithQuery(<DockerSettings />);
    await userEvent.click(screen.getByRole("button", { name: "Probar conexión" }));
    expect(await screen.findByRole("status")).toHaveTextContent("No se pudo conectar: El socket /tmp/d.sock existe, pero ningún proceso responde en él.");
  });

  it("Compose se consulta solo cuando se pide", async () => {
    renderWithQuery(<DockerSettings />);
    expect(screen.getByText("Sin comprobar")).toBeInTheDocument();
    expect(api.composeStatus).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Comprobar" }));
    expect(await screen.findByText("2.40.0 (plugin «docker compose»)")).toBeInTheDocument();
  });

  it("Compose no instalado: lo dice claramente y explica cómo instalarlo", async () => {
    api.composeStatus.mockResolvedValueOnce({ status: "notInstalled", cliPath: "/usr/bin/docker", searched: ["/usr/libexec/docker/cli-plugins/docker-compose"] });
    renderWithQuery(<DockerSettings />);
    await userEvent.click(screen.getByRole("button", { name: "Comprobar" }));
    expect(await screen.findByText("No instalado")).toBeInTheDocument();
    expect(screen.getByText("sudo apt install docker-compose")).toBeInTheDocument();
    expect(screen.getByText("sudo apt install docker-compose-v2")).toBeInTheDocument();
    expect(screen.getByText("sudo apt install docker-compose-plugin")).toBeInTheDocument();
  });

  it("Volver a conectar llama al backend", async () => {
    renderWithQuery(<DockerSettings />);
    await act(async () => userEvent.click(screen.getByRole("button", { name: "Volver a conectar" })));
    expect(api.engineReconnect).toHaveBeenCalledOnce();
  });
});
