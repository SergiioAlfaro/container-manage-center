// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import { container, network, networks } from "@/test/apiMock";
import { connectBlock, connectCommand, disconnectCommand, matchesNetwork, pruneCandidates, splitAliases } from "./model";

describe("modelo de redes", () => {
  it("filtra por uso, por creadas por el usuario y por texto (nombre, subred, contenedor)", () => {
    const names = (f: Parameters<typeof matchesNetwork>[1], q = "") => networks.filter((n) => matchesNetwork(n, f, q)).map((n) => n.name);
    expect(names("inUse")).toEqual(["bridge", "backend", "vieja"]);
    expect(names("unused")).toEqual(["host", "none", "aislada"]);
    expect(names("custom")).toEqual(["backend", "vieja", "aislada"]);
    expect(names("all", "172.31")).toEqual(["vieja"]);
    expect(names("all", "CACHE")).toEqual(["bridge"]);
    expect(names("all", "n1n1")).toEqual(["backend"]);
  });

  it("la limpieza estimada incluye las redes que solo usan contenedores detenidos", () => {
    expect(pruneCandidates(networks).map((n) => n.name)).toEqual(["vieja", "aislada"]);
    // Un contenedor pausado cuenta como activo: Docker no elimina esa red.
    expect(pruneCandidates([network({ containers: [{ id: "x", name: "x", state: "paused", ipv4: null, ipv6: null, mac: null }] })])).toEqual([]);
    expect(pruneCandidates([network({ scope: "swarm", containers: [] })])).toEqual([]);
  });

  it("explica por qué Docker rechazaría conectar un contenedor", () => {
    const backend = networks[3]!;
    expect(connectBlock(backend, container({ id: "a1b2c3d4e5f6a7b8c9d0" }))).toBe("already");
    expect(connectBlock(backend, container({ id: "nuevo", networks: ["bridge"] }))).toBeNull();
    expect(connectBlock(backend, container({ id: "nuevo", networks: ["host"] }))).toBe("host");
    expect(connectBlock(backend, container({ id: "nuevo", networks: ["none"] }))).toBe("none");
    expect(connectBlock(networks[1]!, container({ id: "nuevo" }))).toBe("host");
    expect(connectBlock(networks[2]!, container({ id: "nuevo" }))).toBe("none");
    // Una red de usuario llamada "host" no es la predefinida.
    expect(connectBlock(network({ name: "host", driver: "bridge", containers: [] }), container({ id: "nuevo" }))).toBeNull();
  });

  it("separa alias y genera el comando equivalente con comillas", () => {
    expect(splitAliases(" web, api  db ")).toEqual(["web", "api", "db"]);
    expect(connectCommand("backend", "web", { aliases: ["web", "api", "web"], ipv4: " 172.30.0.50 ", ipv6: "" })).toBe(
      "docker network connect --alias web --alias api --ip 172.30.0.50 backend web",
    );
    expect(connectCommand("mi red", "web", { aliases: [], ipv4: "", ipv6: "fd00::5" })).toBe("docker network connect --ip6 fd00::5 'mi red' web");
    expect(disconnectCommand("backend", "web", true)).toBe("docker network disconnect --force backend web");
  });
});
