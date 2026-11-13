import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api, streams } from "@/test/apiMock";
import { layerProgress, overallProgress, usePulls } from "./pulls";

const L = (status: string, current: number | null = null, total: number | null = null) => ({ id: "a1b2c3d4e5f6", status, current, total });

describe("progreso de descarga", () => {
  it("la descarga es la primera mitad y la extracción la segunda", () => {
    expect(layerProgress(L("Waiting"))).toBe(0);
    expect(layerProgress(L("Downloading", 50, 100))).toBe(0.25);
    expect(layerProgress(L("Download complete"))).toBe(0.5);
    expect(layerProgress(L("Extracting", 50, 100))).toBe(0.75);
    expect(layerProgress(L("Pull complete"))).toBe(1);
    expect(layerProgress(L("Already exists"))).toBe(1);
  });
  it("pondera por tamaño y reconoce el final del almacén containerd", () => {
    expect(overallProgress([])).toBeNull();
    expect(overallProgress([L("Pull complete", null, 300), L("Downloading", 0, 100)])).toBe(0.75);
    expect(overallProgress([L("Download complete")])).toBe(1);
  });
});

describe("descargas", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    streams.reset();
    usePulls.setState({ items: [] });
  });

  it("sigue el progreso, termina y avisa", async () => {
    const done = vi.fn();
    await usePulls.getState().start("local", "nginx:1.27", { mode: "dockerConfig" }, done);
    expect(api.imagePullOpen).toHaveBeenCalledWith("nginx:1.27", { mode: "dockerConfig" }, expect.any(Function), false);
    streams.emit(1, { event: "progress", update: { layers: [L("Downloading", 10, 100)], messages: ["1.27: Pulling from library/nginx"] } });
    let p = usePulls.getState().items[0]!;
    expect(p.status).toBe("running");
    expect(p.stream).toBe(1);
    expect(p.messages).toEqual(["1.27: Pulling from library/nginx"]);
    streams.emit(1, { event: "end", closed: { end: { reason: "finished", summary: "nginx:1.27: Downloaded newer image", exitCode: null }, error: null } });
    p = usePulls.getState().items[0]!;
    expect(p.status).toBe("done");
    expect(p.summary).toBe("nginx:1.27: Downloaded newer image");
    expect(done).toHaveBeenCalledWith(true);
  });

  it("una descarga sin capas (Docker ya tenía el contenido) termina bien con su resumen", async () => {
    const done = vi.fn();
    await usePulls.getState().start("local", "127.0.0.1:5000/app:1", { mode: "anonymous" }, done);
    // Almacén clásico: solo mensajes generales, ninguna capa.
    streams.emit(1, { event: "progress", update: { layers: [], messages: ["1: Pulling from app", "Digest: sha256:a974", "Status: Downloaded newer image for 127.0.0.1:5000/app:1"] } });
    let p = usePulls.getState().items[0]!;
    expect(p.layers).toEqual([]);
    expect(overallProgress(p.layers)).toBeNull();
    const summary = "127.0.0.1:5000/app:1: Downloaded newer image for 127.0.0.1:5000/app:1 (sha256:a974); Docker ya tenía todo su contenido, no ha descargado capas; sin credenciales";
    streams.emit(1, { event: "end", closed: { end: { reason: "finished", summary, exitCode: null }, error: null } });
    p = usePulls.getState().items[0]!;
    expect(p.status).toBe("done");
    expect(p.summary).toBe(summary);
    expect(p.messages).toHaveLength(3);
    expect(done).toHaveBeenCalledWith(true);
  });

  it("cancelar cierra el flujo y queda como cancelada", async () => {
    const key = await usePulls.getState().start("local", "nginx", { mode: "anonymous" });
    usePulls.getState().cancel(key);
    expect(api.streamClose).toHaveBeenCalledWith(1);
    expect(usePulls.getState().items[0]!.status).toBe("cancelled");
  });

  it("los errores de Docker se conservan tal cual", async () => {
    await usePulls.getState().start("local", "privada/app", { mode: "anonymous" });
    streams.emit(1, { event: "end", closed: { end: null, error: { code: "engine.docker", message: "pull access denied for privada/app", httpStatus: 404, issues: null } } });
    expect(usePulls.getState().items[0]).toMatchObject({ status: "failed", error: "pull access denied for privada/app" });
    usePulls.getState().clearFinished("local");
    expect(usePulls.getState().items).toHaveLength(0);
  });
});
