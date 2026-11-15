// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comprobación de bajo nivel de un socket, antes de hablar HTTP con él.
//! Permite distinguir "no existe", "sin permiso" y "daemon detenido", que la
//! librería HTTP mezclaría en un único error de E/S.

use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::time::Duration;

use crate::model::{ProbeResult, SocketPermissions};
use crate::sys;

pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

pub async fn probe(path: &str) -> ProbeResult {
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ProbeResult::Missing,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return ProbeResult::PermissionDenied
        }
        Err(e) => {
            return ProbeResult::Error {
                message: e.to_string(),
            }
        }
    };
    if !meta.file_type().is_socket() {
        return ProbeResult::NotASocket;
    }
    match tokio::time::timeout(PROBE_TIMEOUT, tokio::net::UnixStream::connect(path)).await {
        Ok(Ok(_)) => ProbeResult::Reachable,
        Ok(Err(e)) => match e.raw_os_error() {
            Some(libc::EACCES) | Some(libc::EPERM) => ProbeResult::PermissionDenied,
            Some(libc::ECONNREFUSED) | Some(libc::ENOENT) => ProbeResult::Refused,
            _ => ProbeResult::Error {
                message: e.to_string(),
            },
        },
        Err(_) => ProbeResult::Error {
            message: "tiempo de espera agotado al conectar con el socket".into(),
        },
    }
}

/// Propietario, grupo y permisos del socket, y si el usuario puede usar ese grupo.
pub fn permissions(path: &str) -> SocketPermissions {
    let meta = std::fs::metadata(path).ok();
    let (uid, gid, mode) = meta
        .map(|m| (m.uid(), m.gid(), m.permissions().mode() & 0o777))
        .unwrap_or((0, 0, 0));
    let groups = sys::groups();
    let group = groups.iter().find(|g| g.gid == gid);
    let user = sys::user_context();
    let primary_gid = sys::parse_passwd_for(
        &std::fs::read_to_string("/etc/passwd").unwrap_or_default(),
        user.uid,
    )
    .map(|(_, g)| g);
    let user_in_group = primary_gid == Some(gid)
        || group.is_some_and(|g| g.members.iter().any(|m| m == &user.name));
    SocketPermissions {
        socket_path: path.to_owned(),
        owner: sys::user_name_for(uid),
        group: group
            .map(|g| g.name.clone())
            .unwrap_or_else(|| gid.to_string()),
        mode: format!("{mode:o}"),
        user_in_group,
        group_active_in_session: sys::process_groups().contains(&gid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_and_not_a_socket() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("nada.sock");
        assert_eq!(probe(p.to_str().unwrap()).await, ProbeResult::Missing);
        std::fs::write(&p, "x").unwrap();
        assert_eq!(probe(p.to_str().unwrap()).await, ProbeResult::NotASocket);
    }

    #[tokio::test]
    async fn listening_socket_is_reachable_and_stale_socket_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.sock");
        let path = p.to_str().unwrap().to_owned();
        let listener = std::os::unix::net::UnixListener::bind(&p).unwrap();
        assert_eq!(probe(&path).await, ProbeResult::Reachable);
        drop(listener); // el archivo queda, pero nadie escucha
        assert_eq!(probe(&path).await, ProbeResult::Refused);
    }

    #[test]
    fn permissions_reports_mode() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.sock");
        let _l = std::os::unix::net::UnixListener::bind(&p).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o660)).unwrap();
        let perms = permissions(p.to_str().unwrap());
        assert_eq!(perms.mode, "660");
        assert!(perms.group_active_in_session);
    }
}
