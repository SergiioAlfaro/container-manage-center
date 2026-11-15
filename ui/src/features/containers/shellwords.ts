// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * División de una línea en argumentos con las reglas de comillas de POSIX sh
 * (sin expansión de variables ni comodines: no hay shell, los argumentos se
 * envían tal cual a Docker). Se usa para los campos Comando y Entrypoint.
 */
export type SplitResult = { ok: true; args: string[] } | { ok: false; error: "unclosedQuote" | "trailingEscape" };

export function splitArgs(line: string): SplitResult {
  const args: string[] = [];
  let cur = "";
  let inArg = false;
  let i = 0;
  while (i < line.length) {
    const c = line[i]!;
    if (c === "'") {
      const end = line.indexOf("'", i + 1);
      if (end < 0) return { ok: false, error: "unclosedQuote" };
      cur += line.slice(i + 1, end);
      inArg = true;
      i = end + 1;
    } else if (c === '"') {
      i++;
      let closed = false;
      while (i < line.length) {
        const d = line[i]!;
        if (d === '"') {
          closed = true;
          i++;
          break;
        }
        if (d === "\\" && i + 1 < line.length && '"\\$`'.includes(line[i + 1]!)) {
          cur += line[i + 1];
          i += 2;
        } else {
          cur += d;
          i++;
        }
      }
      if (!closed) return { ok: false, error: "unclosedQuote" };
      inArg = true;
    } else if (c === "\\") {
      if (i + 1 >= line.length) return { ok: false, error: "trailingEscape" };
      cur += line[i + 1];
      inArg = true;
      i += 2;
    } else if (/\s/.test(c)) {
      if (inArg) args.push(cur);
      cur = "";
      inArg = false;
      i++;
    } else {
      cur += c;
      inArg = true;
      i++;
    }
  }
  if (inArg) args.push(cur);
  return { ok: true, args };
}

/** Inverso de `splitArgs`: comillas simples solo cuando hacen falta. */
export function joinArgs(args: string[]): string {
  return args.map(quote).join(" ");
}

export function quote(a: string): string {
  if (a !== "" && /^[A-Za-z0-9_@%+=:,./-]+$/.test(a)) return a;
  return `'${a.replace(/'/g, `'\\''`)}'`;
}
