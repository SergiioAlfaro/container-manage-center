// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Utilidades comunes a los módulos de recursos: validación de identificadores,
//! construcción de rutas idénticas a las de bollard y bloqueo por recurso.

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;

use crate::error::EngineError;

/// Valida un ID o nombre de contenedor/imagen antes de usarlo en una ruta HTTP.
/// Docker solo admite `[a-zA-Z0-9][a-zA-Z0-9_.-]*` en nombres; los IDs son hexadecimales.
pub fn validate_ref(kind: &str, r: &str) -> Result<(), EngineError> {
    let ok = !r.is_empty()
        && r.len() <= 255
        && r.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && r.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if ok {
        Ok(())
    } else {
        Err(EngineError::InvalidInput {
            message: format!("identificador de {kind} no válido: {r:?}"),
        })
    }
}

/// Ruta con query string serializada exactamente como lo hace bollard cuando
/// recibe opciones: `serde_urlencoded` sobre el mismo struct y `?` siempre,
/// aunque la query quede vacía (comprobado con el tráfico real del socket).
/// Para no enviar un `?` vacío, el llamador pasa `None` a bollard y no usa esta función.
pub fn path_with_query<O: Serialize>(path: &str, opts: &O) -> String {
    format!(
        "{path}?{}",
        serde_urlencoded::to_string(opts).unwrap_or_default()
    )
}

/// Impide dos operaciones simultáneas sobre el mismo recurso.
#[derive(Clone, Default)]
pub struct ResourceLocks {
    busy: Arc<Mutex<HashSet<String>>>,
}

pub struct LockGuard {
    locks: ResourceLocks,
    key: String,
}

impl ResourceLocks {
    pub fn acquire(&self, key: &str) -> Result<LockGuard, EngineError> {
        let mut busy = self.busy.lock();
        if !busy.insert(key.to_owned()) {
            return Err(EngineError::Busy {
                resource: key.to_owned(),
            });
        }
        Ok(LockGuard {
            locks: self.clone(),
            key: key.to_owned(),
        })
    }

    pub fn is_busy(&self, key: &str) -> bool {
        self.busy.lock().contains(key)
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        self.locks.busy.lock().remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_are_validated() {
        for ok in ["web", "a1b2c3d4e5f6", "my_app.v2-1", "0abc"] {
            assert!(validate_ref("contenedor", ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "../etc",
            "a/b",
            "-x",
            ".x",
            "a b",
            "a?x=1",
            "a#b",
            "ñ",
            &"a".repeat(256),
        ] {
            assert!(validate_ref("contenedor", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn path_query_matches_bollard_serialization() {
        let o = bollard::query_parameters::StopContainerOptions {
            signal: None,
            t: Some(10),
        };
        assert_eq!(
            path_with_query("/containers/web/stop", &o),
            "/containers/web/stop?t=10"
        );
        let o = bollard::query_parameters::StopContainerOptions::default();
        assert_eq!(
            path_with_query("/containers/web/stop", &o),
            "/containers/web/stop?"
        );
    }

    #[test]
    fn documented_query_strings() {
        use bollard::query_parameters::*;
        let lc = ListContainersOptions {
            all: true,
            ..Default::default()
        };
        assert_eq!(
            path_with_query("/containers/json", &lc),
            "/containers/json?all=true&size=false"
        );
        let rm = RemoveContainerOptions {
            v: true,
            force: false,
            link: false,
        };
        assert_eq!(
            path_with_query("/containers/x", &rm),
            "/containers/x?v=true&force=false&link=false"
        );
        let li = ListImagesOptions::default();
        assert_eq!(
            path_with_query("/images/json", &li),
            "/images/json?all=false&shared-size=false&digests=false&manifests=false"
        );
    }

    #[test]
    fn locks_are_exclusive_and_released_on_drop() {
        let l = ResourceLocks::default();
        let g = l.acquire("c1").unwrap();
        assert!(matches!(l.acquire("c1"), Err(EngineError::Busy { .. })));
        assert!(l.acquire("c2").is_ok());
        drop(g);
        assert!(!l.is_busy("c1"));
        assert!(l.acquire("c1").is_ok());
    }
}
