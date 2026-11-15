// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
// Configuración mínima de ESLint (E2, D-092): errores reales y reglas de
// seguridad, sin reglas de estilo (de eso se encarga el formato existente).
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default tseslint.config(
  { ignores: ["dist/**", "node_modules/**", "target/**", "src-tauri/**", "ui/src/shared/ipc/bindings/**"] },
  {
    files: ["ui/src/**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    plugins: { "react-hooks": reactHooks },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      // Convención del proyecto: `_nombre` = argumento que no se usa a propósito.
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_", caughtErrorsIgnorePattern: "^_" }],
      // Seguridad: nada de evaluar texto como código ni de insertar HTML.
      "no-eval": "error",
      "no-implied-eval": "error",
      "no-new-func": "error",
      "no-script-url": "error",
      "no-restricted-properties": [
        "error",
        { property: "innerHTML", message: "No insertes HTML: crea elementos de React." },
        { property: "outerHTML", message: "No insertes HTML: crea elementos de React." },
        { property: "insertAdjacentHTML", message: "No insertes HTML: crea elementos de React." },
      ],
      "no-restricted-syntax": [
        "error",
        { selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']", message: "No insertes HTML: crea elementos de React." },
      ],
    },
  },
  {
    // Las pruebas usan a propósito direcciones y HTML peligrosos (para
    // comprobar que se rechazan) y leen el HTML generado.
    files: ["ui/src/**/*.test.{ts,tsx}", "ui/src/test/**"],
    rules: { "no-script-url": "off", "no-restricted-properties": "off" },
  },
);
