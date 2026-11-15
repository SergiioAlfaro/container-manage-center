#!/usr/bin/env node
/*
 * Comprobación de desarrollo: carga la interfaz compilada (dist/) en Chromium
 * con Object.prototype congelado, como hace Tauri con `freezePrototype: true`.
 * Detecta dependencias que asignan propiedades
 * heredadas (p. ej. `obj.toString = …`), que dejan la ventana en blanco.
 *
 * Uso: npm run build && node scripts/dev/check-frozen-prototype.cjs
 * Requiere el paquete `playwright` con Chromium instalado.
 */
const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");

const dist = path.resolve(__dirname, "../../dist");
const types = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".woff2": "font/woff2", ".woff": "font/woff", ".svg": "image/svg+xml", ".png": "image/png" };
const server = http.createServer((req, res) => {
  const file = path.join(dist, decodeURIComponent(req.url.split("?")[0]) === "/" ? "index.html" : decodeURIComponent(req.url.split("?")[0]));
  if (!file.startsWith(dist) || !fs.existsSync(file)) return res.writeHead(404).end();
  res.writeHead(200, { "content-type": types[path.extname(file)] || "application/octet-stream" }).end(fs.readFileSync(file));
});

server.listen(0, "127.0.0.1", async () => {
  const url = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.addInitScript(() => Object.freeze(Object.prototype));
  await page.goto(url);
  await page.waitForTimeout(1500);
  const booted = await page.evaluate(() => window.__cmcBooted === true && document.querySelector("nav, aside") !== null);
  await browser.close();
  server.close();
  console.log(`interfaz arrancada: ${booted}`);
  console.log(`errores: ${errors.length ? errors.join(" | ") : "ninguno"}`);
  process.exit(booted && errors.length === 0 ? 0 : 1);
});
