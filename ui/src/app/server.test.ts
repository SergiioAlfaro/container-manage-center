// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@/shared/ipc/client", async () => (await import("@/test/apiMock")).module);
import { readSelected } from "./server";

describe("servidor elegido (1.8.0)", () => {
  beforeEach(() => window.localStorage.clear());

  it("la preferencia guardada con el nombre anterior pasa a la clave nueva", () => {
    window.localStorage.setItem("dcc.selectedServer", "srv-1");
    expect(readSelected()).toBe("srv-1");
    expect(window.localStorage.getItem("cmc.selectedServer")).toBe("srv-1");
    expect(window.localStorage.getItem("dcc.selectedServer")).toBeNull();
  });

  it("si ya hay una con la clave nueva, se conserva esa y la antigua se quita", () => {
    window.localStorage.setItem("cmc.selectedServer", "srv-2");
    window.localStorage.setItem("dcc.selectedServer", "srv-1");
    expect(readSelected()).toBe("srv-2");
    expect(window.localStorage.getItem("dcc.selectedServer")).toBeNull();
  });

  it("sin nada guardado, el equipo local", () => {
    expect(readSelected()).toBe("local");
  });
});
