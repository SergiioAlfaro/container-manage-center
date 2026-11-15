// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Notificaciones de escritorio (E5, D-105) por D-Bus
//! (`org.freedesktop.Notifications`), con `zbus`, que ya estaba en el árbol
//! de dependencias (lo usa el llavero). No hace falta el complemento de
//! notificaciones ni ningún permiso nuevo en la interfaz.
//!
//! El texto lo pone la interfaz (traducido); aquí solo se limita y se envía.

#[cfg(unix)]
use std::collections::HashMap;

#[cfg(unix)]
use zbus::zvariant::Value;

#[cfg_attr(windows, allow(dead_code))]
pub const APP_NAME: &str = "Container Manage Center";
#[cfg(unix)]
const ICON: &str = "container-manage-center";
const MAX_SUMMARY: usize = 120;
const MAX_BODY: usize = 500;

fn clip(s: &str, n: usize) -> String {
    let t: String = s
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(n)
        .collect();
    t.trim().to_owned()
}

/// Windows (1.9.0, D-114): notificación del sistema (WinRT). El instalador
/// registra el acceso directo con el identificador de la aplicación, que es
/// con el que Windows la muestra.
#[cfg(windows)]
pub async fn send(summary: &str, body: &str) -> Result<u32, String> {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    let summary = clip(summary, MAX_SUMMARY);
    if summary.is_empty() {
        return Err("notificación vacía".into());
    }
    let body = clip(body, MAX_BODY);
    let esc = |t: &str| {
        t.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let xml = format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        esc(&summary),
        esc(&body)
    );
    let r = (|| -> windows::core::Result<()> {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
            crate::migration::IDENTIFIER,
        ))?;
        notifier.Show(&toast)
    })();
    r.map(|()| 0).map_err(|e| e.to_string())
}

/// Envía la notificación por la conexión dada. Devuelve el id que asigna el
/// servidor de notificaciones.
#[cfg(unix)]
pub async fn send_on(conn: &zbus::Connection, summary: &str, body: &str) -> Result<u32, String> {
    let summary = clip(summary, MAX_SUMMARY);
    if summary.is_empty() {
        return Err("notificación vacía".into());
    }
    let body = clip(body, MAX_BODY);
    let hints: HashMap<&str, Value<'_>> = HashMap::from([("urgency", Value::U8(1))]);
    let reply = conn
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                APP_NAME,
                0u32,
                ICON,
                summary.as_str(),
                body.as_str(),
                Vec::<&str>::new(),
                hints,
                -1i32,
            ),
        )
        .await
        .map_err(|e| e.to_string())?;
    reply.body().deserialize::<u32>().map_err(|e| e.to_string())
}

/// Envía por el bus de sesión del usuario.
#[cfg(unix)]
pub async fn send(summary: &str, body: &str) -> Result<u32, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| format!("no hay bus de sesión: {e}"))?;
    send_on(&conn, summary, body).await
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Seen = Arc<Mutex<Vec<(String, String, String, String)>>>;

    /// Servidor de notificaciones falso, en un bus D-Bus privado.
    struct Fake {
        seen: Seen,
    }

    #[zbus::interface(name = "org.freedesktop.Notifications")]
    impl Fake {
        #[allow(clippy::too_many_arguments)]
        fn notify(
            &self,
            app_name: String,
            _replaces_id: u32,
            app_icon: String,
            summary: String,
            body: String,
            _actions: Vec<String>,
            _hints: HashMap<String, zbus::zvariant::OwnedValue>,
            _expire_timeout: i32,
        ) -> u32 {
            self.seen
                .lock()
                .unwrap()
                .push((app_name, app_icon, summary, body));
            7
        }
    }

    #[tokio::test]
    async fn notifications_go_through_dbus() {
        let Ok(mut daemon) = std::process::Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            eprintln!("omitida: no hay dbus-daemon");
            return;
        };
        let mut line = String::new();
        {
            use std::io::BufRead;
            let out = daemon.stdout.take().unwrap();
            std::io::BufReader::new(out).read_line(&mut line).unwrap();
        }
        let addr = line.trim().to_owned();
        let seen: Seen = Default::default();
        let _server = zbus::connection::Builder::address(addr.as_str())
            .unwrap()
            .name("org.freedesktop.Notifications")
            .unwrap()
            .serve_at(
                "/org/freedesktop/Notifications",
                Fake { seen: seen.clone() },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = zbus::connection::Builder::address(addr.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
        let id = send_on(&client, "CPU alta en «web»", "web: 97 % de CPU\u{7}")
            .await
            .unwrap();
        assert_eq!(id, 7);
        let v = seen.lock().unwrap().clone();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, APP_NAME);
        assert_eq!(v[0].1, ICON);
        assert_eq!(v[0].2, "CPU alta en «web»");
        assert_eq!(v[0].3, "web: 97 % de CPU", "sin caracteres de control");
        assert!(send_on(&client, "  ", "x").await.is_err(), "vacía");
        let long = "x".repeat(1000);
        send_on(&client, &long, &long).await.unwrap();
        let v = seen.lock().unwrap().clone();
        assert_eq!(v[1].2.chars().count(), MAX_SUMMARY);
        assert_eq!(v[1].3.chars().count(), MAX_BODY);
        let _ = daemon.kill();
        let _ = daemon.wait();
    }
}
