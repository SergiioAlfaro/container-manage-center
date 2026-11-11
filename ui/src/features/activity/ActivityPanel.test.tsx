import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { apiRecord, cliRecord } from "@/test/fixtures";

vi.mock("@/shared/ipc/client", () => ({
  api: {
    activityList: vi.fn(async () => []),
    activityClear: vi.fn(async () => {}),
    copyText: vi.fn(async () => {}),
  },
  dockerApi: vi.fn(() => ({})),
  LOCAL_SERVER: "local",
  toCommandError: (e: unknown) => e,
}));

import { api } from "@/shared/ipc/client";
import { ActivityPanel } from "./ActivityPanel";
import { useActivity } from "./store";
import { EMPTY_FILTER } from "./logic";

function setRecords(records: ReturnType<typeof apiRecord>[]) {
  useActivity.setState({ records, filter: EMPTY_FILTER, loadError: null, max: 500 });
}

describe("ActivityPanel", () => {
  beforeEach(() => vi.clearAllMocks());

  it("muestra el estado vacío sin registros inventados", () => {
    setRecords([]);
    render(<ActivityPanel onClose={() => {}} />);
    expect(screen.getByText("Aún no hay operaciones")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Actividad" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Limpiar historial" })).toBeDisabled();
  });

  it("distingue operación API y comando equivalente", async () => {
    setRecords([apiRecord()]);
    render(<ActivityPanel onClose={() => {}} />);
    const item = screen.getByRole("list", { name: "Actividad" }).querySelector("li")!;
    expect(within(item).getByText("API")).toBeInTheDocument();
    expect(within(item).getByText("docker start web")).toBeInTheDocument();
    expect(within(item).getByText("Correcto")).toBeInTheDocument();

    await userEvent.click(within(item).getByRole("button", { expanded: false }));
    expect(within(item).getByText("Operación ejecutada")).toBeInTheDocument();
    expect(within(item).getByText("POST /v1.47/containers/web/start")).toBeInTheDocument();
    expect(within(item).getByText("Comando equivalente")).toBeInTheDocument();
    expect(within(item).getByText(/Traducción orientativa/)).toBeInTheDocument();
    expect(within(item).getByText("204")).toBeInTheDocument();
  });

  it("para CLI muestra el comando ejecutado y su salida, sin nota de equivalencia", async () => {
    setRecords([cliRecord()]);
    render(<ActivityPanel onClose={() => {}} />);
    const item = screen.getByRole("list", { name: "Actividad" }).querySelector("li")!;
    expect(within(item).getByText("CLI")).toBeInTheDocument();
    await userEvent.click(within(item).getByRole("button", { expanded: false }));
    expect(within(item).getByText("Comando ejecutado")).toBeInTheDocument();
    expect(within(item).queryByText("Comando equivalente")).not.toBeInTheDocument();
    expect(within(item).getByText("Container blog-db-1 Started")).toBeInTheDocument();
    expect(within(item).getByText("Salida estándar")).toBeInTheDocument();
  });

  it("los errores se muestran expandidos y con su mensaje", () => {
    setRecords([apiRecord({ status: "failed", error: "No such container: web", httpStatus: 404 })]);
    render(<ActivityPanel onClose={() => {}} />);
    expect(screen.getByText("No such container: web")).toBeInTheDocument();
    expect(screen.getByText("1 error")).toBeInTheDocument();
    expect(screen.getByRole("button", { expanded: true })).toBeInTheDocument();
  });

  it("filtra por estado y por texto, y permite quitar filtros", async () => {
    setRecords([
      apiRecord({ kind: "container.start" }),
      apiRecord({ kind: "image.pull", category: "image", equivalentCommand: "docker pull alpine:3", status: "failed", error: "x" }),
    ]);
    render(<ActivityPanel onClose={() => {}} />);
    const list = () => screen.getByRole("list", { name: "Actividad" });

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Estado" }), "failed");
    expect(within(list()).getAllByRole("listitem")).toHaveLength(1);
    expect(within(list()).getAllByText("docker pull alpine:3").length).toBeGreaterThan(0);

    await userEvent.type(screen.getByRole("searchbox"), "inexistente");
    expect(screen.getByText("Ninguna operación coincide con los filtros.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Quitar filtros" }));
    expect(within(list()).getAllByRole("listitem")).toHaveLength(2);
  });

  it("copia el comando al portapapeles", async () => {
    setRecords([apiRecord()]);
    render(<ActivityPanel onClose={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Copiar" }));
    expect(api.copyText).toHaveBeenCalledWith("docker start web");
    expect(await screen.findByRole("button", { name: "Copiado" })).toBeInTheDocument();
  });

  it("limpiar pide al backend vaciar el historial", async () => {
    setRecords([apiRecord()]);
    render(<ActivityPanel onClose={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Limpiar historial" }));
    expect(api.activityClear).toHaveBeenCalledOnce();
  });

  it("el evento 'cleared' conserva las operaciones en curso", () => {
    const running = apiRecord({ status: "running", finishedAt: null, durationMs: null });
    setRecords([running, apiRecord()]);
    useActivity.getState().apply({ type: "cleared" });
    expect(useActivity.getState().records.map((r) => r.id)).toEqual([running.id]);
  });

  it("cerrar invoca onClose", async () => {
    setRecords([]);
    const onClose = vi.fn();
    render(<ActivityPanel onClose={onClose} />);
    await userEvent.click(screen.getByRole("button", { name: "Cerrar panel de actividad" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
