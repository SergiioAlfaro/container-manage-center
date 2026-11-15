// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { describe, expect, it } from "vitest";
import type { RemoteProblem } from "@/shared/ipc/bindings/RemoteProblem";
import { serverConfig } from "@/test/apiMock";
import { displayAddress, explainRemote, hostPattern, sshCommand, statusOf } from "./remote";

const c = serverConfig({ host: "docker-a.example.com", port: 2222, user: "deploy", auth: "key", keyPath: "~/.ssh/id_ed25519" });

const ALL: RemoteProblem[] = [
  { kind: "sshMissing" },
  { kind: "invalidProfile", codes: ["hostInvalid"] },
  { kind: "hostUnresolvable", host: "x" },
  { kind: "hostUnreachable", message: "No route to host" },
  { kind: "sshRefused", port: 2222 },
  { kind: "notSsh" },
  { kind: "timeout" },
  { kind: "hostKeyUnknown", presented: { keyType: "ssh-ed25519", fingerprint: "SHA256:abc" } },
  { kind: "hostKeyChanged", presented: { keyType: "ssh-ed25519", fingerprint: "SHA256:abc" }, file: "/x/known_hosts", inUserFile: false },
  { kind: "authFailed", methods: "publickey" },
  { kind: "secretRequired", secret: "passphrase" },
  { kind: "keyUnreadable", path: "/k" },
  { kind: "dockerMissing" },
  { kind: "dockerNotRunning", message: "Cannot connect" },
  { kind: "socketDenied", message: "permission denied" },
  { kind: "contextMissing", name: "prod" },
  { kind: "notDocker", message: "?" },
  { kind: "apiTooOld", apiVersion: "1.40", minimum: "1.41" },
  { kind: "connectionLost" },
  { kind: "other", message: "x" },
];

describe("servidores remotos: problemas y soluciones", () => {
  it("cada problema tiene explicación y pasos; ninguno los ejecuta la aplicación", () => {
    for (const p of ALL) {
      const e = explainRemote(p, c);
      expect(e.problem, p.kind).toBeTruthy();
      expect(e.steps.length, p.kind).toBeGreaterThan(0);
      // Las órdenes propuestas dicen en qué equipo se ejecutan.
      for (const s of e.steps) if (s.command) expect(s.where, `${p.kind}: ${s.command}`).toBeDefined();
    }
  });

  it("distingue los casos del diagnóstico por servidor (§10)", () => {
    const kinds = ALL.map((p) => explainRemote(p, c).problem);
    for (const k of ["hostUnreachable", "sshRefused", "authFailedKey", "hostKeyChanged", "dockerNotRunning", "socketDenied", "apiTooOld"])
      expect(kinds).toContain(k);
    expect(explainRemote({ kind: "authFailed", methods: null }, { ...c, auth: "password" }).problem).toBe("authFailedPassword");
    expect(explainRemote({ kind: "authFailed", methods: null }, { ...c, auth: "agent" }).problem).toBe("authFailedAgent");
  });

  it("una identidad cambiada es un aviso de seguridad y no propone conectar sin revisar", () => {
    const e = explainRemote(ALL[8]!, c);
    expect(e.danger).toBe(true);
    expect(e.steps[0]!.key).toBe("doNotConnect");
    const inUser = explainRemote({ kind: "hostKeyChanged", presented: null, file: "/home/u/.ssh/known_hosts", inUserFile: true }, c);
    expect(inUser.steps.at(-1)).toMatchObject({ key: "removeUserKey", command: "ssh-keygen -R '[docker-a.example.com]:2222'", where: "local" });
  });

  it("sin permiso en el socket avisa de que el grupo docker equivale a root", () => {
    const e = explainRemote({ kind: "socketDenied", message: "x" }, c);
    expect(e.groupWarning).toBe(true);
    expect(e.steps.find((s) => s.command)?.where).toBe("server");
  });

  it("direcciones y órdenes", () => {
    expect(hostPattern({ host: "a", port: 22 })).toBe("a");
    expect(hostPattern({ host: "a", port: 2222 })).toBe("[a]:2222");
    expect(displayAddress({ user: "u", host: "fe80::1", port: 22 })).toBe("u@[fe80::1]");
    expect(sshCommand(c, "docker version")).toBe("ssh -p 2222 deploy@docker-a.example.com 'docker version'");
    // Nada que el shell del usuario pudiera interpretar.
    expect(sshCommand({ user: "u", host: "h", port: 22 }, "a'b")).toBe(`ssh u@h 'a'\\''b'`);
  });

  it("estado resumido de la lista", () => {
    expect(statusOf(null)).toBe("disconnected");
    expect(statusOf({ state: "idle" })).toBe("disconnected");
    expect(statusOf({ state: "connecting" })).toBe("connecting");
    const down = (remote: RemoteProblem) =>
      ({
        state: "disconnected",
        since: "",
        diagnosis: { problem: { kind: "remote", remote, steps: [] }, candidates: [], installation: {}, user: {}, mode: { mode: "auto" } },
      }) as never;
    expect(statusOf(down({ kind: "authFailed", methods: null }))).toBe("authError");
    expect(statusOf(down({ kind: "sshRefused", port: 22 }))).toBe("connectionError");
    expect(statusOf(down({ kind: "dockerNotRunning", message: "" }))).toBe("dockerUnavailable");
    expect(statusOf(down({ kind: "hostKeyChanged", presented: null, file: null, inUserFile: false }))).toBe("identityError");
  });
});
