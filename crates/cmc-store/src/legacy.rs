// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Identificadores del nombre anterior del producto, *Docker Control Center*
//! (hasta la 1.7.0, D-113).
//!
//! Solo existen para encontrar lo que dejó una versión anterior y pasarlo al
//! nombre nuevo ([`crate::migrate`], y en la aplicación las contraseñas y las
//! tareas programadas). Nada nuevo se crea con ellos.

/// Carpeta de configuración (`$XDG_CONFIG_HOME/…`) y de estado
/// (`$XDG_STATE_HOME/…`).
pub const APP_DIR: &str = "docker-control-center";
/// Identificador de Tauri: carpetas de datos y caché de WebKit
/// (`$XDG_DATA_HOME/…`, `$XDG_CACHE_HOME/…`).
pub const TAURI_IDENTIFIER: &str = "org.dockercontrolcenter.app";
/// Paquete Debian. El nuevo lo sustituye (`Conflicts`/`Replaces`) y el
/// historial de APT lo nombra en las instalaciones anteriores.
pub const PACKAGE: &str = "docker-control-center";
/// Atributos de las contraseñas guardadas en el almacén del sistema.
pub const KEYRING_APP: &str = "docker-control-center";
pub const KEYRING_ATTR_SERVER: &str = "dcc-server";
pub const KEYRING_ATTR_KIND: &str = "dcc-kind";
/// Comienzo de la etiqueta visible de esas entradas.
pub const KEYRING_LABEL_PREFIX: &str = "Docker Control Center";
/// Unidades de systemd de usuario de las tareas de copia programadas.
pub const UNIT_PREFIX: &str = "dcc-backup-";
pub const UNIT_MARKER: &str = "# dcc-managed-backup-job:";
