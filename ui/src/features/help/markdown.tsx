// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import type { ReactNode } from "react";

/**
 * Lector mínimo del Markdown de la ayuda (D-066). Solo lo que usa la
 * documentación: secciones `## Título {#id}`, párrafos, listas, tablas, bloques
 * de código, `código` y **negrita**. Produce elementos de React: nunca inserta
 * HTML, así que el texto no puede ejecutar nada.
 */

export type Block =
  | { kind: "p"; text: string }
  | { kind: "ul"; items: string[] }
  | { kind: "ol"; items: string[] }
  | { kind: "code"; text: string }
  | { kind: "table"; head: string[]; rows: string[][] };

export interface HelpSection {
  id: string;
  title: string;
  blocks: Block[];
}

const HEADING = /^## (.+?)\s*\{#([a-z0-9-]+)\}\s*$/;

const cells = (line: string) =>
  line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((c) => c.trim());

export function parseHelp(md: string): HelpSection[] {
  const out: HelpSection[] = [];
  const lines = md.replace(/\r\n/g, "\n").split("\n");
  let cur: HelpSection | null = null;
  let i = 0;
  const push = (b: Block) => cur?.blocks.push(b);
  while (i < lines.length) {
    const line = lines[i]!;
    const h = HEADING.exec(line);
    if (h) {
      cur = { title: h[1]!, id: h[2]!, blocks: [] };
      out.push(cur);
      i++;
      continue;
    }
    if (line.trim() === "") {
      i++;
      continue;
    }
    if (line.startsWith("```")) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !lines[i]!.startsWith("```")) body.push(lines[i++]!);
      i++;
      push({ kind: "code", text: body.join("\n") });
      continue;
    }
    if (line.startsWith("|")) {
      const rows: string[][] = [];
      while (i < lines.length && lines[i]!.startsWith("|")) rows.push(cells(lines[i++]!));
      const [head, sep, ...body] = rows;
      push({ kind: "table", head: head ?? [], rows: sep && sep.every((c) => /^:?-+:?$/.test(c)) ? body : [sep ?? [], ...body].filter((r) => r.length) });
      continue;
    }
    const list = /^(- |\d+\. )/.exec(line);
    if (list) {
      const ordered = list[1] !== "- ";
      const items: string[] = [];
      while (i < lines.length && /^(- |\d+\. )/.test(lines[i]!)) {
        let item = lines[i++]!.replace(/^(- |\d+\. )/, "");
        // Continuación de un elemento en la línea siguiente (sangrada).
        while (i < lines.length && /^ {2,}\S/.test(lines[i]!)) item += " " + lines[i++]!.trim();
        items.push(item);
      }
      push({ kind: ordered ? "ol" : "ul", items });
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && lines[i]!.trim() !== "" && !/^(## |```|\||- |\d+\. )/.test(lines[i]!)) para.push(lines[i++]!.trim());
    push({ kind: "p", text: para.join(" ") });
  }
  return out;
}

/** `código` y **negrita** como elementos. */
export function inline(text: string): ReactNode[] {
  const parts: ReactNode[] = [];
  const re = /(`[^`]+`|\*\*[^*]+\*\*)/g;
  let last = 0;
  let k = 0;
  for (let m = re.exec(text); m; m = re.exec(text)) {
    if (m.index > last) parts.push(text.slice(last, m.index));
    const tok = m[0];
    parts.push(
      tok.startsWith("`") ? (
        <code key={k++} className="rounded bg-surface-2 px-1 py-px font-mono text-[0.92em] text-ink">
          {tok.slice(1, -1)}
        </code>
      ) : (
        <strong key={k++} className="font-semibold text-ink">
          {tok.slice(2, -2)}
        </strong>
      ),
    );
    last = m.index + tok.length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts;
}

export function BlockView({ b }: { b: Block }) {
  switch (b.kind) {
    case "p":
      return <p className="text-[13.5px] leading-relaxed text-ink-2">{inline(b.text)}</p>;
    case "ul":
    case "ol": {
      const L = b.kind === "ul" ? "ul" : "ol";
      return (
        <L className={b.kind === "ul" ? "list-disc space-y-1 pl-5 text-[13.5px] text-ink-2" : "list-decimal space-y-1 pl-5 text-[13.5px] text-ink-2"}>
          {b.items.map((it, i) => (
            <li key={i} className="leading-relaxed">
              {inline(it)}
            </li>
          ))}
        </L>
      );
    }
    case "code":
      return <pre className="selectable overflow-x-auto rounded-lg bg-code px-3 py-2 font-mono text-[12.5px] text-ink">{b.text}</pre>;
    case "table":
      return (
        <div className="overflow-x-auto rounded-lg border border-line">
          <table className="w-full text-left text-[13px]">
            <thead className="bg-surface-2 text-ink">
              <tr>
                {b.head.map((h, i) => (
                  <th key={i} className="px-3 py-2 font-medium">
                    {inline(h)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-line text-ink-2">
              {b.rows.map((r, i) => (
                <tr key={i}>
                  {r.map((c, j) => (
                    <td key={j} className="px-3 py-2 align-top">
                      {inline(c)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
  }
}
