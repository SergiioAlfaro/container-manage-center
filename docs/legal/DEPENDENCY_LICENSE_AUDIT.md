# Auditoría de licencias de dependencias · Container Manage Center 1.10.0

Generada por `scripts/third-party-licenses.py` a partir de `Cargo.lock` y
`package-lock.json` (sin conexión). La licencia del proyecto es **GPL-3.0-or-later**.
Las conclusiones sobre compatibilidad son técnicas, no asesoramiento jurídico.

## 1. Alcance

| Ecosistema | Analizados | Se distribuyen en el paquete |
|---|---|---|
| Rust (`Cargo.lock`, Linux x86_64) | 449 crates de terceros | **428** enlazadas en el programa; 20 solo de compilación y 1 solo de pruebas no se distribuyen |
| npm (`package-lock.json`) | 356 paquetes | **20** de producción incluidos en la interfaz (con las tipografías); 336 de desarrollo no se distribuyen |
| Sistema (Debian) | dependencias del paquete | Ninguna: `libwebkit2gtk-4.1-0`, `libgtk-3-0` y `libc6` las instala apt y se enlazan dinámicamente; `openssh-client` es recomendada |
| Recursos | logotipo e iconos de la aplicación, iconos de la interfaz, tipografías | Logotipo e iconos de la aplicación: propios del proyecto. Iconos de la interfaz: lucide-react (ISC, con partes de Feather, MIT). Tipografías: IBM Plex Sans y JetBrains Mono (OFL-1.1) |
| Herramientas externas | Docker Engine, Docker Compose, OpenSSH, Trivy/Grype (opcionales), systemd | No se incluyen: se usan las del sistema o del servidor |

## 2. Resultado (componentes que se distribuyen)

| Clase | Componentes | Significado |
|---|---|---|
| 1. Compatibles | 441 | Licencias permisivas compatibles con GPL-3.0-or-later. Obligan a conservar su aviso y su licencia: están completos en `THIRD_PARTY_NOTICES.md` |
| 2. Compatibles bajo condiciones | 7 | MPL-2.0 y OFL-1.1 (condiciones abajo). Apache-2.0 sola es compatible con GPL v3 (no lo sería con GPL v2) y se cuenta en la clase 1 |
| 3. Requieren avisos o materiales adicionales | 11 (también contados en la clase 1 o 2) | Sin archivo de licencia propio en su paquete: se reproduce el texto estándar de su licencia declarada con sus autores declarados. Ningún componente trae archivo NOTICE de Apache |
| 4. Pendientes de revisión | 0 | — |
| 5. Potencialmente incompatibles | 0 | — |
| 6. Licencia no determinable | 0 | — |

### Condiciones

- **MPL-2.0** (cssparser 0.37.0, cssparser-macros 0.7.1, dtoa-short 0.3.5, option-ext 0.2.0, selectors 0.38.0): copyleft por archivo: se conservan sus avisos y su código fuente está disponible (crates.io y el .tar.gz con Cargo.lock). Ningún archivo lleva el aviso «Incompatible With Secondary Licenses», así que la cláusula 3.3 permite combinarlo con GPL-3.0-or-later.
- **OFL-1.1** (@fontsource/ibm-plex-sans 5.3.0, @fontsource/jetbrains-mono 5.3.0): tipografías: se distribuyen con el programa conservando su licencia y sus avisos; no se venden por separado ni se modifican (no se usa su nombre reservado en versiones modificadas).

### Comprobaciones hechas a mano

- **MPL-2.0**: archivos de código (`*.rs`) con el aviso «Incompatible With Secondary Licenses» (Exhibit B): cssparser 0.37.0: 0, cssparser-macros 0.7.1: 0, dtoa-short 0.3.5: 0, option-ext 0.2.0: 0, selectors 0.38.0: 0.
- **ring 0.17.14** (Apache-2.0 AND ISC): sus archivos `LICENSE`, `LICENSE-BoringSSL` y `LICENSE-other-bits` solo contienen Apache-2.0, ISC y MIT (once_cell); no queda código bajo la licencia OpenSSL/SSLeay, que sería incompatible con la GPL.
- **lucide-react** (ISC): su `LICENSE` reconoce partes de Feather (MIT, Cole Bemis); el aviso se conserva.
- **Complementos de Tauri** (`@tauri-apps/plugin-*`): solo traen `LICENSE.spdx` (MIT OR Apache-2.0); se reproducen los textos de `@tauri-apps/api`, del mismo proyecto y titulares.
- **Bibliotecas del sistema**: no se redistribuyen; se enlazan dinámicamente desde Debian.

### Componentes de la clase 3 (texto estándar)

- alloc-stdlib 0.3.0 (BSD-3-Clause)
- bollard-stubs 1.53.1-rc.29.3.1 (Apache-2.0)
- dlopen2 0.8.2 (MIT)
- dlopen2_derive 0.4.3 (MIT)
- libappindicator-sys 0.9.0 (Apache-2.0 OR MIT)
- selectors 0.38.0 (MPL-2.0)
- ts-rs 10.1.0 (MIT)
- ts-rs-macros 10.1.0 (MIT)
- @tauri-apps/plugin-clipboard-manager 2.5.0 (MIT OR Apache-2.0)
- @tauri-apps/plugin-dialog 2.8.4 (MIT OR Apache-2.0)
- @tauri-apps/plugin-opener 2.7.2 (MIT OR Apache-2.0)

## 3. Licencias de los componentes distribuidos

| Expresión de licencia | Componentes |
|---|---|
| MIT OR Apache-2.0 | 197 |
| MIT | 127 |
| Apache-2.0 OR MIT | 50 |
| Unicode-3.0 | 18 |
| Unlicense OR MIT | 9 |
| Apache-2.0 | 5 |
| MPL-2.0 | 5 |
| ISC | 4 |
| MIT OR Apache-2.0 OR Zlib | 4 |
| Apache-2.0 OR ISC OR MIT | 3 |
| BSD-3-Clause | 3 |
| Zlib | 3 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 2 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 |
| BSD-3-Clause OR Apache-2.0 | 2 |
| MIT OR Zlib OR Apache-2.0 | 2 |
| OFL-1.1 | 2 |
| Zlib OR Apache-2.0 OR MIT | 2 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| Apache-2.0 AND ISC | 1 |
| Apache-2.0 AND MIT | 1 |
| Apache-2.0 OR BSL-1.0 | 1 |
| BSD-3-Clause AND MIT | 1 |
| BSD-3-Clause OR MIT | 1 |
| CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |

## 4. Herramientas y límites

- `cargo metadata --offline` (grafo de dependencias para Linux x86_64) y `cargo deny check licenses` (política de `deny.toml`, que forma parte de la verificación).
- Lectura de los archivos `LICENSE*`, `COPYING*` y `NOTICE*` de cada componente en el registro local de Cargo y en `node_modules`.
- No se ha usado `cargo-about` ni herramientas de SBOM (no instaladas; decisión del propietario).
- La licencia de cada crate es la que declara su autor en crates.io; no se ha revisado archivo por archivo salvo en las comprobaciones de arriba.
- Las conclusiones son técnicas; no sustituyen a un asesoramiento jurídico.

## 5. Inventario: se distribuyen

