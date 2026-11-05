/*
 * Si la interfaz falla antes de arrancar (p. ej. un error al cargar un módulo),
 * muestra el error en lugar de una ventana vacía. Es un script clásico aparte
 * porque la CSP no permite scripts en línea.
 */
(function () {
  function show(msg) {
    if (window.__dccBooted) return;
    var root = document.getElementById("root");
    if (!root) return;
    root.innerHTML = "";
    var box = document.createElement("div");
    box.setAttribute("role", "alert");
    box.style.cssText = "font:14px system-ui,sans-serif;max-width:720px;margin:48px auto;padding:20px;border:1px solid #bf412e;border-radius:10px;background:#fff;color:#17212c";
    var h = document.createElement("h1");
    h.style.cssText = "font-size:16px;margin:0 0 8px";
    h.textContent = document.documentElement.lang === "en" ? "The interface could not start" : "La interfaz no pudo arrancar";
    var pre = document.createElement("pre");
    pre.style.cssText = "white-space:pre-wrap;user-select:text;font:12px monospace;margin:0";
    pre.textContent = msg;
    box.appendChild(h);
    box.appendChild(pre);
    root.appendChild(box);
  }
  window.addEventListener("error", function (e) {
    show(String(e.message || e.error) + (e.filename ? "\n" + e.filename + ":" + e.lineno + ":" + e.colno : ""));
  });
  window.addEventListener("unhandledrejection", function (e) {
    show(String(e.reason && e.reason.stack ? e.reason.stack : e.reason));
  });
})();
