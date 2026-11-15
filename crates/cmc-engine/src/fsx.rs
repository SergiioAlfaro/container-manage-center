// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Permisos de archivos en todas las plataformas (1.9.0, D-114).
//!
//! En Linux, los mismos rasgos de la biblioteca estándar: carpetas 0700 y
//! archivos 0600 para todo lo privado. En Windows no hay modos de Unix: las
//! carpetas de la aplicación están en el perfil del usuario, que Windows ya
//! protege con su lista de control de acceso; los mismos métodos no hacen nada.

#[cfg(unix)]
pub use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

#[cfg(windows)]
pub trait DirBuilderExt {
    fn mode(&mut self, mode: u32) -> &mut Self;
}

#[cfg(windows)]
impl DirBuilderExt for std::fs::DirBuilder {
    fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }
}

#[cfg(windows)]
pub trait OpenOptionsExt {
    fn mode(&mut self, mode: u32) -> &mut Self;
}

#[cfg(windows)]
impl OpenOptionsExt for std::fs::OpenOptions {
    fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }
}

/// `tokio::fs::OpenOptions` tiene `mode` propio en Unix.
#[cfg(windows)]
impl OpenOptionsExt for tokio::fs::OpenOptions {
    fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }
}

/// Modo de Unix de un archivo (en Windows, `default`).
pub fn mode_of(meta: &std::fs::Metadata, default: u32) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = default;
        meta.permissions().mode() & 0o7777
    }
    #[cfg(windows)]
    {
        let _ = meta;
        default
    }
}

/// Cambia el modo de Unix de un archivo (en Windows no hace nada).
pub fn set_mode(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(windows)]
    {
        let _ = (path, mode);
        Ok(())
    }
}
