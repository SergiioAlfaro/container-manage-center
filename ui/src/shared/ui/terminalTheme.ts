// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import type { ITheme, Terminal } from "@xterm/xterm";

/**
 * Colores de las terminales (detalle del contenedor y apartado «Terminales»).
 * El fondo es siempre oscuro; el cursor y la selección usan el color
 * principal morado (D-072, D-082): el valor de `--beacon-nav`, que es el
 * acento pensado para fondos oscuros. Desde la 1.0.0 no hay otro color.
 */
const CURSOR = "#a184f2";

export function terminalTheme(): ITheme {
  const cursor = CURSOR;
  return {
    background: "#0b1219",
    foreground: "#d5dee7",
    cursor,
    selectionBackground: `${cursor}55`,
    black: "#1d2e40",
    brightBlack: "#6f8399",
  };
}

const open = new Set<Terminal>();

/** Registra una terminal para actualizar sus colores si cambia el color principal. */
export function trackTerminal(term: Terminal): () => void {
  open.add(term);
  return () => open.delete(term);
}

/** Aplica el color principal actual a todas las terminales abiertas, sin reiniciarlas. */
export function refreshTerminalThemes() {
  const theme = terminalTheme();
  for (const t of open) t.options.theme = theme;
}
