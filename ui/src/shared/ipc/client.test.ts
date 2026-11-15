// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { afterEach, describe, expect, it } from "vitest";
import i18n from "@/shared/i18n";
import { LOCALIZED_ERRORS, localizeError, toCommandError } from "./client";

const err = (code: string, message: string) => ({ code, message, httpStatus: null, issues: null });

describe("Errores en el idioma de la interfaz (Fase 9)", () => {
  afterEach(() => void i18n.changeLanguage("es"));

  it("los errores de texto fijo se traducen; los de Docker y con detalle no", async () => {
    await i18n.changeLanguage("en");
    expect(toCommandError(err("engine.notConnected", "no hay conexión con Docker Engine")).message).toBe("Not connected to Docker Engine.");
    expect(toCommandError(err("engine.timeout", "tiempo de espera agotado")).message).toBe("Docker did not respond in time.");
    // El mensaje de Docker (y su estado HTTP) se conserva tal cual.
    const docker = { ...err("engine.docker", "Conflict. The container name \"/web\" is already in use"), httpStatus: 409 };
    expect(toCommandError(docker)).toEqual(docker);
    // Con detalle propio (ruta, valor no válido…) también se conserva.
    expect(toCommandError(err("engine.invalidInput", "nombre no válido: «a b»")).message).toBe("nombre no válido: «a b»");
    await i18n.changeLanguage("es");
    expect(toCommandError(err("engine.notConnected", "x")).message).toBe("No hay conexión con Docker Engine.");
  });

  it("todos los códigos traducidos tienen texto en español y en inglés", () => {
    for (const code of LOCALIZED_ERRORS) {
      const key = `errorCodes.${code.replace(".", "_")}`;
      for (const lng of ["es", "en"]) expect(i18n.exists(key, { lng }), `${lng}: ${key}`).toBe(true);
      expect(localizeError(err(code, "original")).message).not.toBe("original");
    }
  });

  it("lo que no es un error del backend se envuelve", () => {
    expect(toCommandError(new Error("boom"))).toEqual(err("unknown", "boom"));
  });
});
