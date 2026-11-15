// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Identificadores del nombre anterior del producto (hasta la 1.7.0, D-113).
//!
//! Solo se usan para **leer** lo que ya existe en Docker o en disco y pasarlo
//! al nombre nuevo: nada se crea con ellos.
//! - Etiqueta de la descripción en contenedores ya creados: se lee como la
//!   nueva y, al editar la descripción, se sustituye por la nueva.
//! - Nombres de los contenedores apartados al editar (`<nombre>-dcc-backup-…`):
//!   se siguen reconociendo como copias de la edición.
//! - Carpeta de copias dentro del destino elegido: se renombra a la nueva la
//!   primera vez que se usa ese destino ([`crate::backup::store::LocalTarget`]).

/// Etiqueta de la descripción de un contenedor.
pub const DESCRIPTION_LABEL: &str = "dcc.description";
/// Partes del nombre de los contenedores apartados al editar.
pub const BACKUP_INFIX: &str = "-dcc-backup-";
pub const REPLACED_INFIX: &str = "-dcc-replaced-";
/// Carpeta de copias dentro del destino elegido.
pub const BACKUP_ROOT_DIR: &str = "docker-control-center-copias";
/// Carpeta de los sockets de control de SSH en `$XDG_RUNTIME_DIR`: solo hay
/// archivos temporales; no se migra, se borra si queda vacía.
pub const RUNTIME_DIR: &str = "docker-control-center";
