import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, componentsReport, connected, removalPlan } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useEngine } from "@/features/engine/store";
import { DEFAULT_CONFIG, useApp } from "@/app/store";
import { ComponentsSettings, formatKib } from "./ComponentsSettings";
import { SettingsView } from "./SettingsView";

async function analyze() {
  renderWithQuery(<ComponentsSettings />);
  // Nada se consulta hasta que el usuario lo pide.
  expect(api.componentsInspect).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Analizar componentes" }));
  await screen.findByText("Paquete de la aplicación");
}

describe("Configuración: desinstalación y componentes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEngine.setState({ state: connected, reconnecting: false });
    useApp.setState({ config: structuredClone(DEFAULT_CONFIG), saveError: null });
  });

  it("se abre desde Configuración y vuelve atrás", async () => {
    renderWithQuery(<SettingsView />);
    await userEvent.click(within(screen.getByText("Desinstalación y componentes").closest("div")!.parentElement!).getByRole("button", { name: /Abrir/ }));
    expect(screen.getByRole("heading", { name: "Desinstalación y componentes" })).toHaveFocus();
    await userEvent.click(screen.getByRole("button", { name: "Configuración" }));
    expect(screen.getByText("Apariencia")).toBeInTheDocument();
  });

  it("clasifica los paquetes: propio, quitables, compartidos con quién los usa, previos, manuales y del sistema", async () => {
    await analyze();
    const removable = screen.getByRole("heading", { name: "Dependencias que se pueden quitar (2)" }).closest("section")!;
    expect(within(removable).getByText("libwebkit2gtk-4.1-0")).toBeInTheDocument();
    expect(within(removable).getByText("libjavascriptcoregtk-4.1-0")).toBeInTheDocument();
    expect(within(removable).queryByText("libgtk-3-0")).not.toBeInTheDocument();
    const kept = screen.getByRole("heading", { name: "Se conservan" }).closest("section")!;
    expect(within(kept).getByText("Compartida con otros programas (1)")).toBeInTheDocument();
    expect(within(kept).getByText("La usan 2 paquetes: zenity, gnome-terminal")).toBeInTheDocument();
    expect(within(kept).getByText("Ya estaba instalada antes (1)")).toBeInTheDocument();
    expect(within(kept).getByText("Instalada de forma explícita (1)")).toBeInTheDocument();
    expect(within(kept).getByText("Componente del sistema (1)")).toBeInTheDocument();
    // Ninguno de los que se conservan tiene casilla para quitarlo.
    expect(within(kept).queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("Docker Engine, el CLI y Compose se muestran como herramientas externas que se conservan siempre", async () => {
    await analyze();
    const ext = screen.getByTestId("external-tools");
    expect(within(ext).getByText("docker.io 26.1.5+dfsg1-9")).toBeInTheDocument();
    expect(within(ext).getByText("docker-compose 2.26.1-4")).toBeInTheDocument();
    expect(within(ext).getAllByText("Se conserva")).toHaveLength(3);
    expect(within(ext).queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("los datos y la configuración se muestran como datos que no se borran", async () => {
    await analyze();
    const data = screen.getByRole("heading", { name: "Datos que no se borran" }).closest("section")!;
    expect(within(data).getByText("/home/u/.config/docker-control-center")).toBeInTheDocument();
    expect(within(data).getByText("no existe")).toBeInTheDocument();
    expect(within(data).getByText(/imágenes, contenedores, redes, volúmenes y proyectos Compose/)).toBeInTheDocument();
  });

  it("selección: solo la aplicación por omisión; las dependencias exigen la aplicación; el resumen calcula el espacio", async () => {
    await analyze();
    const summary = screen.getByTestId("components-summary");
    expect(summary).toHaveTextContent(`1 paquete · se liberan ${formatKib(25_000, "es")}`);
    await userEvent.click(screen.getByRole("button", { name: "Docker Control Center y las dependencias que se pueden quitar" }));
    expect(summary).toHaveTextContent(`3 paquetes · se liberan ${formatKib(145_000, "es")}`);
    // Quitar la aplicación quita también las dependencias elegidas.
    await userEvent.click(screen.getByRole("checkbox", { name: "Desinstalar Docker Control Center" }));
    expect(summary).toHaveTextContent("No hay nada seleccionado");
    expect(screen.getByRole("button", { name: "Revisar y desinstalar…" })).toBeDisabled();
    // Elegir una dependencia vuelve a marcar la aplicación.
    await userEvent.click(screen.getByRole("checkbox", { name: "Quitar libwebkit2gtk-4.1-0" }));
    expect(screen.getByRole("checkbox", { name: "Desinstalar Docker Control Center" })).toBeChecked();
    expect(summary).toHaveTextContent(`2 paquetes · se liberan ${formatKib(115_000, "es")}`);
  });

  it("revisar muestra la simulación y la orden exacta; cancelar no ejecuta nada", async () => {
    await analyze();
    await userEvent.click(screen.getByRole("button", { name: "Revisar y desinstalar…" }));
    expect(api.componentsPlan).toHaveBeenCalledWith(["docker-control-center"]);
    const dialog = await screen.findByRole("dialog", { name: "¿Desinstalar?" });
    expect(dialog).toHaveTextContent("/usr/bin/pkexec /usr/bin/apt-get remove -y --no-auto-remove docker-control-center");
    expect(dialog).toHaveTextContent("Remv docker-control-center");
    expect(dialog).toHaveTextContent("Se conservan Docker Engine, Docker CLI, Docker Compose");
    expect(dialog).toHaveTextContent("Se desinstala la propia aplicación");
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.componentsRemove).not.toHaveBeenCalled();
  });

  it("confirmar desinstala lo elegido y lo informa", async () => {
    await analyze();
    await userEvent.click(screen.getByRole("button", { name: "Docker Control Center y las dependencias que se pueden quitar" }));
    await userEvent.click(screen.getByRole("button", { name: "Revisar y desinstalar…" }));
    const dialog = await screen.findByRole("dialog");
    await act(() => userEvent.click(within(dialog).getByRole("button", { name: "Desinstalar" })));
    expect(api.componentsRemove).toHaveBeenCalledWith(["docker-control-center", "libjavascriptcoregtk-4.1-0", "libwebkit2gtk-4.1-0"]);
    expect(await screen.findByText("Se han desinstalado 3 paquetes.")).toBeInTheDocument();
    expect(screen.getByText(/Docker Control Center ya no está instalado/)).toBeInTheDocument();
  });

  it("si APT fuera a quitar algo más, no se puede confirmar", async () => {
    api.componentsPlan.mockResolvedValueOnce(removalPlan(["docker-control-center"], { extra: ["zenity"] }));
    await analyze();
    await userEvent.click(screen.getByRole("button", { name: "Revisar y desinstalar…" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("APT quitaría también: zenity");
    expect(within(dialog).getByRole("button", { name: "Desinstalar" })).toBeDisabled();
  });

  it("cancelar la autenticación del sistema se informa y no cambia nada", async () => {
    api.componentsRemove.mockRejectedValueOnce({ code: "packages.cancelled", message: "Se canceló la autenticación: no se ha desinstalado nada." });
    await analyze();
    await userEvent.click(screen.getByRole("button", { name: "Revisar y desinstalar…" }));
    const dialog = await screen.findByRole("dialog");
    await act(() => userEvent.click(within(dialog).getByRole("button", { name: "Desinstalar" })));
    expect(await screen.findByRole("alert")).toHaveTextContent("Se canceló la autenticación");
  });

  it("sin pkexec se muestra la orden para ejecutarla a mano y no se puede confirmar", async () => {
    api.componentsInspect.mockResolvedValueOnce(componentsReport({ elevation: null }));
    api.componentsPlan.mockResolvedValueOnce(removalPlan(["docker-control-center"], { command: "sudo apt-get remove --no-auto-remove docker-control-center" }));
    await analyze();
    await userEvent.click(screen.getByRole("button", { name: "Revisar y desinstalar…" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("pkexec no está instalado");
    expect(dialog).toHaveTextContent("sudo apt-get remove --no-auto-remove docker-control-center");
    expect(within(dialog).getByRole("button", { name: "Desinstalar" })).toBeDisabled();
  });

  it("sin historial de APT no ofrece quitar dependencias y explica por qué", async () => {
    const r = componentsReport({ history: "notFound" });
    r.packages = r.packages.map((p) => (p.class === "removable" || p.class === "shared" || p.class === "preExisting" ? { ...p, class: "uncertain" } : p));
    api.componentsInspect.mockResolvedValueOnce(r);
    await analyze();
    expect(screen.getByText(/El historial de APT no muestra cómo se instaló la aplicación/)).toBeInTheDocument();
    expect(screen.getByText("Origen incierto (4)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Docker Control Center y las dependencias que se pueden quitar" })).toBeDisabled();
  });

  it("sin paquete .deb no hay nada que desinstalar", async () => {
    api.componentsInspect.mockResolvedValueOnce(componentsReport({ own: null, packages: [], simulation: null, history: "notFound" }));
    renderWithQuery(<ComponentsSettings />);
    await userEvent.click(screen.getByRole("button", { name: "Analizar componentes" }));
    expect(await screen.findByText(/no está instalado como paquete .deb/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Revisar y desinstalar…" })).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByTestId("external-tools")).toBeInTheDocument());
  });
});
