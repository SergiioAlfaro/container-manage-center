// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);

import { api } from "@/test/apiMock";
import { renderWithQuery } from "@/test/render";
import i18n from "@/shared/i18n";
import { HELP_DOCS, HELP_SECTION_IDS, HelpView } from "./HelpView";
import { parseHelp } from "./markdown";
import { LANGUAGES } from "@/shared/i18n";

describe("Ayuda: documento", () => {
  it.each(Object.keys(HELP_DOCS))("%s: tiene las 29 secciones (26 exigidas, «Servidores remotos» de 0.10.0, «Legal» de 1.7.0 y «Windows» de 1.9.0), en orden y con contenido", (lang) => {
    const s = parseHelp(HELP_DOCS[lang]!);
    expect(s.map((x) => x.id)).toEqual([...HELP_SECTION_IDS]);
    s.forEach((x, i) => {
      expect(x.title.startsWith(`${i + 1}. `), `${lang}/${x.id}`).toBe(true);
      expect(x.blocks.length, `${lang}/${x.id}`).toBeGreaterThan(0);
    });
  });

  it("cada idioma tiene su propia traducción (no una copia del español)", () => {
    const es = parseHelp(HELP_DOCS.es!);
    for (const lang of Object.keys(HELP_DOCS).filter((l) => l !== "es")) {
      const other = parseHelp(HELP_DOCS[lang]!);
      // Se compara el texto completo de cada sección (un título como «Docker Compose» puede coincidir).
      const same = other.filter((s, i) => JSON.stringify(s) === JSON.stringify(es[i]));
      expect(same.map((s) => s.id), lang).toEqual([]);
    }
  });

  it("hay ayuda para cada idioma de la interfaz, con las secciones nuevas de 0.9.2", () => {
    expect(Object.keys(HELP_DOCS).sort()).toEqual([...LANGUAGES].sort());
    const es = parseHelp(HELP_DOCS.es!);
    expect(es.find((x) => x.id === "container-edit")?.title).toBe("11. Editar la configuración de un contenedor");
    expect(es.find((x) => x.id === "container-commit")?.title).toBe("12. Crear una imagen a partir de un contenedor");
  });

  it.each(Object.keys(HELP_DOCS))("%s: la sección legal da la licencia, el copyright, el enlace GPL y los avisos de terceros (1.7.0)", (lang) => {
    const legal = parseHelp(HELP_DOCS[lang]!).find((x) => x.id === "legal")!;
    const text = legal.blocks.map((b) => (b.kind === "p" ? b.text : "")).join("\n");
    expect(legal.title).toMatch(/^28\. /);
    for (const must of ["Container Manage Center", "CMC", "GPL-3.0-or-later", "Copyright © 2026 Sergio Alfaro Lorite", "https://www.gnu.org/licenses/gpl-3.0.html", "THIRD_PARTY_NOTICES.md", "Docker, Inc."]) {
      expect(text, `${lang}: ${must}`).toContain(must);
    }
  });

  it("el Markdown se convierte en elementos, sin HTML", () => {
    const [s] = parseHelp("## 1. A {#a}\n\nTexto con <script>x</script> y `código`.\n\n- uno\n  sigue\n- dos\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```\nsudo x\n```\n");
    expect(s!.blocks).toEqual([
      { kind: "p", text: "Texto con <script>x</script> y `código`." },
      { kind: "ul", items: ["uno sigue", "dos"] },
      { kind: "table", head: ["a", "b"], rows: [["1", "2"]] },
      { kind: "code", text: "sudo x" },
    ]);
  });
});

