import { describe, expect, it } from "vitest";
import { composeProjects, project, tiendaModel } from "@/test/apiMock";
import { downAction, matchesProject, nameFromDir, portLabel, refOf, relativeTo, removableVolumes, serviceRows, template, upAction, validProjectName } from "./model";

describe("modelo de Compose", () => {
  it("une los servicios del archivo con sus contenedores reales", () => {
    const p = project({
      containers: [
        ...project().containers,
        { id: "x1", name: "tienda-viejo-1", service: "viejo", number: 1, state: "exited", status: "", image: "a", oneoff: false },
        { id: "x2", name: "tienda-web-run-1", service: "web", number: 1, state: "running", status: "", image: "a", oneoff: true },
      ],
    });
    const rows = serviceRows(tiendaModel, p.containers);
    expect(rows.map((r) => [r.name, r.state, r.orphan])).toEqual([
      ["db", "running", false],
      ["extra", "notCreated", false],
      ["viejo", "stopped", true],
      ["web", "running", false],
    ]);
    expect(rows.find((r) => r.name === "web")!.containers).toHaveLength(1);
    // Sin modelo (archivos ausentes) se muestran los servicios por sus contenedores, sin marcar huérfanos.
    expect(serviceRows(null, p.containers).every((r) => !r.orphan)).toBe(true);
  });

  it("filtra y busca proyectos", () => {
    const f = (filter: Parameters<typeof matchesProject>[1], q = "") => composeProjects.filter((p) => matchesProject(p, filter, q)).map((p) => p.name);
    expect(f("all")).toEqual(["tienda", "blog", "nuevo"]);
    expect(f("running")).toEqual(["tienda"]);
    expect(f("stopped")).toEqual(["blog"]);
    expect(f("notDeployed")).toEqual(["nuevo"]);
    expect(f("all", "/srv")).toEqual(["blog"]);
    expect(f("all", "web")).toEqual(["tienda"]);
  });

  it("acciones, puertos, volúmenes y nombres", () => {
    expect(upAction({ build: true })).toEqual({ action: "up", services: [], build: true, pull: "missing", forceRecreate: false, removeOrphans: false, profiles: [] });
    expect(downAction()).toEqual({ action: "down", volumes: false, images: "none", removeOrphans: false });
    expect(portLabel({ published: "8080", target: "80", protocol: "tcp", hostIp: null })).toBe("8080→80");
    expect(portLabel({ published: "53", target: "53", protocol: "udp", hostIp: "127.0.0.1" })).toBe("127.0.0.1:53→53/udp");
    expect(portLabel({ published: null, target: "9000", protocol: "tcp", hostIp: null })).toBe("9000");
    expect(removableVolumes(tiendaModel)).toEqual(["tienda_datos"]);
    expect(nameFromDir("/home/ana/Mi Tienda.v2/")).toBe("mitiendav2");
    expect(validProjectName("mi-app_2")).toBe(true);
    expect(validProjectName("Mal")).toBe(false);
    expect(validProjectName("-x")).toBe(false);
    expect(template("demo")).toMatch(/^name: demo\n/);
    expect(template("")).toMatch(/^services:/);
    expect(relativeTo("/srv/app/db", "/srv/app")).toBe("./db");
    expect(relativeTo("/srv/app", "/srv/app/")).toBe(".");
    expect(relativeTo("/otra/db", "/srv/app")).toBe("/otra/db");
    expect(refOf(project())).toEqual({ name: "tienda", files: ["/home/u/tienda/compose.yaml"], workingDir: "/home/u/tienda", envFiles: [] });
  });
});
