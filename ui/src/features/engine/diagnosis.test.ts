import { describe, expect, it } from "vitest";
import { diagnosis } from "@/test/apiMock";
import { explain } from "./diagnosis";

const perms = (over: Partial<{ group: string; userInGroup: boolean; groupActiveInSession: boolean }> = {}) => ({
  kind: "permissionDenied" as const,
  permissions: { socketPath: "/var/run/docker.sock", owner: "root", group: "docker", mode: "660", userInGroup: false, groupActiveInSession: false, ...over },
});

describe("explain", () => {
  it("permiso denegado fuera del grupo: propone usermod, reinicio de sesión y aviso de seguridad", () => {
    const e = explain(diagnosis(perms()));
    expect(e.problem).toBe("permissionDenied");
    expect(e.params).toMatchObject({ user: "sergio", group: "docker", mode: "660", path: "/var/run/docker.sock" });
    expect(e.steps.map((s) => s.key)).toEqual(["addGroup", "relogin", "rootlessAlternative", "retry"]);
    expect(e.steps[0]!.command).toBe("sudo usermod -aG docker sergio");
    expect(e.groupWarning).toBe("docker");
  });

  it("permiso denegado ya en el grupo pero sin sesión renovada: solo pide reiniciar sesión", () => {
    const e = explain(diagnosis(perms({ userInGroup: true, groupActiveInSession: false })));
    expect(e.steps.map((s) => s.key)).toEqual(["alreadyInGroup", "retry"]);
    expect(e.steps.some((s) => s.command)).toBe(false);
    expect(e.groupWarning).toBeUndefined();
  });

  it("socket con grupo inesperado: no propone añadir al usuario a ese grupo", () => {
    const e = explain(diagnosis(perms({ group: "root" })));
    expect(e.steps.map((s) => s.key)).toEqual(["ownerUnexpected", "rootlessAlternative", "retry"]);
    expect(e.steps.some((s) => s.command?.includes("usermod"))).toBe(false);
  });

  it("daemon detenido en el sistema o en rootless", () => {
    const sys = explain(diagnosis({ kind: "daemonNotRunning", socketPath: "/var/run/docker.sock" }));
    expect(sys.steps[0]!.command).toBe("sudo systemctl enable --now docker");
    const rl = explain(diagnosis({ kind: "daemonNotRunning", socketPath: "/run/user/1000/docker.sock" }));
    expect(rl.steps[0]!.command).toBe("systemctl --user enable --now docker");
    expect(rl.steps.some((s) => s.command?.startsWith("sudo"))).toBe(false);
  });

  it("sin socket ni instalación: propone instalar", () => {
    const e = explain(diagnosis({ kind: "noSocket" }, { installation: { cliPath: null, daemonPath: null, rootlessTools: false } }));
    expect(e.problem).toBe("notInstalled");
    expect(e.steps[0]!.command).toBe("sudo apt install docker.io");
  });

  it("sin socket con solo herramientas rootless: arranque rootless", () => {
    const e = explain(diagnosis({ kind: "noSocket" }, { installation: { cliPath: "/usr/bin/docker", daemonPath: null, rootlessTools: true } }));
    expect(e.problem).toBe("noSocket");
    expect(e.steps[0]!.command).toBe("systemctl --user enable --now docker");
  });

  it("host no soportado, ruta inválida, no es Docker, API antigua y tiempo agotado", () => {
    expect(explain(diagnosis({ kind: "unsupportedHost", host: "tcp://1.2.3.4:2375" })).params.host).toBe("tcp://1.2.3.4:2375");
    expect(explain(diagnosis({ kind: "invalidPath", path: "x.sock", reason: "la ruta debe ser absoluta" })).steps.map((s) => s.key)).toEqual(["fixPath"]);
    expect(explain(diagnosis({ kind: "notDocker", socketPath: "/tmp/x.sock", message: "respuesta inesperada" })).problem).toBe("notDocker");
    expect(explain(diagnosis({ kind: "apiTooOld", socketPath: "/var/run/docker.sock", apiVersion: "1.40", minimum: "1.41" })).params).toEqual({ api: "1.40", min: "1.41" });
    expect(explain(diagnosis({ kind: "timeout", socketPath: "/var/run/docker.sock" })).steps[0]!.command).toBe("sudo systemctl restart docker");
  });
});