describe("Ayuda: navegación", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  afterEach(async () => {
    await act(() => i18n.changeLanguage("es"));
  });

  it("muestra el índice y la primera sección; el índice y Siguiente/Anterior navegan", async () => {
    renderWithQuery(<HelpView />);
    const toc = screen.getByRole("navigation", { name: "Índice de la ayuda" });
    expect(within(toc).getAllByRole("button").filter((b) => /^\d+\. /.test(b.textContent ?? ""))).toHaveLength(29);
    expect(screen.getByRole("heading", { level: 2, name: "1. Qué es Container Manage Center" })).toBeInTheDocument();
    await userEvent.click(within(toc).getByRole("button", { name: "21. Terminales simultáneas" }));
    expect(screen.getByRole("heading", { level: 2, name: "21. Terminales simultáneas" })).toBeInTheDocument();
    expect(within(toc).getByRole("button", { name: "21. Terminales simultáneas" })).toHaveAttribute("aria-current", "true");
    await userEvent.click(within(screen.getByTestId("help-content")).getByRole("button", { name: /22\. Panel de actividad/ }));
    expect(screen.getByRole("heading", { level: 2, name: /^22\. Panel de actividad/ })).toBeInTheDocument();
    // Las tablas y el código se muestran como tales.
    await userEvent.click(within(toc).getByRole("button", { name: "24. Desinstalación y gestión de dependencias" }));
    expect(screen.getByText(/pkexec \/usr\/bin\/apt-get remove -y --no-auto-remove/).tagName).toBe("PRE");
  });

  it("la búsqueda filtra las secciones", async () => {
    renderWithQuery(<HelpView />);
    await userEvent.type(screen.getByRole("searchbox", { name: "Buscar en la ayuda" }), "pkexec");
    const content = screen.getByTestId("help-content");
    expect(within(content).getByRole("heading", { name: "24. Desinstalación y gestión de dependencias" })).toBeInTheDocument();
    expect(within(content).queryByRole("heading", { name: "16. Redes Docker" })).not.toBeInTheDocument();
    await userEvent.clear(screen.getByRole("searchbox"));
    await userEvent.type(screen.getByRole("searchbox"), "zzzz-nada");
    expect(screen.getByText("Ninguna sección contiene ese texto.")).toBeInTheDocument();
  });

  it("el botón de GitHub abre el perfil con el mecanismo de Tauri", async () => {
    renderWithQuery(<HelpView />);
    await userEvent.click(screen.getByRole("button", { name: "Visitar mi perfil de GitHub" }));
    expect(api.openGithubProfile).toHaveBeenCalledTimes(1);
    api.openGithubProfile.mockRejectedValueOnce({ code: "x", message: "No se pudo abrir el navegador" });
    await userEvent.click(screen.getByRole("button", { name: "Visitar mi perfil de GitHub" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo abrir el navegador");
  });

  it.each([
    ["en", "1. What is Container Manage Center", "Visit my GitHub profile"],
    ["fr", "1. Qu’est-ce que Container Manage Center", "Visiter mon profil GitHub"],
    ["it", "1. Che cos’è Container Manage Center", "Visita il mio profilo GitHub"],
    ["ca", "1. Què és Container Manage Center", "Visitar el meu perfil de GitHub"],
    ["gl", "1. Que é Container Manage Center", "Visitar o meu perfil de GitHub"],
    ["eu", "1. Zer da Container Manage Center", "Bisitatu nire GitHub profila"],
    ["pt", "1. O que é o Container Manage Center", "Visitar o meu perfil do GitHub"],
  ])("en %s muestra el documento de su idioma", async (lang, title, github) => {
    await act(() => i18n.changeLanguage(lang));
    renderWithQuery(<HelpView />);
    expect(screen.getByRole("heading", { level: 2, name: title })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: github })).toBeInTheDocument();
  });
});

describe("Ayuda: servidores remotos (0.10.0)", () => {
  it.each(Object.keys(HELP_DOCS))("%s: explica los servidores remotos tras «Docker rootless» y ya no dice que ssh:// no se admite", (lang) => {
    const s = parseHelp(HELP_DOCS[lang]!);
    const i = s.findIndex((x) => x.id === "remote-servers");
    expect(i).toBe(s.findIndex((x) => x.id === "rootless") + 1);
    expect(s[i]!.title.startsWith("7. ")).toBe(true);
    const text = HELP_DOCS[lang]!;
    expect(text).toContain("docker system dial-stdio");
    expect(text).toContain("2375");
    expect(text).not.toMatch(/`tcp:\/\/` (y|and|i|e|et|eta) `ssh:\/\/`/);
  });
});
