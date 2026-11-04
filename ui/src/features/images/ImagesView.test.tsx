import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, connected } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import { useEngine } from "@/features/engine/store";
import { ImagesView } from "./ImagesView";

describe("Imágenes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEngine.setState({ state: connected });
  });

  it("lista imágenes con etiquetas, tamaño, ID corto y uso real", async () => {
    renderWithQuery(<ImagesView />);
    expect(await screen.findByText("nginx:1.27")).toBeInTheDocument();
    expect(screen.getByText("nginx:latest")).toBeInTheDocument();
    const nginx = screen.getByText("nginx:1.27").closest("tr")!;
    expect(within(nginx).getByText("1111aaaabbbb")).toBeInTheDocument();
    expect(within(nginx).getByText("183 MiB")).toBeInTheDocument();
    expect(within(nginx).getByText("1 contenedor")).toBeInTheDocument();
    const table = screen.getByRole("table");
    expect(within(table).getByText("Sin etiqueta")).toBeInTheDocument();
    expect(within(table).getByText("Sin uso")).toBeInTheDocument();
  });

  it("no ofrece acciones que aún no están implementadas", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    const table = screen.getByRole("table");
    expect(within(table).queryAllByRole("button")).toHaveLength(0);
  });

  it("filtra por uso y busca por ID", async () => {
    renderWithQuery(<ImagesView />);
    await screen.findByText("nginx:1.27");
    await userEvent.click(screen.getByRole("radio", { name: "Sin uso" }));
    expect(screen.queryByText("nginx:1.27")).not.toBeInTheDocument();
    expect(screen.getByText("1 imagen")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "Todas" }));
    await userEvent.type(screen.getByRole("searchbox"), "9999dd");
    expect(screen.getByText("1 imagen")).toBeInTheDocument();
    expect(api.imagesList).toHaveBeenCalledTimes(1);
  });
});
