import type { ITheme, Terminal } from "@xterm/xterm";

/**
 * Colores de las terminales (detalle del contenedor y apartado «Terminales»).
 * El fondo es siempre oscuro; el cursor y la selección siguen el color
 * principal (0.9.2, D-072). Son los valores de `--beacon-nav`, que es el
 * acento pensado para fondos oscuros.
 */
const CURSOR: Record<string, string> = {
  teal: "#25b0b3",
  purple: "#a184f2",
};

export function terminalTheme(accent: string = document.documentElement.dataset.accent ?? "teal"): ITheme {
  const cursor = CURSOR[accent] ?? CURSOR.teal!;
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
