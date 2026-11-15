// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Github, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/ipc/client";
import { Button, cx } from "@/shared/ui/controls";
import { Banner } from "@/shared/ui/feedback";
import es from "./docs/es.md?raw";
import en from "./docs/en.md?raw";
import fr from "./docs/fr.md?raw";
import it from "./docs/it.md?raw";
import ca from "./docs/ca.md?raw";
import gl from "./docs/gl.md?raw";
import eu from "./docs/eu.md?raw";
import pt from "./docs/pt.md?raw";
import { BlockView, parseHelp, type Block } from "./markdown";

/** Documento de cada idioma. Para ampliar la ayuda basta con editar estos
 *  archivos (las mismas secciones `{#id}` en todos los idiomas: lo comprueba
 *  una prueba). */
export const HELP_DOCS: Record<string, string> = { en, es, ca, gl, eu, fr, it, pt };

/** Secciones exigidas, en orden (0.9.2). */
export const HELP_SECTION_IDS = [
  "about",
  "requirements",
  "install",
  "first-setup",
  "connection",
  "rootless",
  "remote-servers",
  "dashboard",
  "containers",
  "container-lifecycle",
  "container-edit",
  "container-commit",
  "logs-stats-terminal",
  "images",
  "pull-build",
  "networks",
  "volumes",
  "binds",
  "compose",
  "compose-projects",
  "terminals",
  "activity",
  "settings",
  "uninstall",
  "security",
  "troubleshooting",
  "faq",
  "legal",
  "windows",
] as const;

const blockText = (b: Block): string =>
  b.kind === "p" || b.kind === "code" ? b.text : b.kind === "table" ? [...b.head, ...b.rows.flat()].join(" ") : b.items.join(" ");

export function HelpView() {
  const { t, i18n } = useTranslation();
  const sections = useMemo(() => parseHelp(HELP_DOCS[i18n.language] ?? HELP_DOCS.es!), [i18n.language]);
  const [current, setCurrent] = useState(sections[0]?.id ?? "");
  const [query, setQuery] = useState("");
  const [openError, setOpenError] = useState<string | null>(null);
  const content = useRef<HTMLDivElement>(null);

  const q = query.trim().toLocaleLowerCase(i18n.language);
  const visible = q ? sections.filter((s) => (s.title + " " + s.blocks.map(blockText).join(" ")).toLocaleLowerCase(i18n.language).includes(q)) : sections;
  const index = sections.findIndex((s) => s.id === current);
  const section = q ? null : (sections[index] ?? sections[0]);

  useEffect(() => {
    content.current?.scrollTo?.({ top: 0 });
  }, [current]);

  const go = (id: string) => {
    setQuery("");
    setCurrent(id);
    // Lleva el foco al título para que el teclado y los lectores de pantalla sigan.
    requestAnimationFrame(() => document.getElementById(`help-${id}`)?.focus());
  };

  const sectionView = (s: (typeof sections)[number]) => (
    <article key={s.id} aria-labelledby={`help-${s.id}`} className="space-y-3">
      <h2 id={`help-${s.id}`} tabIndex={-1} className="text-[17px] font-semibold text-ink outline-none">
        {s.title}
      </h2>
      {s.blocks.map((b, i) => (
        <BlockView key={i} b={b} />
      ))}
    </article>
  );

  return (
    <div className="flex min-h-0 flex-col gap-4 lg:flex-row">
      <nav aria-label={t("help.toc")} className="shrink-0 lg:sticky lg:top-0 lg:w-[248px] lg:self-start">
        <label className="relative mb-2 block">
          <Search size={14} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" aria-hidden />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("help.search")}
            aria-label={t("help.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] text-ink outline-none transition-colors placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <ol className="max-h-[50vh] space-y-0.5 overflow-y-auto lg:max-h-[calc(100vh-260px)]">
          {sections.map((s) => (
            <li key={s.id}>
              <button
                type="button"
                onClick={() => go(s.id)}
                aria-current={!q && s.id === current ? "true" : undefined}
                className={cx(
                  "w-full rounded-md px-2.5 py-1.5 text-left text-[12.5px] transition-colors duration-150",
                  !q && s.id === current ? "bg-beacon-soft font-medium text-beacon-strong" : "text-ink-2 hover:bg-surface-sunk hover:text-ink",
                  q && !visible.includes(s) && "opacity-40",
                )}
              >
                {s.title}
              </button>
            </li>
          ))}
        </ol>
        <Button
          className="mt-3 w-full"
          onClick={() => {
            setOpenError(null);
            api.openGithubProfile().catch((e: { message: string }) => setOpenError(e.message));
          }}
        >
          <Github size={14} aria-hidden /> {t("help.github")}
        </Button>
        {openError && (
          <div className="mt-2">
            <Banner tone="error">{openError}</Banner>
          </div>
        )}
      </nav>

      <div ref={content} className="min-w-0 flex-1 rounded-xl border border-line bg-surface px-6 py-5" data-testid="help-content">
        {section ? (
          <>
            {sectionView(section)}
            <div className="mt-6 flex items-center justify-between gap-3 border-t border-line pt-4">
              {index > 0 ? (
                <Button variant="ghost" onClick={() => go(sections[index - 1]!.id)}>
                  <ChevronLeft size={14} aria-hidden /> {sections[index - 1]!.title}
                </Button>
              ) : (
                <span />
              )}
              {index < sections.length - 1 && (
                <Button variant="ghost" onClick={() => go(sections[index + 1]!.id)}>
                  {sections[index + 1]!.title} <ChevronRight size={14} aria-hidden />
                </Button>
              )}
            </div>
          </>
        ) : visible.length ? (
          <div className="space-y-8">
            <p className="text-[12.5px] text-ink-3" aria-live="polite">
              {t("help.results", { count: visible.length })}
            </p>
            {visible.map(sectionView)}
          </div>
        ) : (
          <p className="text-[13px] text-ink-3" aria-live="polite">
            {t("help.noResults")}
          </p>
        )}
      </div>
    </div>
  );
}
