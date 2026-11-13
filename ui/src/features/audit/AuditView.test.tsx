import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import i18n from "@/shared/i18n";
import { api, auditReport, connected, localServer, remoteServer, streams } from "@/test/apiMock";
import { renderWithQuery, setServerState } from "@/test/render";
import { useApp } from "@/app/store";
import { useServers } from "@/app/server";
import { useContainersNav } from "@/features/containers/store";
import { applyAuditFixes } from "@/features/containers/edit/model";
import type { EditRequest } from "@/shared/ipc/bindings/EditRequest";
import { AuditView } from "./AuditView";
import { useAudits } from "./store";
import { escapeHtml, groupByTarget, matchesFinding, toHtml, toJson, toMarkdown, vulnStatusText } from "./model";

const t = i18n.t.bind(i18n) as unknown as (k: string, o?: Record<string, unknown>) => string;

function reset() {
  vi.clearAllMocks();
  streams.reset();
  useAudits.setState({ runs: {} });
  setServerState(connected);
  useApp.setState({ section: "audit" });
  useContainersNav.setState({ page: { kind: "list" } });
}

describe("modelo de la auditoría", () => {
  it("agrupa por elemento con los peores primero y filtra", () => {
    const r = auditReport();
    const g = groupByTarget(r.findings);
    expect(g.map((x) => [x.name, x.worst])).toEqual([
      ["web", "critical"],
      ["db", "medium"],
      ["viejo:1", "low"],
    ]);
    expect(g[0]!.findings.map((f) => f.level)).toEqual(["critical", "medium", "low"]);
    expect(r.findings.filter((f) => matchesFinding(f, "medium", "all", "", t)).length).toBe(2);
    expect(r.findings.filter((f) => matchesFinding(f, "all", "image", "", t)).map((f) => f.target.name)).toEqual(["viejo:1"]);
    expect(r.findings.filter((f) => matchesFinding(f, "all", "all", "socket", t)).map((f) => f.code)).toEqual(["risk.runtimeSocket"]);
  });

  it("nunca dice «0 vulnerabilidades» si el análisis no terminó, y siempre da la fecha de la base", () => {
    expect(vulnStatusText(t, { status: "notAnalyzed", image: "x", reason: "audit.vuln.noTool" }, "es")).toBe(
      "No analizado: no hay Trivy ni Grype instalados en este equipo (la aplicación no los instala).",
    );
    expect(vulnStatusText(t, { status: "failed", image: "x", tool: "trivy", message: "base dañada" }, "es")).toBe("El análisis con trivy no terminó: base dañada");
    const zero = { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
    expect(vulnStatusText(t, { status: "done", image: "x", tool: "grype", toolVersion: "0.86", dbUpdated: "2026-11-09T04:00:00Z", counts: zero, top: [] }, "es")).toMatch(
      /^grype no encuentra vulnerabilidades conocidas en su base del .*2026.*Eso no garantiza que no las haya\.$/,
    );
    expect(vulnStatusText(t, { status: "done", image: "x", tool: "trivy", toolVersion: null, dbUpdated: null, counts: { ...zero, high: 2 }, top: [] }, "es")).toContain(
      "trivy (base del fecha desconocida): 0 críticas, 2 altas",
    );
  });

  it("los informes exportados escapan el HTML, ocultan secretos e incluyen explicación y recomendación", () => {
    const r = auditReport();
    r.findings[0]!.target.name = "<script>alert(1)</script>";
    r.skipped = [{ target: { kind: "service", id: "p", name: "", project: "p" }, reason: "audit.skipped.composeConfig", detail: "postgres://app:clave-e3@db/app MYSQL_PWD=otra-e3" }];
    const html = toHtml(t, r, "Este equipo", "es");
    expect(html).not.toContain("<script>alert(1)</script>");
    expect(html).toContain(escapeHtml("<script>alert(1)</script>"));
    expect(html).toContain("Por qué importa");
    for (const out of [html, toMarkdown(t, r, "Este equipo", "es"), toJson(r)]) {
      expect(out).not.toContain("clave-e3");
      expect(out).not.toContain("otra-e3");
    }
    const md = toMarkdown(t, r, "Este equipo", "es");
    expect(md).toContain("# Auditoría de seguridad · Este equipo");
    expect(md).toContain("Recomendación: Quita el socket.");
    expect(md).toContain("No analizado: no se pidió el análisis.");
  });

  it("las correcciones propuestas solo cambian el borrador: puerto en 127.0.0.1 y carpeta en solo lectura", () => {
    const req = {
      name: "web",
      description: "",
      restart: { name: "no", maxRetries: null },
      cpus: null,
      memoryBytes: null,
      memoryReservationBytes: null,
      env: [],
      ports: [
        { hostIp: "", hostPort: 8080, containerPort: 80, protocol: "tcp" },
        { hostIp: "127.0.0.1", hostPort: 5432, containerPort: 5432, protocol: "tcp" },
      ],
      mounts: [
        { kind: "bind", source: "/etc/nginx", target: "/etc/nginx", readOnly: false },
        { kind: "volume", source: "datos", target: "/datos", readOnly: false },
      ],
    } as unknown as EditRequest;
    const before = structuredClone(req);
    const out = applyAuditFixes(req, [
      { kind: "resources" },
      { kind: "portLocalhost", containerPort: 80, protocol: "tcp", hostPort: "8080" },
      { kind: "mountReadOnly", target: "/etc/nginx" },
    ]);
    expect(req).toEqual(before);
    expect(out.request.ports[0]!.hostIp).toBe("127.0.0.1");
    expect(out.request.mounts[0]!.readOnly).toBe(true);
    expect(out.request.mounts[1]!.readOnly).toBe(false);
    expect(out).toMatchObject({ applied: 2, tab: "ports", resources: true });
    expect(applyAuditFixes(req, [{ kind: "resources" }])).toMatchObject({ applied: 0, tab: "resources" });
  });
});

describe("Auditoría", () => {
  beforeEach(reset);

  it("audita en solo lectura, muestra el resultado con explicaciones y propone correcciones en la edición", async () => {
    renderWithQuery(<AuditView />);
    expect(await screen.findByText(/no cambia nada/)).toBeInTheDocument();
    expect(screen.getByText("Todavía no hay ninguna auditoría de este servidor.")).toBeInTheDocument();
    // Sin Trivy ni Grype, la casilla lo dice.
    expect(await screen.findByText(/^No hay Trivy ni Grype instalados/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Auditar" }));
    expect(api.auditRunOpen).toHaveBeenCalledWith(
      { container: null, includeCompose: true, includeImages: true, scanVulnerabilities: false, tool: null },
      expect.any(Function),
    );
    act(() => streams.emit(1, { event: "progress", progress: { step: "containers", done: 0, total: 2, current: "web" } }));
    expect(screen.getByText("Contenedores 1 de 2: web")).toBeInTheDocument();
    act(() => streams.emit(1, { event: "end", report: auditReport(), error: null }));

    const counts = screen.getByRole("list", { name: "Hallazgos por nivel" });
    expect(within(counts).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["Crítico1", "Alto0", "Medio2", "Bajo2"]);
    const web = screen.getByRole("heading", { name: "web" }).closest("li")!;
    expect(within(web).getByText(/el socket del motor de contenedores da control total/)).toBeInTheDocument();
    expect(within(web).getByText("Quita el socket. Si de verdad lo necesita, usa un proxy del socket con permisos limitados o Docker rootless.")).toBeInTheDocument();
    expect(screen.getByText(/Sin hallazgos: seguro/)).toBeInTheDocument();
    expect(screen.getByText("No analizado: no se pidió el análisis.")).toBeInTheDocument();

    // Filtro por nivel.
    await userEvent.click(screen.getByRole("radio", { name: "Medio" }));
    expect(screen.getByText("2 hallazgos")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "viejo:1" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "Todos" }));

    // Corrección propuesta: abre la edición con los cambios (nada se aplica).
    await userEvent.click(within(web).getByRole("button", { name: "Proponer 2 correcciones" }));
    expect(useApp.getState().section).toBe("containers");
    expect(useContainersNav.getState().page).toEqual({
      kind: "edit",
      id: "c0ffee0123456789",
      name: "web",
      proposal: [{ kind: "resources" }, { kind: "portLocalhost", containerPort: 80, protocol: "tcp", hostPort: "8080" }],
    });
    expect(api.containerEditApply).not.toHaveBeenCalled();
  });

  it("exporta a Markdown, HTML y JSON por el diálogo de Rust", async () => {
    useAudits.setState({ runs: { local: { status: "done", stream: null, progress: null, report: auditReport(), error: null, cancelRequested: false } } });
    renderWithQuery(<AuditView />);
    await userEvent.click(await screen.findByRole("button", { name: "Markdown" }));
    expect(api.auditExportSave).toHaveBeenCalledWith("markdown", expect.stringContaining("# Auditoría de seguridad"), "auditoria-local-20261113T120000-abc123");
    expect(await screen.findByText("Informe guardado en /home/u/auditoria-local-20261113T120000-abc123 (solo tú puedes leerlo).")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "HTML" }));
    expect(api.auditExportSave).toHaveBeenLastCalledWith("html", expect.stringMatching(/^<!doctype html>/), expect.any(String));
    await userEvent.click(screen.getByRole("button", { name: "JSON" }));
    expect(JSON.parse(vi.mocked(api.auditExportSave).mock.calls.at(-1)![1]).id).toBe("20261113T120000-abc123");
  });

  it("cancelar cierra el flujo; el historial abre auditorías anteriores", async () => {
    vi.mocked(api.auditHistory).mockResolvedValueOnce([
      { id: "20261101T100000-old", server: "local", startedAt: "2026-11-01T10:00:00+00:00", finishedAt: "2026-11-01T10:00:01+00:00", container: null, targets: 3, counts: { critical: 0, high: 1, medium: 0, low: 4 }, vulnsAnalyzed: false },
    ]);
    renderWithQuery(<AuditView />);
    await userEvent.click(await screen.findByRole("button", { name: "Auditar" }));
    await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    await waitFor(() => expect(api.streamClose).toHaveBeenCalledWith(1));
    expect(await screen.findByText("Auditoría cancelada.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Ver" }));
    expect(api.auditGet).toHaveBeenCalledWith("20261101T100000-old");
    expect(await screen.findByRole("heading", { name: "Resultado" })).toBeInTheDocument();
  });

  it("en un servidor remoto no se analizan vulnerabilidades", async () => {
    const remote = "srv-0123abcd";
    useServers.setState({ servers: [localServer(), remoteServer({ id: remote, name: "Producción" }, connected)] });
    setServerState(connected, remote);
    renderWithQuery(<AuditView />, remote);
    const box = await screen.findByRole("checkbox", { name: /Vulnerabilidades conocidas/ });
    expect(box).toBeDisabled();
    expect(screen.getByText("Solo se analizan imágenes del Docker de este equipo.")).toBeInTheDocument();
    expect(api.auditTools).not.toHaveBeenCalled();
  });
});
