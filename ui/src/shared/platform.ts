// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Plataforma en la que se ejecuta la interfaz (1.9.0, D-114). En Windows,
 * WebView2 se identifica como Windows en su agente de usuario.
 */
export function isWindows(): boolean {
  return typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
}

/**
 * Clave de traducción para esta plataforma: en Windows, la variante `…Win`
 * si existe (textos que en Linux hablan de systemd, apt o grupos de Unix).
 */
export function platformKey(key: string, exists: (k: string) => boolean, windows: boolean = isWindows()): string {
  return windows && exists(`${key}Win`) ? `${key}Win` : key;
}
