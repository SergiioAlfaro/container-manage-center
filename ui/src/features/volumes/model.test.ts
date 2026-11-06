import { describe, expect, it } from "vitest";
import { volume, volumes } from "@/test/apiMock";
import { bindMount, legacyPrune, matchesVolume, modeString, pruneCandidates, volumeMount } from "./model";

describe("modelo de volúmenes", () => {
  it("filtra por uso, anónimos y texto (nombre, contenedor, metadato)", () => {
    const names = (f: Parameters<typeof matchesVolume>[1], q = "") => volumes.filter((v) => matchesVolume(v, f, q)).map((v) => v.name.slice(0, 8));
    expect(names("inUse")).toEqual(["datos", "copias"]);
    expect(names("unused")).toEqual(["huerfano", "3f1c9e2b"]);
    expect(names("anonymous")).toEqual(["3f1c9e2b"]);
    expect(names("all", "WEB")).toEqual(["datos"]);
    expect(names("all", "com.example")).toEqual(["datos"]);
  });

  it("estima la limpieza: anónimos sin uso, o todos con --all; nunca los montados", () => {
    expect(pruneCandidates(volumes, false, "1.56").map((v) => v.name.slice(0, 8))).toEqual(["3f1c9e2b"]);
    expect(pruneCandidates(volumes, true, "1.56").map((v) => v.name.slice(0, 8))).toEqual(["huerfano", "3f1c9e2b"]);
    // Antes de la API 1.42 Docker elimina también los que tienen nombre.
    expect(legacyPrune("1.41")).toBe(true);
    expect(legacyPrune("1.42")).toBe(false);
    expect(legacyPrune("1.43")).toBe(false);
    expect(legacyPrune(null)).toBe(false);
    expect(pruneCandidates(volumes, false, "1.41").map((v) => v.name.slice(0, 8))).toEqual(["huerfano", "3f1c9e2b"]);
    expect(pruneCandidates([volume({ scope: "global", containers: [] })], true, "1.56")).toEqual([]);
  });

  it("prepara montajes para el asistente de contenedores", () => {
    expect(volumeMount("datos")).toEqual({ kind: "volume", source: "datos", target: "/data", readOnly: false });
    expect(bindMount("/home/ana/mis fotos/")).toEqual({ kind: "bind", source: "/home/ana/mis fotos/", target: "/mnt/mis_fotos", readOnly: false });
    expect(bindMount("/").target).toBe("/mnt/carpeta");
  });

  it("muestra los permisos como ls", () => {
    expect(modeString(0o755)).toBe("rwxr-xr-x");
    expect(modeString(0o640)).toBe("rw-r-----");
    expect(modeString(0)).toBe("---------");
  });
});
