import { beforeEach, describe, expect, it, vi } from "vitest";

// Sustituye el puente de Tauri para ver exactamente qué recibe el backend.
const calls: { cmd: string; args: Record<string, unknown> | undefined }[] = [];
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    calls.push({ cmd, args });
    return null;
  }),
  Channel: class {
    onmessage: unknown = null;
  },
}));

const { api, dockerApi, LOCAL_SERVER } = await import("./client");

describe("órdenes con Docker ligadas a un servidor (0.10.0)", () => {
  beforeEach(() => {
    calls.length = 0;
  });

  it("cada llamada lleva el servidor con el que se creó el cliente, sin mezclarlos", async () => {
    const a = dockerApi("srv-aaaaaaaa");
    const b = dockerApi("srv-bbbbbbbb");
    await a.containersList();
    await b.containerRemove("web", "web", true, false);
    await a.volumeRemove("datos");
    await b.engineOverview();
    expect(calls.map((c) => [c.cmd, c.args?.server])).toEqual([
      ["containers_list", "srv-aaaaaaaa"],
      ["container_remove", "srv-bbbbbbbb"],
      ["volume_remove", "srv-aaaaaaaa"],
      ["engine_overview", "srv-bbbbbbbb"],
    ]);
    // Los argumentos propios no se pierden ni pueden sustituir al servidor.
    expect(calls[1]!.args).toMatchObject({ id: "web", force: true, volumes: false, confirmed: true });
  });

  it("todas las órdenes del cliente de servidor envían el servidor", async () => {
    const d = dockerApi("srv-cccccccc");
    const names = Object.keys(d) as (keyof typeof d)[];
    expect(names.length).toBe(74); // 1.4.0: + 7 órdenes de copias; 1.5.0: + 5 de monitorización
    for (const name of names) {
      calls.length = 0;
      const fn = d[name] as (...args: unknown[]) => Promise<unknown>;
      // Argumentos de relleno: solo interesa lo que llega al backend.
      await fn("x", "x", "x", "x", "x", "x", () => {}).catch(() => {});
      expect(calls.length, name).toBe(1);
      expect(calls[0]!.args?.server, name).toBe("srv-cccccccc");
    }
  });

  it("el cliente de la aplicación no tiene órdenes de Docker ni envía servidor", async () => {
    expect("containersList" in api).toBe(false);
    expect(Object.keys(dockerApi(LOCAL_SERVER)).every((k) => !(k in api))).toBe(true);
    await api.configGet();
    await api.serverConnect("srv-aaaaaaaa", null, false);
    expect(calls[0]!.cmd).toBe("config_get");
    expect(calls[0]!.args?.server).toBeUndefined();
    // Las órdenes de servidores llevan el suyo, explícito.
    expect(calls[1]).toMatchObject({ cmd: "server_connect", args: { server: "srv-aaaaaaaa" } });
  });
});
