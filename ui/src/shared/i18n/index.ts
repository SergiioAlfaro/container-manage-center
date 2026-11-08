import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import es from "./locales/es.json";
import en from "./locales/en.json";
import fr from "./locales/fr.json";
import it from "./locales/it.json";
import type { Language } from "@/shared/ipc/bindings/Language";

export const LANGUAGES: Language[] = ["es", "en", "fr", "it"];

void i18n.use(initReactI18next).init({
  resources: { es: { translation: es }, en: { translation: en }, fr: { translation: fr }, it: { translation: it } },
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
