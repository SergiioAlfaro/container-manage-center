/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

// Tauri espera un puerto fijo en desarrollo.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  root: "ui",
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./ui/src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "safari16",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    root: ".",
    environment: "jsdom",
    // Zona horaria fija: las pruebas de fechas no dependen del equipo.
    env: { TZ: "Europe/Madrid" },
    globals: true,
    setupFiles: ["./ui/src/test/setup.ts"],
    include: ["ui/src/**/*.test.{ts,tsx}"],
    css: false,
  },
});
