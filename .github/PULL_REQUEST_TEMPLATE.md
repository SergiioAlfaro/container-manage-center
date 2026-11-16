## Qué cambia

<!-- Resumen del cambio y por qué. Enlaza el issue si lo hay. -->

## Cómo se ha probado

- [ ] `npm run typecheck`, `npm run lint` y `npm test`
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace`
- [ ] Si toca Docker: `bash scripts/verify-evolution-1.9.1.sh` (o la parte que aplique) en Debian/Ubuntu con Docker
- [ ] Si toca Windows: compila para Windows (`cargo check --target x86_64-pc-windows-gnu` o el flujo `windows-build`) y se indica qué se ha probado en un Windows real
- [ ] Pruebas nuevas para lo que se corrige o se añade

## Lista de comprobación

- [ ] Encabezados SPDX en los archivos nuevos (`python3 scripts/spdx-headers.py --check`)
- [ ] Textos de la interfaz en los 8 idiomas
- [ ] Sin secretos, rutas personales ni datos reales
- [ ] Dependencias nuevas con licencia compatible y avisos actualizados (`python3 scripts/third-party-licenses.py` y `--windows`)
- [ ] Documentación y CHANGELOG actualizados si cambia algo visible

Al enviar este cambio aceptas publicarlo con la licencia GPL-3.0-or-later y el
permiso adicional para WebView2 (CONTRIBUTING.md).
