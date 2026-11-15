// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import es from "./locales/es.json";
import en from "./locales/en.json";
import fr from "./locales/fr.json";
import it from "./locales/it.json";
import ca from "./locales/ca.json";
import gl from "./locales/gl.json";
import eu from "./locales/eu.json";
import pt from "./locales/pt.json";
import type { Language } from "@/shared/ipc/bindings/Language";

/** Orden del selector de idioma (0.9.2): inglés primero. */
export const LANGUAGES: Language[] = ["en", "es", "ca", "gl", "eu", "fr", "it", "pt"];

/**
 * Nombre de cada idioma en sí mismo (endónimo), igual en todas las
 * traducciones: así cada persona encuentra el suyo aunque la interfaz esté en
 * otro idioma.
 */
export const LANGUAGE_NAMES: Record<Language, string> = {
  en: "English",
  es: "Español",
  ca: "Català",
  gl: "Galego",
  eu: "Euskara",
  fr: "Français",
  it: "Italiano",
  pt: "Português",
};

void i18n.use(initReactI18next).init({
  resources: {
    es: { translation: es },
    en: { translation: en },
    ca: { translation: ca },
    gl: { translation: gl },
    eu: { translation: eu },
    fr: { translation: fr },
    it: { translation: it },
    pt: { translation: pt },
  },
  lng: "es",
  fallbackLng: "es",
  interpolation: { escapeValue: false },
  returnNull: false,
  // Sin mensajes promocionales en la consola.
  showSupportNotice: false,
});

export function setLanguage(lang: Language) {
  if (i18n.language !== lang) void i18n.changeLanguage(lang);
  document.documentElement.lang = lang;
}

export default i18n;
