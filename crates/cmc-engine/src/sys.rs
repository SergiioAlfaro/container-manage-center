// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Información del sistema necesaria para diagnosticar el acceso a Docker.
//! Se leen /etc/passwd y /etc/group directamente: no se ejecuta ningún programa.

#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

use crate::model::{Installation, UserContext};

#[cfg(unix)]
pub fn uid() -> u32 {
    // SAFETY: getuid no falla ni tiene precondiciones.
    unsafe { libc::getuid() }
}

/// En Windows no hay uid: se usa el de un usuario normal, para que nada se
/// trate como «root» (1.9.0, D-114).
#[cfg(windows)]
pub fn uid() -> u32 {
    1000
}

/// Grupos activos del proceso (en Windows, ninguno: el acceso a Docker no
/// depende de grupos de Unix).
#[cfg(windows)]
pub fn process_groups() -> Vec<u32> {
    Vec::new()
}

/// Grupos activos del proceso (primario y suplementarios).
#[cfg(unix)]
pub fn process_groups() -> Vec<u32> {
    // SAFETY: primero se pide el tamaño (0, null) y después se pasa un búfer de ese tamaño.
    unsafe {
        let n = libc::getgroups(0, std::ptr::null_mut());
        let mut v = vec![0 as libc::gid_t; n.max(0) as usize];
        let got = if n > 0 {
            libc::getgroups(n, v.as_mut_ptr())
        } else {
            0
        };
        v.truncate(got.max(0) as usize);
        let egid = libc::getegid();
        if !v.contains(&egid) {
            v.push(egid);
        }
        v
    }
}

/// Entrada de /etc/group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupEntry {
    pub name: String,
    pub gid: u32,
    pub members: Vec<String>,
}

pub fn parse_group_file(content: &str) -> Vec<GroupEntry> {
    content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut f = l.split(':');
            let name = f.next()?.to_owned();
            let _pw = f.next()?;
            let gid = f.next()?.parse().ok()?;
            let members = f
                .next()
                .unwrap_or("")
                .split(',')
                .filter(|m| !m.is_empty())
                .map(str::to_owned)
                .collect();
            Some(GroupEntry { name, gid, members })
        })
        .collect()
}

/// (nombre, gid primario) del usuario `uid` según /etc/passwd.
pub fn parse_passwd_for(content: &str, uid: u32) -> Option<(String, u32)> {
    content.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        if f.len() >= 4 && f[2].parse::<u32>().ok()? == uid {
            Some((f[0].to_owned(), f[3].parse().ok()?))
        } else {
            None
        }
    })
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn user_context() -> UserContext {
    let uid = uid();
    #[cfg(windows)]
    let name = std::env::var("USERNAME").unwrap_or_else(|_| uid.to_string());
    #[cfg(unix)]
    let name = parse_passwd_for(&read("/etc/passwd"), uid)
        .map(|(n, _)| n)
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| uid.to_string());
    UserContext { name, uid }
}

pub fn user_name_for(uid: u32) -> String {
    parse_passwd_for(&read("/etc/passwd"), uid)
        .map(|(n, _)| n)
        .unwrap_or_else(|| uid.to_string())
}

pub fn groups() -> Vec<GroupEntry> {
    parse_group_file(&read("/etc/group"))
}

/// Busca un ejecutable en PATH y en las rutas habituales (una app lanzada desde el
/// menú puede tener un PATH reducido).
pub fn find_executable(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    #[cfg(windows)]
    {
        // Docker Desktop instala sus programas aquí aunque no estén en el PATH.
        for v in ["ProgramFiles", "ProgramW6432"] {
            if let Some(pf) = std::env::var_os(v) {
                dirs.push(PathBuf::from(pf).join(r"Docker\Docker\resources\bin"));
            }
        }
        let exts: Vec<String> = std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_ascii_lowercase())
            .collect();
        #[allow(clippy::needless_return)]
        return dirs.into_iter().find_map(|d| {
            exts.iter()
                .map(|e| d.join(format!("{name}{e}")))
                .chain(std::iter::once(d.join(name)))
                .find(|p| p.is_file())
        });
    }
    #[cfg(unix)]
    for d in [
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/local/sbin",
        "/usr/sbin",
        "/sbin",
    ] {
        dirs.push(PathBuf::from(d));
    }
    #[cfg(unix)]
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("bin"));
        dirs.push(home.join(".local/bin"));
    }
    #[cfg(unix)]
    dirs.into_iter()
        .map(|d| d.join(name))
        .find(|p| is_executable(p))
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// En Windows, un programa de consola lanzado desde la aplicación abriría
/// una ventana negra: se pide que no la tenga (1.9.0, D-114).
pub fn no_console(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    let _ = cmd;
}

pub fn installation() -> Installation {
    Installation {
        cli_path: find_executable("docker").map(|p| p.display().to_string()),
        daemon_path: find_executable("dockerd").map(|p| p.display().to_string()),
        rootless_tools: find_executable("dockerd-rootless.sh").is_some()
            || find_executable("dockerd-rootless-setuptool.sh").is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_group_file() {
        let g = parse_group_file("root:x:0:\n# c\ndocker:x:998:ana,luis\nsudo:x:27:ana\n");
        assert_eq!(g.len(), 3);
        assert_eq!(
            g[1],
            GroupEntry {
                name: "docker".into(),
                gid: 998,
                members: vec!["ana".into(), "luis".into()]
            }
        );
        assert!(g[0].members.is_empty());
    }

    #[test]
    fn parses_passwd() {
        let p = "root:x:0:0:root:/root:/bin/bash\nana:x:1000:1000:Ana,,,:/home/ana:/bin/bash\n";
        assert_eq!(parse_passwd_for(p, 1000), Some(("ana".into(), 1000)));
        assert_eq!(parse_passwd_for(p, 5), None);
    }

    #[cfg(unix)]
    #[test]
    fn process_groups_includes_effective_gid() {
        // SAFETY: getegid no tiene precondiciones.
        let egid = unsafe { libc::getegid() };
        assert!(process_groups().contains(&egid));
    }

    #[cfg(unix)]
    #[test]
    fn finds_sh() {
        assert!(find_executable("sh").is_some());
        assert!(find_executable("definitely-not-a-binary-cmc").is_none());
    }
}
