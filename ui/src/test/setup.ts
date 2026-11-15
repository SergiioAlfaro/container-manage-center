// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import i18n from "@/shared/i18n";

// jsdom no implementa matchMedia.
if (!window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}

afterEach(async () => {
  cleanup();
  // Cada prueba empieza sin servidores conocidos ni cachés (0.10.0).
  const { useServers, resetQueryClients } = await import("@/app/server");
  useServers.setState({ servers: [], states: {}, selected: "local", busy: {}, keyringNotice: {}, secretPrompt: null, error: null, loaded: false });
  resetQueryClients();
  await i18n.changeLanguage("es");
});

// xterm.js necesita canvas, que jsdom no tiene: se sustituye por un doble.
vi.mock("@xterm/xterm", async () => ({ Terminal: (await import("./xtermMock")).FakeTerminal }));
vi.mock("@xterm/addon-fit", async () => ({ FitAddon: (await import("./xtermMock")).FakeFit }));

if (!("ResizeObserver" in window)) {
  (window as unknown as { ResizeObserver: unknown }).ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

// jsdom no tiene PointerEvent: sin él, los eventos de puntero pierden la
// posición y el botón (los separadores de «Terminales» los usan).
if (!("PointerEvent" in window)) {
  class PointerEventPolyfill extends MouseEvent {
    pointerId: number;
    constructor(type: string, init: PointerEventInit = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 0;
    }
  }
  (window as unknown as { PointerEvent: unknown }).PointerEvent = PointerEventPolyfill;
}
