import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import i18n from "@/shared/i18n";
import { api, binds, connected } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { useEngine } from "@/features/engine/store";
import { BindsView, matchesBind, needsAttention } from "./BindsView";

function reset() {
  vi.clearAllMocks();
  useEngine.setState({ state: connected });
  useApp.setState({ section: "binds" });
}

/** Tarjeta de una carpeta (la ruta aparece primero como origen). */
const card = (path: string) => screen.getAllByText(path)[0]!.closest("li")!;

describe("utilidades de vinculaciones", () => {
  it("filtra por escritura, atención y texto", () => {
    expect(binds.filter(needsAttention).map((b) => b.source)).toEqual(["/var/run/docker.sock", "/home/u/borrado"]);
    expect(binds.filter((b) => matchesBind(b, "writable", "")).map((b) => b.source)).toEqual(["/srv/web", "/var/run/docker.sock"]);
    expect(binds.filter((b) => matchesBind(b, "all", "nginx")).map((b) => b.source)).toEqual(["/srv/web"]);
  });
});

describe("Directorios vinculados", () => {
  beforeEach(reset);

  it("agrupa por carpeta con su estado real, contenedores, acceso y riesgos", async () => {
    renderWithQuery(<BindsView />);
    expect(await screen.findByText("/srv/web")).toBeInTheDocument();
    const web = card("/srv/web");
    expect(within(web).getByText("Carpeta")).toBeInTheDocument();
    expect(within(web).getByText("rwxr-xr-x · uid 1000")).toBeInTheDocument();
    expect(within(web).getByText("/usr/share/nginx/html")).toBeInTheDocument();
    expect(within(web).getAllByText("Solo lectura")).toHaveLength(1);
    expect(within(web).getByText("Lectura y escritura")).toBeInTheDocument();
    const sock = card("/var/run/docker.sock");
    expect(within(sock).getByText(/el contenedor puede controlar Docker y, en la práctica, todo el equipo/)).toBeInTheDocument();
    const gone = card("/home/u/borrado");
    expect(within(gone).getByText("No existe")).toBeInTheDocument();
    expect(within(gone).getByText(/Docker volverá a crearla vacía \(como root\)/)).toBeInTheDocument();
    expect(screen.getByText(/2 vinculaciones requieren atención/)).toBeInTheDocument();
    // Solo las carpetas existentes ofrecen crear un contenedor con ellas.
    expect(within(web).getByRole("button", { name: "Nuevo contenedor con ella" })).toBeInTheDocument();
    expect(within(gone).queryByRole("button", { name: "Nuevo contenedor con ella" })).not.toBeInTheDocument();
  });

  it("filtra las que requieren atención", async () => {
    const user = userEvent.setup();
    renderWithQuery(<BindsView />);
    await screen.findByText("/srv/web");
    await user.click(screen.getByRole("radio", { name: "Requieren atención (2)" }));
    expect(screen.queryByText("/srv/web")).not.toBeInTheDocument();
    expect(screen.getByText("2 carpetas")).toBeInTheDocument();
  });

  it("vincular una carpeta usa el selector del sistema y abre el asistente con el montaje", async () => {
    const user = userEvent.setup();
    renderWithQuery(<BindsView />);
    await user.click(await screen.findByRole("button", { name: "Vincular una carpeta…" }));
    await waitFor(() => expect(api.pickFolder).toHaveBeenCalledWith("Carpeta que verá el contenedor"));
    expect(useApp.getState().section).toBe("containers");
    const page = useContainersNav.getState().page;
    expect(page.kind === "create" && page.spec?.mounts).toEqual([{ kind: "bind", source: "/home/u/datos", target: "/mnt/datos", readOnly: false }]);
  });

  it("un contenedor abre su detalle", async () => {
    const user = userEvent.setup();
    renderWithQuery(<BindsView />);
    await screen.findByText("/srv/web");
    await user.click(within(card("/srv/web")).getByRole("button", { name: "web" }));
    expect(useApp.getState().section).toBe("containers");
    expect(useContainersNav.getState().page).toMatchObject({ kind: "detail", id: "a1b2c3d4e5f6a7b8c9d0" });
  });

  it("sin vinculaciones explica cómo crear una; en inglés aparece en inglés", async () => {
    api.bindsList.mockResolvedValueOnce([]);
    renderWithQuery(<BindsView />);
    expect(await screen.findByText("Ningún contenedor tiene carpetas vinculadas")).toBeInTheDocument();
    await i18n.changeLanguage("en");
    try {
      api.bindsList.mockResolvedValueOnce(binds);
      renderWithQuery(<BindsView />);
      expect(await screen.findAllByText("Bind a folder…")).not.toHaveLength(0);
    } finally {
      await i18n.changeLanguage("es");
    }
  });
});