| Componente | Versión | Ecosistema | Licencia | Clase |
|---|---|---|---|---|
| adler2 | 2.0.1 | Rust | 0BSD OR MIT OR Apache-2.0 | 1 |
| aes | 0.9.4 | Rust | MIT OR Apache-2.0 | 1 |
| aho-corasick | 1.1.5 | Rust | Unlicense OR MIT | 1 |
| alloc-no-stdlib | 3.0.0 | Rust | BSD-3-Clause | 1 |
| alloc-stdlib | 0.3.0 | Rust | BSD-3-Clause | 1 · 3 |
| anyhow | 1.0.104 | Rust | MIT OR Apache-2.0 | 1 |
| arboard | 3.7.0 | Rust | MIT OR Apache-2.0 | 1 |
| async-broadcast | 0.7.2 | Rust | MIT OR Apache-2.0 | 1 |
| async-channel | 2.5.0 | Rust | Apache-2.0 OR MIT | 1 |
| async-executor | 1.14.1 | Rust | Apache-2.0 OR MIT | 1 |
| async-io | 2.6.0 | Rust | Apache-2.0 OR MIT | 1 |
| async-lock | 3.5.0 | Rust | Apache-2.0 OR MIT | 1 |
| async-process | 2.5.0 | Rust | Apache-2.0 OR MIT | 1 |
| async-recursion | 1.2.0 | Rust | MIT OR Apache-2.0 | 1 |
| async-signal | 0.2.14 | Rust | Apache-2.0 OR MIT | 1 |
| async-task | 4.7.1 | Rust | Apache-2.0 OR MIT | 1 |
| async-trait | 0.1.92 | Rust | MIT OR Apache-2.0 | 1 |
| atk | 0.18.2 | Rust | MIT | 1 |
| atk-sys | 0.18.2 | Rust | MIT | 1 |
| atomic-waker | 1.1.2 | Rust | Apache-2.0 OR MIT | 1 |
| base64 | 0.22.1 | Rust | MIT OR Apache-2.0 | 1 |
| base64 | 0.23.1 | Rust | MIT OR Apache-2.0 | 1 |
| bit-set | 0.8.0 | Rust | Apache-2.0 OR MIT | 1 |
| bit-vec | 0.8.0 | Rust | Apache-2.0 OR MIT | 1 |
| bitflags | 1.3.2 | Rust | MIT/Apache-2.0 | 1 |
| bitflags | 2.13.2 | Rust | MIT OR Apache-2.0 | 1 |
| block-buffer | 0.10.4 | Rust | MIT OR Apache-2.0 | 1 |
| block-buffer | 0.12.1 | Rust | MIT OR Apache-2.0 | 1 |
| block-padding | 0.4.2 | Rust | MIT OR Apache-2.0 | 1 |
| blocking | 1.7.0 | Rust | Apache-2.0 OR MIT | 1 |
| bollard | 0.21.1 | Rust | Apache-2.0 | 1 |
| bollard-stubs | 1.53.1-rc.29.3.1 | Rust | Apache-2.0 | 1 · 3 |
| brotli | 9.1.1 | Rust | BSD-3-Clause AND MIT | 1 |
| brotli-decompressor | 6.2.0 | Rust | BSD-3-Clause/MIT | 1 |
| bs58 | 0.5.1 | Rust | MIT/Apache-2.0 | 1 |
| bytemuck | 1.25.2 | Rust | Zlib OR Apache-2.0 OR MIT | 1 |
| byteorder | 1.5.0 | Rust | Unlicense OR MIT | 1 |
| byteorder-lite | 0.1.0 | Rust | Unlicense OR MIT | 1 |
| bytes | 1.12.1 | Rust | MIT | 1 |
| cairo-rs | 0.18.5 | Rust | MIT | 1 |
| cairo-sys-rs | 0.18.2 | Rust | MIT | 1 |
| camino | 1.2.6 | Rust | MIT OR Apache-2.0 | 1 |
| cargo-platform | 0.1.9 | Rust | MIT OR Apache-2.0 | 1 |
| cargo_metadata | 0.19.2 | Rust | MIT | 1 |
| cbc | 0.2.1 | Rust | MIT OR Apache-2.0 | 1 |
| cfb | 0.14.0 | Rust | MIT | 1 |
| cfg-if | 1.0.5 | Rust | MIT OR Apache-2.0 | 1 |
| chacha20 | 0.10.2 | Rust | MIT OR Apache-2.0 | 1 |
| chrono | 0.4.45 | Rust | MIT OR Apache-2.0 | 1 |
| cipher | 0.5.2 | Rust | MIT OR Apache-2.0 | 1 |
| cmov | 0.5.4 | Rust | Apache-2.0 OR MIT | 1 |
| concurrent-queue | 2.5.0 | Rust | Apache-2.0 OR MIT | 1 |
| const-oid | 0.10.3 | Rust | Apache-2.0 OR MIT | 1 |
| cookie | 0.18.2 | Rust | MIT OR Apache-2.0 | 1 |
| cpubits | 0.1.1 | Rust | MIT OR Apache-2.0 | 1 |
| cpufeatures | 0.2.17 | Rust | MIT OR Apache-2.0 | 1 |
| cpufeatures | 0.3.1 | Rust | MIT OR Apache-2.0 | 1 |
| crc32fast | 1.5.4 | Rust | MIT OR Apache-2.0 | 1 |
| crossbeam-channel | 0.5.17 | Rust | MIT OR Apache-2.0 | 1 |
| crossbeam-utils | 0.8.23 | Rust | MIT OR Apache-2.0 | 1 |
| crypto-common | 0.1.7 | Rust | MIT OR Apache-2.0 | 1 |
| crypto-common | 0.2.2 | Rust | MIT OR Apache-2.0 | 1 |
| cssparser | 0.37.0 | Rust | MPL-2.0 | 2 (MPL-2.0) |
| cssparser-macros | 0.7.1 | Rust | MPL-2.0 | 2 (MPL-2.0) |
| ctor | 1.0.13 | Rust | Apache-2.0 OR MIT | 1 |
| ctutils | 0.4.8 | Rust | Apache-2.0 OR MIT | 1 |
| darling | 0.24.1 | Rust | MIT | 1 |
| darling_core | 0.24.1 | Rust | MIT | 1 |
| darling_macro | 0.24.1 | Rust | MIT | 1 |
| dbus | 0.9.12 | Rust | Apache-2.0/MIT | 1 |
| defmt | 1.2.0 | Rust | MIT OR Apache-2.0 | 1 |
| defmt-macros | 1.2.0 | Rust | MIT OR Apache-2.0 | 1 |
| defmt-parser | 1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| deranged | 0.5.9 | Rust | MIT OR Apache-2.0 | 1 |
| derive_more | 2.1.1 | Rust | MIT | 1 |
| derive_more-impl | 2.1.1 | Rust | MIT | 1 |
| digest | 0.10.7 | Rust | MIT OR Apache-2.0 | 1 |
| digest | 0.11.3 | Rust | MIT OR Apache-2.0 | 1 |
| dirs | 6.0.0 | Rust | MIT OR Apache-2.0 | 1 |
| dirs | 7.0.0 | Rust | MIT OR Apache-2.0 | 1 |
| dirs-sys | 0.5.0 | Rust | MIT OR Apache-2.0 | 1 |
| displaydoc | 0.2.7 | Rust | MIT OR Apache-2.0 | 1 |
| dlopen2 | 0.8.2 | Rust | MIT | 1 · 3 |
| dlopen2_derive | 0.4.3 | Rust | MIT | 1 · 3 |
| dom_query | 0.28.0 | Rust | MIT | 1 |
| downcast-rs | 1.2.1 | Rust | MIT/Apache-2.0 | 1 |
| dpi | 0.1.2 | Rust | Apache-2.0 AND MIT | 1 |
| dtoa | 1.0.11 | Rust | MIT OR Apache-2.0 | 1 |
| dtoa-short | 0.3.5 | Rust | MPL-2.0 | 2 (MPL-2.0) |
| dunce | 1.0.5 | Rust | CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |
| dyn-clone | 1.0.20 | Rust | MIT OR Apache-2.0 | 1 |
| endi | 1.1.1 | Rust | MIT | 1 |
| enumflags2 | 0.7.12 | Rust | MIT OR Apache-2.0 | 1 |
| enumflags2_derive | 0.7.12 | Rust | MIT OR Apache-2.0 | 1 |
| equivalent | 1.0.3 | Rust | Apache-2.0 OR MIT | 1 |
| erased-serde | 0.4.11 | Rust | MIT OR Apache-2.0 | 1 |
| errno | 0.3.14 | Rust | MIT OR Apache-2.0 | 1 |
| event-listener | 5.4.2 | Rust | Apache-2.0 OR MIT | 1 |
| event-listener-strategy | 0.5.4 | Rust | Apache-2.0 OR MIT | 1 |
| fastrand | 2.5.0 | Rust | Apache-2.0 OR MIT | 1 |
| fax | 0.2.7 | Rust | MIT | 1 |
| fdeflate | 0.3.7 | Rust | MIT OR Apache-2.0 | 1 |
| field-offset | 0.3.6 | Rust | MIT OR Apache-2.0 | 1 |
| filetime | 0.2.29 | Rust | MIT/Apache-2.0 | 1 |
| fixedbitset | 0.5.7 | Rust | MIT OR Apache-2.0 | 1 |
| flate2 | 1.1.10 | Rust | MIT OR Apache-2.0 | 1 |
| fnv | 1.0.7 | Rust | Apache-2.0 / MIT | 1 |
| foldhash | 0.1.5 | Rust | Zlib | 1 |
| foldhash | 0.2.0 | Rust | Zlib | 1 |
| form_urlencoded | 1.2.2 | Rust | MIT OR Apache-2.0 | 1 |
| futures-channel | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| futures-core | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| futures-executor | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| futures-io | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| futures-lite | 2.6.1 | Rust | Apache-2.0 OR MIT | 1 |
| futures-macro | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| futures-sink | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| futures-task | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| futures-util | 0.3.34 | Rust | MIT OR Apache-2.0 | 1 |
| gdk | 0.18.2 | Rust | MIT | 1 |
| gdk-pixbuf | 0.18.5 | Rust | MIT | 1 |
| gdk-pixbuf-sys | 0.18.0 | Rust | MIT | 1 |
| gdk-sys | 0.18.2 | Rust | MIT | 1 |
| gdkwayland-sys | 0.18.2 | Rust | MIT | 1 |
| gdkx11 | 0.18.2 | Rust | MIT | 1 |
| gdkx11-sys | 0.18.2 | Rust | MIT | 1 |
| generic-array | 0.14.7 | Rust | MIT | 1 |
| gethostname | 1.1.0 | Rust | Apache-2.0 | 1 |
| getrandom | 0.2.17 | Rust | MIT OR Apache-2.0 | 1 |
| getrandom | 0.3.4 | Rust | MIT OR Apache-2.0 | 1 |
| getrandom | 0.4.3 | Rust | MIT OR Apache-2.0 | 1 |
| gio | 0.18.4 | Rust | MIT | 1 |
| gio-sys | 0.18.1 | Rust | MIT | 1 |
| glib | 0.18.5 | Rust | MIT | 1 |
| glib-macros | 0.18.5 | Rust | MIT | 1 |
| glib-sys | 0.18.1 | Rust | MIT | 1 |
| glob | 0.3.4 | Rust | MIT OR Apache-2.0 | 1 |
| gobject-sys | 0.18.0 | Rust | MIT | 1 |
| gtk | 0.18.2 | Rust | MIT | 1 |
| gtk-sys | 0.18.2 | Rust | MIT | 1 |
| gtk3-macros | 0.18.2 | Rust | MIT | 1 |
| half | 2.7.1 | Rust | MIT OR Apache-2.0 | 1 |
| hashbrown | 0.12.3 | Rust | MIT OR Apache-2.0 | 1 |
| hashbrown | 0.15.5 | Rust | MIT OR Apache-2.0 | 1 |
| hashbrown | 0.17.1 | Rust | MIT OR Apache-2.0 | 1 |
| heck | 0.4.1 | Rust | MIT OR Apache-2.0 | 1 |
| heck | 0.5.0 | Rust | MIT OR Apache-2.0 | 1 |
| hex | 0.4.3 | Rust | MIT OR Apache-2.0 | 1 |
| hkdf | 0.13.0 | Rust | MIT OR Apache-2.0 | 1 |
| hmac | 0.13.0 | Rust | MIT OR Apache-2.0 | 1 |
| html5ever | 0.39.0 | Rust | MIT OR Apache-2.0 | 1 |
| http | 1.5.0 | Rust | MIT OR Apache-2.0 | 1 |
| http-body | 1.1.0 | Rust | MIT | 1 |
| http-body-util | 0.1.5 | Rust | MIT | 1 |
| httparse | 1.10.1 | Rust | MIT OR Apache-2.0 | 1 |
| httpdate | 1.0.3 | Rust | MIT OR Apache-2.0 | 1 |
| hybrid-array | 0.4.15 | Rust | MIT OR Apache-2.0 | 1 |
| hyper | 1.12.0 | Rust | MIT | 1 |
| hyper-rustls | 0.27.10 | Rust | Apache-2.0 OR ISC OR MIT | 1 |
| hyper-util | 0.1.21 | Rust | MIT | 1 |
| hyperlocal | 0.9.1 | Rust | MIT | 1 |
| iana-time-zone | 0.1.65 | Rust | MIT OR Apache-2.0 | 1 |
| ico | 0.6.0 | Rust | MIT | 1 |
| icu_collections | 2.4.0 | Rust | Unicode-3.0 | 1 |
| icu_locale_core | 2.4.0 | Rust | Unicode-3.0 | 1 |
| icu_normalizer | 2.4.0 | Rust | Unicode-3.0 | 1 |
| icu_normalizer_data | 2.4.0 | Rust | Unicode-3.0 | 1 |
| icu_properties | 2.4.0 | Rust | Unicode-3.0 | 1 |
| icu_properties_data | 2.4.0 | Rust | Unicode-3.0 | 1 |
| icu_provider | 2.4.0 | Rust | Unicode-3.0 | 1 |
| ident_case | 1.0.1 | Rust | MIT/Apache-2.0 | 1 |
| idna | 1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| idna_adapter | 1.2.2 | Rust | Apache-2.0 OR MIT | 1 |
| image | 0.25.10 | Rust | MIT OR Apache-2.0 | 1 |
| indexmap | 1.9.3 | Rust | Apache-2.0 OR MIT | 1 |
| indexmap | 2.14.2 | Rust | Apache-2.0 OR MIT | 1 |
| infer | 0.22.0 | Rust | MIT | 1 |
| inout | 0.2.6 | Rust | MIT OR Apache-2.0 | 1 |
| ipnet | 2.12.2 | Rust | MIT OR Apache-2.0 | 1 |
| is-docker | 0.2.0 | Rust | MIT | 1 |
| is-wsl | 0.4.0 | Rust | MIT | 1 |
| itoa | 1.0.18 | Rust | MIT OR Apache-2.0 | 1 |
| javascriptcore-rs | 1.1.2 | Rust | MIT | 1 |
| javascriptcore-rs-sys | 1.1.1 | Rust | MIT | 1 |
| jiff | 0.2.39 | Rust | Unlicense OR MIT | 1 |
| jiff-core | 0.1.2 | Rust | Unlicense OR MIT | 1 |
| json-patch | 4.2.0 | Rust | MIT/Apache-2.0 | 1 |
| jsonptr | 0.7.1 | Rust | MIT OR Apache-2.0 | 1 |
| keyboard-types | 0.8.3 | Rust | MIT OR Apache-2.0 | 1 |
| lazy_static | 1.5.1 | Rust | MIT OR Apache-2.0 | 1 |
| libappindicator | 0.9.0 | Rust | Apache-2.0 OR MIT | 1 |
| libappindicator-sys | 0.9.0 | Rust | Apache-2.0 OR MIT | 1 · 3 |
| libc | 0.2.191 | Rust | MIT OR Apache-2.0 | 1 |
| libdbus-sys | 0.2.7 | Rust | Apache-2.0/MIT | 1 |
| libloading | 0.7.4 | Rust | ISC | 1 |
| linux-raw-sys | 0.12.1 | Rust | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 1 |
| litemap | 0.8.3 | Rust | Unicode-3.0 | 1 |
| lock_api | 0.4.14 | Rust | MIT OR Apache-2.0 | 1 |
| log | 0.4.35 | Rust | MIT OR Apache-2.0 | 1 |
| lru-slab | 0.1.3 | Rust | MIT OR Apache-2.0 OR Zlib | 1 |
| markup5ever | 0.39.0 | Rust | MIT OR Apache-2.0 | 1 |
| matchers | 0.2.0 | Rust | MIT | 1 |
| memchr | 2.8.3 | Rust | Unlicense OR MIT | 1 |
| memoffset | 0.9.1 | Rust | MIT | 1 |
| mime | 0.3.17 | Rust | MIT OR Apache-2.0 | 1 |
| miniz_oxide | 0.8.9 | Rust | MIT OR Zlib OR Apache-2.0 | 1 |
| miniz_oxide | 0.9.1 | Rust | MIT OR Zlib OR Apache-2.0 | 1 |
| mio | 1.2.4 | Rust | MIT | 1 |
| moxcms | 0.8.1 | Rust | BSD-3-Clause OR Apache-2.0 | 1 |
| muda | 0.20.0 | Rust | Apache-2.0 OR MIT | 1 |
| new_debug_unreachable | 1.0.6 | Rust | MIT | 1 |
| nom | 8.0.0 | Rust | MIT | 1 |
| nu-ansi-term | 0.50.3 | Rust | MIT | 1 |
| num | 0.4.3 | Rust | MIT OR Apache-2.0 | 1 |
| num-bigint | 0.4.8 | Rust | MIT OR Apache-2.0 | 1 |
| num-complex | 0.4.6 | Rust | MIT OR Apache-2.0 | 1 |
| num-conv | 0.2.2 | Rust | MIT OR Apache-2.0 | 1 |
| num-integer | 0.1.47 | Rust | MIT OR Apache-2.0 | 1 |
| num-iter | 0.1.46 | Rust | MIT OR Apache-2.0 | 1 |
| num-rational | 0.4.2 | Rust | MIT OR Apache-2.0 | 1 |
| num-traits | 0.2.19 | Rust | MIT OR Apache-2.0 | 1 |
| once_cell | 1.21.4 | Rust | MIT OR Apache-2.0 | 1 |
| open | 5.4.5 | Rust | MIT | 1 |
| openssl-probe | 0.2.2 | Rust | MIT OR Apache-2.0 | 1 |
| option-ext | 0.2.0 | Rust | MPL-2.0 | 2 (MPL-2.0) |
| ordered-stream | 0.2.0 | Rust | MIT OR Apache-2.0 | 1 |
| os_pipe | 1.2.3 | Rust | MIT | 1 |
| pango | 0.18.3 | Rust | MIT | 1 |
| pango-sys | 0.18.0 | Rust | MIT | 1 |
| parking | 2.2.1 | Rust | Apache-2.0 OR MIT | 1 |
| parking_lot | 0.12.5 | Rust | MIT OR Apache-2.0 | 1 |
| parking_lot_core | 0.9.12 | Rust | MIT OR Apache-2.0 | 1 |
| percent-encoding | 2.3.2 | Rust | MIT OR Apache-2.0 | 1 |
| petgraph | 0.8.3 | Rust | MIT OR Apache-2.0 | 1 |
| phf | 0.13.1 | Rust | MIT | 1 |
| phf_generator | 0.13.1 | Rust | MIT | 1 |
| phf_macros | 0.13.1 | Rust | MIT | 1 |
| phf_shared | 0.13.1 | Rust | MIT | 1 |
| pin-project-lite | 0.2.17 | Rust | Apache-2.0 OR MIT | 1 |
| piper | 0.2.5 | Rust | MIT OR Apache-2.0 | 1 |
| plist | 1.10.3 | Rust | MIT | 1 |
| png | 0.18.1 | Rust | MIT OR Apache-2.0 | 1 |
| polling | 3.11.0 | Rust | Apache-2.0 OR MIT | 1 |
| potential_utf | 0.1.6 | Rust | Unicode-3.0 | 1 |
| powerfmt | 0.2.1 | Rust | MIT OR Apache-2.0 | 1 |
| precomputed-hash | 0.1.1 | Rust | MIT | 1 |
| proc-macro-crate | 1.3.1 | Rust | MIT OR Apache-2.0 | 1 |
| proc-macro-crate | 2.0.2 | Rust | MIT OR Apache-2.0 | 1 |
| proc-macro-crate | 3.5.0 | Rust | MIT OR Apache-2.0 | 1 |
| proc-macro-error | 1.0.4 | Rust | MIT OR Apache-2.0 | 1 |
| proc-macro-error-attr | 1.0.4 | Rust | MIT OR Apache-2.0 | 1 |
| proc-macro2 | 1.0.107 | Rust | MIT OR Apache-2.0 | 1 |
| pxfm | 0.1.30 | Rust | BSD-3-Clause OR Apache-2.0 | 1 |
| quick-error | 2.0.1 | Rust | MIT/Apache-2.0 | 1 |
| quick-xml | 0.41.0 | Rust | MIT | 1 |
| quick-xml | 0.42.0 | Rust | MIT | 1 |
| quinn | 0.11.12 | Rust | MIT OR Apache-2.0 | 1 |
| quinn-proto | 0.11.19 | Rust | MIT OR Apache-2.0 | 1 |
| quinn-udp | 0.5.16 | Rust | MIT OR Apache-2.0 | 1 |
| quote | 1.0.47 | Rust | MIT OR Apache-2.0 | 1 |
| rand | 0.10.3 | Rust | MIT OR Apache-2.0 | 1 |
| rand_core | 0.10.1 | Rust | MIT OR Apache-2.0 | 1 |
| rand_pcg | 0.10.2 | Rust | MIT OR Apache-2.0 | 1 |
| raw-window-handle | 0.6.2 | Rust | MIT OR Apache-2.0 OR Zlib | 1 |
| ref-cast | 1.0.27 | Rust | MIT OR Apache-2.0 | 1 |
| ref-cast-impl | 1.0.27 | Rust | MIT OR Apache-2.0 | 1 |
| regex | 1.13.1 | Rust | MIT OR Apache-2.0 | 1 |
| regex-automata | 0.4.18 | Rust | MIT OR Apache-2.0 | 1 |
| regex-syntax | 0.8.11 | Rust | MIT OR Apache-2.0 | 1 |
| reqwest | 0.12.28 | Rust | MIT OR Apache-2.0 | 1 |
| rfd | 0.17.2 | Rust | MIT | 1 |
| ring | 0.17.14 | Rust | Apache-2.0 AND ISC | 1 |
| rustc-hash | 2.1.3 | Rust | Apache-2.0 OR MIT | 1 |
| rustix | 1.1.6 | Rust | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 1 |
| rustls | 0.23.46 | Rust | Apache-2.0 OR ISC OR MIT | 1 |
| rustls-native-certs | 0.8.5 | Rust | Apache-2.0 OR ISC OR MIT | 1 |
| rustls-pki-types | 1.15.1 | Rust | MIT OR Apache-2.0 | 1 |
| rustls-webpki | 0.103.16 | Rust | ISC | 1 |
| ryu | 1.0.23 | Rust | Apache-2.0 OR BSL-1.0 | 1 |
| same-file | 1.0.6 | Rust | Unlicense/MIT | 1 |
| schemars | 0.8.22 | Rust | MIT | 1 |
| schemars | 0.9.0 | Rust | MIT | 1 |
| schemars | 1.2.3 | Rust | MIT | 1 |
| schemars_derive | 0.8.22 | Rust | MIT | 1 |
| scopeguard | 1.2.0 | Rust | MIT OR Apache-2.0 | 1 |
| secret-service | 5.2.0 | Rust | MIT OR Apache-2.0 | 1 |
| selectors | 0.38.0 | Rust | MPL-2.0 | 2 · 3 (MPL-2.0) |
| semver | 1.0.28 | Rust | MIT OR Apache-2.0 | 1 |
| serde | 1.0.229 | Rust | MIT OR Apache-2.0 | 1 |
| serde-untagged | 0.1.9 | Rust | MIT OR Apache-2.0 | 1 |
| serde_core | 1.0.229 | Rust | MIT OR Apache-2.0 | 1 |
| serde_derive | 1.0.229 | Rust | MIT OR Apache-2.0 | 1 |
| serde_derive_internals | 0.29.1 | Rust | MIT OR Apache-2.0 | 1 |
| serde_json | 1.0.154 | Rust | MIT OR Apache-2.0 | 1 |
| serde_repr | 0.1.21 | Rust | MIT OR Apache-2.0 | 1 |
| serde_spanned | 0.6.9 | Rust | MIT OR Apache-2.0 | 1 |
| serde_spanned | 1.1.2 | Rust | MIT OR Apache-2.0 | 1 |
| serde_urlencoded | 0.7.1 | Rust | MIT/Apache-2.0 | 1 |
| serde_with | 3.24.0 | Rust | MIT OR Apache-2.0 | 1 |
| serde_with_macros | 3.24.0 | Rust | MIT OR Apache-2.0 | 1 |
| serialize-to-javascript | 0.1.2 | Rust | MIT OR Apache-2.0 | 1 |
| serialize-to-javascript-impl | 0.1.2 | Rust | MIT OR Apache-2.0 | 1 |
| servo_arc | 0.4.3 | Rust | MIT OR Apache-2.0 | 1 |
| sha2 | 0.10.9 | Rust | MIT OR Apache-2.0 | 1 |
| sha2 | 0.11.1 | Rust | MIT OR Apache-2.0 | 1 |
| sharded-slab | 0.1.7 | Rust | MIT | 1 |
| signal-hook-registry | 1.4.8 | Rust | MIT OR Apache-2.0 | 1 |
| simd-adler32 | 0.3.10 | Rust | MIT | 1 |
| siphasher | 1.0.4 | Rust | MIT OR Apache-2.0 | 1 |
| slab | 0.4.12 | Rust | MIT | 1 |
| smallvec | 1.16.4 | Rust | MIT OR Apache-2.0 | 1 |
| socket2 | 0.6.5 | Rust | MIT OR Apache-2.0 | 1 |
| soup3 | 0.5.0 | Rust | MIT | 1 |
| soup3-sys | 0.5.0 | Rust | MIT | 1 |
| stable_deref_trait | 1.2.1 | Rust | MIT OR Apache-2.0 | 1 |
| string_cache | 0.9.0 | Rust | MIT OR Apache-2.0 | 1 |
| strsim | 0.11.1 | Rust | MIT | 1 |
| subtle | 2.6.1 | Rust | BSD-3-Clause | 1 |
| syn | 1.0.109 | Rust | MIT OR Apache-2.0 | 1 |
| syn | 2.0.119 | Rust | MIT OR Apache-2.0 | 1 |
| syn | 3.0.9 | Rust | MIT OR Apache-2.0 | 1 |
| sync_wrapper | 1.0.2 | Rust | Apache-2.0 | 1 |
| synstructure | 0.14.0 | Rust | MIT | 1 |
| tao | 0.37.2 | Rust | Apache-2.0 | 1 |
| tar | 0.4.46 | Rust | MIT OR Apache-2.0 | 1 |
| tauri | 2.12.5 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-codegen | 2.7.4 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-macros | 2.7.4 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-plugin-clipboard-manager | 2.5.0 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-plugin-dialog | 2.8.4 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-plugin-fs | 2.7.1 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-plugin-opener | 2.7.2 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-runtime | 2.12.2 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-runtime-wry | 2.12.3 | Rust | Apache-2.0 OR MIT | 1 |
| tauri-utils | 2.10.2 | Rust | Apache-2.0 OR MIT | 1 |
| tendril | 0.5.1 | Rust | MIT OR Apache-2.0 | 1 |
| termcolor | 1.4.1 | Rust | Unlicense OR MIT | 1 |
| thiserror | 1.0.69 | Rust | MIT OR Apache-2.0 | 1 |
| thiserror | 2.0.21 | Rust | MIT OR Apache-2.0 | 1 |
| thiserror-impl | 1.0.69 | Rust | MIT OR Apache-2.0 | 1 |
| thiserror-impl | 2.0.21 | Rust | MIT OR Apache-2.0 | 1 |
| thread_local | 1.1.10 | Rust | MIT OR Apache-2.0 | 1 |
| tiff | 0.11.3 | Rust | MIT | 1 |
| time | 0.3.55 | Rust | MIT OR Apache-2.0 | 1 |
| time-core | 0.1.9 | Rust | MIT OR Apache-2.0 | 1 |
| time-macros | 0.2.32 | Rust | MIT OR Apache-2.0 | 1 |
| tinystr | 0.8.5 | Rust | Unicode-3.0 | 1 |
| tinyvec | 1.13.3 | Rust | Zlib OR Apache-2.0 OR MIT | 1 |
| tokio | 1.54.1 | Rust | MIT | 1 |
| tokio-macros | 2.7.2 | Rust | MIT | 1 |
| tokio-rustls | 0.26.6 | Rust | MIT OR Apache-2.0 | 1 |
| tokio-util | 0.7.20 | Rust | MIT | 1 |
| toml | 0.8.2 | Rust | MIT OR Apache-2.0 | 1 |
| toml | 1.2.0+spec-1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| toml_datetime | 0.6.3 | Rust | MIT OR Apache-2.0 | 1 |
| toml_datetime | 1.1.2+spec-1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| toml_edit | 0.19.15 | Rust | MIT OR Apache-2.0 | 1 |
| toml_edit | 0.20.2 | Rust | MIT OR Apache-2.0 | 1 |
| toml_edit | 0.25.18+spec-1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| toml_parser | 1.1.5+spec-1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| toml_writer | 1.2.0+spec-1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| tower | 0.5.3 | Rust | MIT | 1 |
| tower-http | 0.6.11 | Rust | MIT | 1 |
| tower-layer | 0.3.3 | Rust | MIT | 1 |
| tower-service | 0.3.3 | Rust | MIT | 1 |
| tracing | 0.1.44 | Rust | MIT | 1 |
| tracing-attributes | 0.1.31 | Rust | MIT | 1 |
| tracing-core | 0.1.36 | Rust | MIT | 1 |
| tracing-log | 0.2.0 | Rust | MIT | 1 |
| tracing-subscriber | 0.3.23 | Rust | MIT | 1 |
| tray-icon | 0.25.1 | Rust | MIT OR Apache-2.0 | 1 |
| tree_magic_mini | 3.2.2 | Rust | MIT | 1 |
| try-lock | 0.2.5 | Rust | MIT | 1 |
| ts-rs | 10.1.0 | Rust | MIT | 1 · 3 |
| ts-rs-macros | 10.1.0 | Rust | MIT | 1 · 3 |
| typeid | 1.0.3 | Rust | MIT OR Apache-2.0 | 1 |
| typenum | 1.20.1 | Rust | MIT OR Apache-2.0 | 1 |
| unicode-ident | 1.0.27 | Rust | (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |
| untrusted | 0.9.0 | Rust | ISC | 1 |
| url | 2.5.8 | Rust | MIT OR Apache-2.0 | 1 |
| urlpattern | 0.6.0 | Rust | MIT | 1 |
| utf8_iter | 1.1.0 | Rust | Apache-2.0 OR MIT | 1 |
| utf_types | 1.0.0 | Rust | Apache-2.0 OR MIT | 1 |
| uuid | 1.29.0 | Rust | Apache-2.0 OR MIT | 1 |
| walkdir | 2.5.0 | Rust | Unlicense/MIT | 1 |
| want | 0.3.2 | Rust | MIT | 1 |
| wayland-backend | 0.3.17 | Rust | MIT | 1 |
| wayland-client | 0.31.15 | Rust | MIT | 1 |
| wayland-protocols | 0.32.13 | Rust | MIT | 1 |
| wayland-protocols-wlr | 0.3.12 | Rust | MIT | 1 |
| wayland-scanner | 0.31.11 | Rust | MIT | 1 |
| wayland-sys | 0.31.11 | Rust | MIT | 1 |
| web-time | 1.1.0 | Rust | MIT OR Apache-2.0 | 1 |
| web_atoms | 0.2.6 | Rust | MIT OR Apache-2.0 | 1 |
| webkit2gtk | 2.0.2 | Rust | MIT | 1 |
| webkit2gtk-sys | 2.0.2 | Rust | MIT | 1 |
| weezl | 0.1.12 | Rust | MIT OR Apache-2.0 | 1 |
| winnow | 0.5.40 | Rust | MIT | 1 |
| winnow | 1.0.4 | Rust | MIT | 1 |
| wl-clipboard-rs | 0.9.5 | Rust | MIT OR Apache-2.0 | 1 |
| writeable | 0.6.4 | Rust | Unicode-3.0 | 1 |
| wry | 0.57.1 | Rust | Apache-2.0 OR MIT | 1 |
| x11 | 2.21.0 | Rust | MIT | 1 |
| x11-dl | 2.21.0 | Rust | MIT | 1 |
| x11rb | 0.13.2 | Rust | MIT OR Apache-2.0 | 1 |
| x11rb-protocol | 0.13.2 | Rust | MIT OR Apache-2.0 | 1 |
| xattr | 1.7.0 | Rust | MIT OR Apache-2.0 | 1 |
| yoke | 0.8.4 | Rust | Unicode-3.0 | 1 |
| yoke-derive | 0.8.4 | Rust | Unicode-3.0 | 1 |
| zbus | 5.19.0 | Rust | MIT | 1 |
| zbus_macros | 5.19.0 | Rust | MIT | 1 |
| zbus_names | 4.3.5 | Rust | MIT | 1 |
| zcheapstr | 1.2.0 | Rust | MIT | 1 |
| zerocopy | 0.8.63 | Rust | BSD-2-Clause OR Apache-2.0 OR MIT | 1 |
| zerocopy-derive | 0.8.63 | Rust | BSD-2-Clause OR Apache-2.0 OR MIT | 1 |
| zerofrom | 0.1.8 | Rust | Unicode-3.0 | 1 |
| zerofrom-derive | 0.1.8 | Rust | Unicode-3.0 | 1 |
| zeroize | 1.9.1 | Rust | Apache-2.0 OR MIT | 1 |
| zerotrie | 0.2.5 | Rust | Unicode-3.0 | 1 |
| zerovec | 0.11.9 | Rust | Unicode-3.0 | 1 |
| zerovec-derive | 0.11.7 | Rust | Unicode-3.0 | 1 |
| zlib-rs | 0.6.8 | Rust | Zlib | 1 |
| zmij | 1.0.23 | Rust | MIT | 1 |
| zune-core | 0.5.3 | Rust | MIT OR Apache-2.0 OR Zlib | 1 |
| zune-jpeg | 0.5.15 | Rust | MIT OR Apache-2.0 OR Zlib | 1 |
| zvariant | 5.15.1 | Rust | MIT | 1 |
| zvariant_derive | 5.15.1 | Rust | MIT | 1 |
| zvariant_utils | 4.2.0 | Rust | MIT | 1 |
| @babel/runtime | 7.29.10 | npm | MIT | 1 |
| @fontsource/ibm-plex-sans | 5.3.0 | npm | OFL-1.1 | 2 (OFL-1.1) |
| @fontsource/jetbrains-mono | 5.3.0 | npm | OFL-1.1 | 2 (OFL-1.1) |
| @tanstack/query-core | 5.105.3 | npm | MIT | 1 |
| @tanstack/react-query | 5.105.3 | npm | MIT | 1 |
| @tauri-apps/api | 2.12.4 | npm | Apache-2.0 OR MIT | 1 |
| @tauri-apps/plugin-clipboard-manager | 2.5.0 | npm | MIT OR Apache-2.0 | 1 · 3 |
| @tauri-apps/plugin-dialog | 2.8.4 | npm | MIT OR Apache-2.0 | 1 · 3 |
| @tauri-apps/plugin-opener | 2.7.2 | npm | MIT OR Apache-2.0 | 1 · 3 |
| @xterm/addon-fit | 0.10.0 | npm | MIT | 1 |
| @xterm/xterm | 5.5.0 | npm | MIT | 1 |
| html-parse-stringify | 3.1.0 | npm | MIT | 1 |
| i18next | 25.10.10 | npm | MIT | 1 |
| lucide-react | 0.540.0 | npm | ISC | 1 |
| react | 19.3.1 | npm | MIT | 1 |
| react-dom | 19.3.1 | npm | MIT | 1 |
| react-i18next | 15.7.4 | npm | MIT | 1 |
| scheduler | 0.28.0 | npm | MIT | 1 |
| void-elements | 3.1.0 | npm | MIT | 1 |
| zustand | 5.0.16 | npm | MIT | 1 |

## 6. Inventario: no se distribuyen (compilación, pruebas y desarrollo)

| Componente | Versión | Uso | Licencia |
|---|---|---|---|
| autocfg | 1.5.1 | Rust · compilación | Apache-2.0 OR MIT |
| cargo_toml | 1.0.1 | Rust · compilación | Apache-2.0 OR MIT |
| cc | 1.11.0 | Rust · compilación | MIT OR Apache-2.0 |
| cfg-expr | 0.15.8 | Rust · compilación | MIT OR Apache-2.0 |
| cfg_aliases | 0.2.2 | Rust · compilación | MIT |
| embed-resource | 3.0.12 | Rust · compilación | MIT |
| find-msvc-tools | 0.1.15 | Rust · compilación | MIT OR Apache-2.0 |
| phf_codegen | 0.13.1 | Rust · compilación | MIT |
| pkg-config | 0.3.34 | Rust · compilación | MIT OR Apache-2.0 |
| rustc_version | 0.4.1 | Rust · compilación | MIT OR Apache-2.0 |
| shlex | 2.0.1 | Rust · compilación | MIT OR Apache-2.0 |
| static_vcruntime | 3.0.0 | Rust · compilación | MIT OR Apache-2.0 OR Zlib |
| string_cache_codegen | 0.6.1 | Rust · compilación | MIT OR Apache-2.0 |
| system-deps | 6.2.2 | Rust · compilación | MIT OR Apache-2.0 |
| target-lexicon | 0.12.16 | Rust · compilación | Apache-2.0 WITH LLVM-exception |
| tauri-build | 2.7.4 | Rust · compilación | Apache-2.0 OR MIT |
| tauri-plugin | 2.7.2 | Rust · compilación | Apache-2.0 OR MIT |
| tauri-winres | 0.3.7 | Rust · compilación | MIT |
| version-compare | 0.2.1 | Rust · compilación | MIT |
| version_check | 0.9.5 | Rust · compilación | MIT/Apache-2.0 |
| tempfile | 3.27.0 | Rust · pruebas | MIT OR Apache-2.0 |
| @adobe/css-tools | 4.5.0 | npm · desarrollo | MIT |
| @asamuzakjp/css-color | 3.2.0 | npm · desarrollo | MIT |
| @babel/code-frame | 7.29.7 | npm · desarrollo | MIT |
| @babel/compat-data | 7.29.7 | npm · desarrollo | MIT |
| @babel/core | 7.29.7 | npm · desarrollo | MIT |
| @babel/generator | 7.29.12 | npm · desarrollo | MIT |
| @babel/helper-compilation-targets | 7.29.7 | npm · desarrollo | MIT |
| @babel/helper-globals | 7.29.7 | npm · desarrollo | MIT |
| @babel/helper-module-imports | 7.29.7 | npm · desarrollo | MIT |
| @babel/helper-module-transforms | 7.29.7 | npm · desarrollo | MIT |
| @babel/helper-plugin-utils | 7.29.7 | npm · desarrollo | MIT |
| @babel/helper-string-parser | 7.29.11 | npm · desarrollo | MIT |
| @babel/helper-validator-identifier | 7.29.7 | npm · desarrollo | MIT |
| @babel/helper-validator-option | 7.29.7 | npm · desarrollo | MIT |
| @babel/helpers | 7.29.10 | npm · desarrollo | MIT |
| @babel/parser | 7.29.12 | npm · desarrollo | MIT |
| @babel/plugin-transform-react-jsx-self | 7.29.7 | npm · desarrollo | MIT |
| @babel/plugin-transform-react-jsx-source | 7.29.7 | npm · desarrollo | MIT |
| @babel/template | 7.29.7 | npm · desarrollo | MIT |
| @babel/traverse | 7.29.12 | npm · desarrollo | MIT |
| @babel/types | 7.29.11 | npm · desarrollo | MIT |
| @cacheable/memory | 2.2.0 | npm · desarrollo | MIT |
| @cacheable/utils | 2.5.0 | npm · desarrollo | MIT |
| @csstools/color-helpers | 5.1.0 | npm · desarrollo | MIT-0 |
| @csstools/css-calc | 2.1.4 | npm · desarrollo | MIT |
| @csstools/css-color-parser | 3.1.0 | npm · desarrollo | MIT |
| @csstools/css-parser-algorithms | 3.0.5 | npm · desarrollo | MIT |
| @csstools/css-tokenizer | 3.0.4 | npm · desarrollo | MIT |
| @emnapi/core | 1.11.3 | npm · desarrollo | MIT |
| @emnapi/runtime | 1.11.3 | npm · desarrollo | MIT |
| @emnapi/wasi-threads | 1.2.3 | npm · desarrollo | MIT |
| @esbuild/aix-ppc64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/android-arm | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/android-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/android-x64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/darwin-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/darwin-x64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/freebsd-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/freebsd-x64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-arm | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-ia32 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-loong64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-mips64el | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-ppc64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-riscv64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-s390x | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/linux-x64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/netbsd-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/netbsd-x64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/openbsd-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/openbsd-x64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/openharmony-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/sunos-x64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/win32-arm64 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/win32-ia32 | 0.28.2 | npm · desarrollo | MIT |
| @esbuild/win32-x64 | 0.28.2 | npm · desarrollo | MIT |
| @eslint-community/eslint-utils | 4.10.1 | npm · desarrollo | MIT |
| @eslint-community/regexpp | 4.12.3 | npm · desarrollo | MIT |
| @eslint/config-array | 0.23.5 | npm · desarrollo | Apache-2.0 |
| @eslint/config-helpers | 0.7.0 | npm · desarrollo | Apache-2.0 |
| @eslint/core | 1.2.1 | npm · desarrollo | Apache-2.0 |
| @eslint/js | 10.0.1 | npm · desarrollo | MIT |
| @eslint/object-schema | 3.0.5 | npm · desarrollo | Apache-2.0 |
| @eslint/plugin-kit | 0.7.3 | npm · desarrollo | Apache-2.0 |
| @humanfs/core | 0.19.2 | npm · desarrollo | Apache-2.0 |
| @humanfs/node | 0.16.8 | npm · desarrollo | Apache-2.0 |
| @humanfs/types | 0.15.0 | npm · desarrollo | Apache-2.0 |
| @humanwhocodes/module-importer | 1.0.1 | npm · desarrollo | Apache-2.0 |
| @humanwhocodes/retry | 0.4.3 | npm · desarrollo | Apache-2.0 |
| @isaacs/nonrecursive-glob-match | 1.1.0 | npm · desarrollo | BlueOak-1.0.0 |
| @jridgewell/gen-mapping | 0.3.13 | npm · desarrollo | MIT |
| @jridgewell/remapping | 2.3.5 | npm · desarrollo | MIT |
| @jridgewell/resolve-uri | 3.1.2 | npm · desarrollo | MIT |
| @jridgewell/sourcemap-codec | 1.6.0 | npm · desarrollo | MIT |
| @jridgewell/trace-mapping | 0.3.31 | npm · desarrollo | MIT |
| @keyv/bigmap | 1.3.1 | npm · desarrollo | MIT |
| @keyv/serialize | 1.1.1 | npm · desarrollo | MIT |
| @napi-rs/wasm-runtime | 1.2.2 | npm · desarrollo | MIT |
| @rolldown/pluginutils | 1.0.0-rc.3 | npm · desarrollo | MIT |
| @rollup/rollup-android-arm-eabi | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-android-arm64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-android-x64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-darwin-arm64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-darwin-x64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-freebsd-arm64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-freebsd-x64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-arm-gnueabihf | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-arm-musleabihf | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-arm64-gnu | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-arm64-musl | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-loong64-gnu | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-loong64-musl | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-ppc64-gnu | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-ppc64-musl | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-riscv64-gnu | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-riscv64-musl | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-s390x-gnu | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-x64-gnu | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-linux-x64-musl | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-openbsd-x64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-openharmony-arm64 | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-win32-arm64-msvc | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-win32-ia32-msvc | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-win32-x64-gnu | 4.67.5 | npm · desarrollo | MIT |
| @rollup/rollup-win32-x64-msvc | 4.67.5 | npm · desarrollo | MIT |
| @tailwindcss/node | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-android-arm64 | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-darwin-arm64 | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-darwin-x64 | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-freebsd-x64 | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-linux-arm-gnueabihf | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-linux-arm64-gnu | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-linux-arm64-musl | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-linux-x64-gnu | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-linux-x64-musl | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-wasm32-wasi | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-win32-arm64-msvc | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/oxide-win32-x64-msvc | 4.3.4 | npm · desarrollo | MIT |
| @tailwindcss/vite | 4.3.4 | npm · desarrollo | MIT |
| @tauri-apps/cli | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-darwin-arm64 | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-darwin-x64 | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-linux-arm-gnueabihf | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-linux-arm64-gnu | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-linux-arm64-musl | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-linux-riscv64-gnu | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-linux-x64-gnu | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-linux-x64-musl | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-win32-arm64-msvc | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-win32-ia32-msvc | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @tauri-apps/cli-win32-x64-msvc | 2.12.4 | npm · desarrollo | Apache-2.0 OR MIT |
| @testing-library/dom | 10.4.2 | npm · desarrollo | MIT |
| @testing-library/jest-dom | 6.9.1 | npm · desarrollo | MIT |
| @testing-library/react | 16.3.3 | npm · desarrollo | MIT |
| @testing-library/user-event | 14.6.8 | npm · desarrollo | MIT |
| @tybys/wasm-util | 0.10.3 | npm · desarrollo | MIT |
| @types/aria-query | 5.0.4 | npm · desarrollo | MIT |
| @types/babel__core | 7.20.5 | npm · desarrollo | MIT |
| @types/babel__generator | 7.27.0 | npm · desarrollo | MIT |
| @types/babel__template | 7.4.4 | npm · desarrollo | MIT |
| @types/babel__traverse | 7.28.0 | npm · desarrollo | MIT |
| @types/chai | 5.2.3 | npm · desarrollo | MIT |
| @types/deep-eql | 4.0.2 | npm · desarrollo | MIT |
| @types/esrecurse | 4.3.1 | npm · desarrollo | MIT |
| @types/estree | 1.0.10 | npm · desarrollo | MIT |
| @types/json-schema | 7.0.15 | npm · desarrollo | MIT |
| @types/node | 22.20.5 | npm · desarrollo | MIT |
| @types/react | 19.3.0 | npm · desarrollo | MIT |
| @types/react-dom | 19.3.0 | npm · desarrollo | MIT |
| @typescript-eslint/eslint-plugin | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/parser | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/project-service | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/scope-manager | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/tsconfig-utils | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/type-utils | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/types | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/typescript-estree | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/utils | 8.72.2 | npm · desarrollo | MIT |
| @typescript-eslint/visitor-keys | 8.72.2 | npm · desarrollo | MIT |
| @vitejs/plugin-react | 5.2.0 | npm · desarrollo | MIT |
| @vitest/mocker | 5.1.0 | npm · desarrollo | MIT |
| @vitest/spy | 5.1.0 | npm · desarrollo | MIT |
| acorn | 8.19.0 | npm · desarrollo | MIT |
| acorn-jsx | 5.3.2 | npm · desarrollo | MIT |
| agent-base | 7.1.4 | npm · desarrollo | MIT |
| ajv | 6.15.0 | npm · desarrollo | MIT |
| ansi-regex | 5.0.1 | npm · desarrollo | MIT |
| ansi-styles | 5.2.0 | npm · desarrollo | MIT |
| aria-query | 5.3.0 | npm · desarrollo | Apache-2.0 |
| assertion-error | 2.0.1 | npm · desarrollo | MIT |
| balanced-match | 4.0.4 | npm · desarrollo | MIT |
| baseline-browser-mapping | 2.11.35 | npm · desarrollo | Apache-2.0 |
| brace-expansion | 5.0.13 | npm · desarrollo | MIT |
| browserslist | 4.29.5 | npm · desarrollo | MIT |
| cacheable | 2.5.0 | npm · desarrollo | MIT |
| caniuse-lite | 1.0.30001820 | npm · desarrollo | CC-BY-4.0 |
| chai | 6.3.0 | npm · desarrollo | MIT |
| convert-source-map | 2.0.0 | npm · desarrollo | MIT |
| cross-spawn | 7.0.6 | npm · desarrollo | MIT |
| css.escape | 1.5.1 | npm · desarrollo | MIT |
| cssstyle | 4.6.0 | npm · desarrollo | MIT |
| csstype | 3.2.3 | npm · desarrollo | MIT |
| data-urls | 5.0.0 | npm · desarrollo | MIT |
| debug | 4.4.3 | npm · desarrollo | MIT |
| decimal.js | 10.6.0 | npm · desarrollo | MIT |
| deep-is | 0.1.4 | npm · desarrollo | MIT |
| dequal | 2.0.3 | npm · desarrollo | MIT |
| detect-libc | 2.1.2 | npm · desarrollo | Apache-2.0 |
| dom-accessibility-api | 0.5.16 | npm · desarrollo | MIT |
| dom-accessibility-api | 0.6.3 | npm · desarrollo | MIT |
| electron-to-chromium | 1.5.476 | npm · desarrollo | ISC |
| enhanced-resolve | 5.26.1 | npm · desarrollo | MIT |
| entities | 6.0.1 | npm · desarrollo | BSD-2-Clause |
| es-module-lexer | 2.3.2 | npm · desarrollo | MIT |
| esbuild | 0.28.2 | npm · desarrollo | MIT |
| escalade | 3.2.0 | npm · desarrollo | MIT |
| escape-string-regexp | 4.0.0 | npm · desarrollo | MIT |
| eslint | 10.14.0 | npm · desarrollo | MIT |
| eslint-plugin-react-hooks | 7.1.1 | npm · desarrollo | MIT |
| eslint-scope | 9.1.2 | npm · desarrollo | BSD-2-Clause |
| eslint-visitor-keys | 3.4.3 | npm · desarrollo | Apache-2.0 |
| eslint-visitor-keys | 5.0.1 | npm · desarrollo | Apache-2.0 |
| espree | 11.2.0 | npm · desarrollo | BSD-2-Clause |
| esquery | 1.7.0 | npm · desarrollo | BSD-3-Clause |
| esrecurse | 4.3.0 | npm · desarrollo | BSD-2-Clause |
| estraverse | 5.3.0 | npm · desarrollo | BSD-2-Clause |
| estree-walker | 3.0.3 | npm · desarrollo | MIT |
| esutils | 2.0.3 | npm · desarrollo | BSD-2-Clause |
| expect-type | 1.4.0 | npm · desarrollo | Apache-2.0 |
| fast-deep-equal | 3.1.3 | npm · desarrollo | MIT |
| fast-json-stable-stringify | 2.1.0 | npm · desarrollo | MIT |
| fast-levenshtein | 2.0.6 | npm · desarrollo | MIT |
| fdir | 6.5.0 | npm · desarrollo | MIT |
| file-entry-cache | 11.1.5 | npm · desarrollo | MIT |
| find-up | 5.0.0 | npm · desarrollo | MIT |
| flat-cache | 6.1.23 | npm · desarrollo | MIT |
| flatted | 3.4.4 | npm · desarrollo | ISC |
| fsevents | 2.3.3 | npm · desarrollo | MIT |
| gensync | 1.0.0-beta.2 | npm · desarrollo | MIT |
| glob-parent | 6.0.2 | npm · desarrollo | ISC |
| graceful-fs | 4.2.11 | npm · desarrollo | ISC |
| hashery | 1.5.1 | npm · desarrollo | MIT |
| hermes-estree | 0.25.1 | npm · desarrollo | MIT |
| hermes-parser | 0.25.1 | npm · desarrollo | MIT |
| hookified | 1.15.1 | npm · desarrollo | MIT |
| hookified | 2.2.0 | npm · desarrollo | MIT |
| html-encoding-sniffer | 4.0.0 | npm · desarrollo | MIT |
| http-proxy-agent | 7.0.2 | npm · desarrollo | MIT |
| https-proxy-agent | 7.0.6 | npm · desarrollo | MIT |
| iconv-lite | 0.6.3 | npm · desarrollo | MIT |
| ignore | 5.3.2 | npm · desarrollo | MIT |
| ignore | 7.0.14 | npm · desarrollo | MIT |
| imurmurhash | 0.1.4 | npm · desarrollo | MIT |
| indent-string | 4.0.0 | npm · desarrollo | MIT |
| is-extglob | 2.1.1 | npm · desarrollo | MIT |
| is-glob | 4.0.3 | npm · desarrollo | MIT |
| is-potential-custom-element-name | 1.0.1 | npm · desarrollo | MIT |
| isexe | 2.0.0 | npm · desarrollo | ISC |
| jiti | 2.7.0 | npm · desarrollo | MIT |
| js-tokens | 4.0.0 | npm · desarrollo | MIT |
| jsdom | 26.1.0 | npm · desarrollo | MIT |
| jsesc | 3.1.0 | npm · desarrollo | MIT |
| json-schema-traverse | 0.4.1 | npm · desarrollo | MIT |
| json-stable-stringify-without-jsonify | 1.0.1 | npm · desarrollo | MIT |
| json5 | 2.2.3 | npm · desarrollo | MIT |
| keyv | 5.6.0 | npm · desarrollo | MIT |
| levn | 0.4.1 | npm · desarrollo | MIT |
| lightningcss | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-android-arm64 | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-darwin-arm64 | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-darwin-x64 | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-freebsd-x64 | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-linux-arm-gnueabihf | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-linux-arm64-gnu | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-linux-arm64-musl | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-linux-x64-gnu | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-linux-x64-musl | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-win32-arm64-msvc | 1.33.0 | npm · desarrollo | MPL-2.0 |
| lightningcss-win32-x64-msvc | 1.33.0 | npm · desarrollo | MPL-2.0 |
| locate-path | 6.0.0 | npm · desarrollo | MIT |
| lru-cache | 10.4.3 | npm · desarrollo | ISC |
| lru-cache | 5.1.1 | npm · desarrollo | ISC |
| lz-string | 1.5.0 | npm · desarrollo | MIT |
| magic-string | 1.4.5 | npm · desarrollo | MIT |
| min-indent | 1.0.1 | npm · desarrollo | MIT |
| minimatch | 11.0.8 | npm · desarrollo | BlueOak-1.0.0 |
| ms | 2.1.3 | npm · desarrollo | MIT |
| nanoid | 3.3.20 | npm · desarrollo | MIT |
| natural-compare | 1.4.0 | npm · desarrollo | MIT |
| node-releases | 2.0.63 | npm · desarrollo | MIT |
| nwsapi | 2.2.28 | npm · desarrollo | MIT |
| obug | 3.0.0 | npm · desarrollo | MIT |
| optionator | 0.9.4 | npm · desarrollo | MIT |
| p-limit | 3.1.0 | npm · desarrollo | MIT |
| p-locate | 5.0.0 | npm · desarrollo | MIT |
| parse5 | 7.3.0 | npm · desarrollo | MIT |
| path-exists | 4.0.0 | npm · desarrollo | MIT |
| path-key | 3.1.1 | npm · desarrollo | MIT |
| picocolors | 1.1.1 | npm · desarrollo | ISC |
| picomatch | 4.0.7 | npm · desarrollo | MIT |
| postcss | 8.5.32 | npm · desarrollo | MIT |
| prelude-ls | 1.2.1 | npm · desarrollo | MIT |
| pretty-format | 27.5.1 | npm · desarrollo | MIT |
| punycode | 2.3.1 | npm · desarrollo | MIT |
| qified | 0.10.1 | npm · desarrollo | MIT |
| react-is | 17.0.2 | npm · desarrollo | MIT |
| react-refresh | 0.18.0 | npm · desarrollo | MIT |
| redent | 3.0.0 | npm · desarrollo | MIT |
| rollup | 4.67.5 | npm · desarrollo | MIT |
| rrweb-cssom | 0.8.0 | npm · desarrollo | MIT |
| safer-buffer | 2.1.2 | npm · desarrollo | MIT |
| saxes | 6.0.0 | npm · desarrollo | ISC |
| semver | 6.3.2 | npm · desarrollo | ISC |
| semver | 7.8.6 | npm · desarrollo | ISC |
| shebang-command | 2.0.0 | npm · desarrollo | MIT |
| shebang-regex | 3.0.0 | npm · desarrollo | MIT |
| source-map-js | 1.2.2 | npm · desarrollo | BSD-3-Clause |
| std-env | 4.3.0 | npm · desarrollo | MIT |
| strip-indent | 3.0.0 | npm · desarrollo | MIT |
| symbol-tree | 3.2.4 | npm · desarrollo | MIT |
| tailwindcss | 4.3.4 | npm · desarrollo | MIT |
| tapable | 2.3.3 | npm · desarrollo | MIT |
| tinybench | 6.2.3 | npm · desarrollo | MIT |
| tinyexec | 1.3.1 | npm · desarrollo | MIT |
| tinyglobby | 0.2.17 | npm · desarrollo | MIT |
| tldts | 6.1.86 | npm · desarrollo | MIT |
| tldts-core | 6.1.86 | npm · desarrollo | MIT |
| tough-cookie | 5.1.2 | npm · desarrollo | BSD-3-Clause |
| tr46 | 5.1.1 | npm · desarrollo | MIT |
| ts-api-utils | 2.5.0 | npm · desarrollo | MIT |
| tslib | 2.8.1 | npm · desarrollo | 0BSD |
| type-check | 0.4.0 | npm · desarrollo | MIT |
| typescript | 5.8.3 | npm · desarrollo | Apache-2.0 |
| typescript-eslint | 8.72.2 | npm · desarrollo | MIT |
| undici-types | 6.21.0 | npm · desarrollo | MIT |
| update-browserslist-db | 1.3.4 | npm · desarrollo | MIT |
| uri-js | 4.4.1 | npm · desarrollo | BSD-2-Clause |
| vite | 7.3.7 | npm · desarrollo | MIT |
| vitest | 5.1.0 | npm · desarrollo | MIT |
| w3c-xmlserializer | 5.0.0 | npm · desarrollo | MIT |
| webidl-conversions | 7.0.0 | npm · desarrollo | BSD-2-Clause |
| whatwg-encoding | 3.1.1 | npm · desarrollo | MIT |
| whatwg-mimetype | 4.0.0 | npm · desarrollo | MIT |
| whatwg-url | 14.2.0 | npm · desarrollo | MIT |
| which | 2.0.2 | npm · desarrollo | ISC |
| why-is-node-running | 3.2.1 | npm · desarrollo | MIT |
| word-wrap | 1.2.5 | npm · desarrollo | MIT |
| ws | 8.22.1 | npm · desarrollo | MIT |
| xml-name-validator | 5.0.0 | npm · desarrollo | Apache-2.0 |
| xmlchars | 2.2.0 | npm · desarrollo | MIT |
| yallist | 3.1.1 | npm · desarrollo | ISC |
| yocto-queue | 0.1.0 | npm · desarrollo | MIT |
| zod | 4.6.7 | npm · desarrollo | MIT |
| zod-validation-error | 4.0.2 | npm · desarrollo | MIT |
