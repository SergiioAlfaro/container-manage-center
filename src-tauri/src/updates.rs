// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Fase 9: comprobación de versiones nuevas de la aplicación (D-060).
//!
//! La aplicación **solo consulta y avisa**: nunca descarga ni instala nada (la
//! actualización se hace con el gestor de paquetes, con los permisos del
//! usuario). Solo se conecta si el usuario configura una fuente y pulsa
//! "Comprobar ahora" o activa la comprobación al iniciar.
//!
//! Fuentes admitidas:
//! * `propietario/repositorio` de GitHub → `https://api.github.com/repos/…/releases/latest`
//!   (respuesta con `tag_name`, `html_url`, `body`, `published_at`);
//! * la dirección HTTPS de un archivo JSON con `version` y, opcionalmente,
//!   `url`, `notes` y `date`.
//!
//! Solo HTTPS (con los certificados del sistema y el proxy de `HTTPS_PROXY`),
//! salvo `http://` hacia la propia máquina (`127.0.0.1`, `localhost`, `[::1]`),
//! útil para un espejo local. Respuesta de 1 MiB como mucho y 15 s de espera.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use ts_rs::TS;

pub const MAX_BODY: usize = 1024 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(15);
pub const MAX_NOTES: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateSource {
    GitHub { owner: String, repo: String },
    Url(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("No hay ninguna fuente de actualizaciones configurada.")]
    NoSource,
    #[error(
        "Fuente no válida: indica «propietario/repositorio» de GitHub o una dirección https://"
    )]
    InvalidSource,
    #[error("Solo se admiten direcciones https:// (o http:// hacia esta misma máquina).")]
    InsecureSource,
    #[error("No se pudo consultar {url}: {reason}")]
    Network { url: String, reason: String },
    #[error("{url} respondió con el estado HTTP {status}.")]
    Http { url: String, status: u16 },
    #[error("La respuesta de {url} supera 1 MiB.")]
    TooLarge { url: String },
    #[error("La respuesta de {url} no tiene el formato esperado: {reason}")]
    Format { url: String, reason: String },
}

impl UpdateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoSource => "updates.noSource",
            Self::InvalidSource => "updates.invalidSource",
            Self::InsecureSource => "updates.insecureSource",
            Self::Network { .. } => "updates.network",
            Self::Http { .. } | Self::TooLarge { .. } | Self::Format { .. } => "updates.response",
        }
    }
}

fn valid_github_part(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

impl UpdateSource {
    /// `Ok(None)` si está vacía.
    pub fn parse(s: &str) -> Result<Option<Self>, UpdateError> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(None);
        }
        if s.len() > cmc_store::UpdatesConfig::MAX_SOURCE_LEN || s.chars().any(char::is_control) {
            return Err(UpdateError::InvalidSource);
        }
        if let Some(rest) = s.strip_prefix("https://") {
            let host = rest.split(['/', '?', '#']).next().unwrap_or("");
            if host.is_empty() || host.contains('@') || s.contains(char::is_whitespace) {
                return Err(UpdateError::InvalidSource);
            }
            return Ok(Some(Self::Url(s.to_owned())));
        }
        if let Some(rest) = s.strip_prefix("http://") {
            let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
            let host = if authority.starts_with('[') {
                authority.split_inclusive(']').next().unwrap_or("")
            } else {
                authority.split(':').next().unwrap_or("")
            };
            if is_loopback_host(host) && !s.contains(char::is_whitespace) {
                return Ok(Some(Self::Url(s.to_owned())));
            }
            return Err(UpdateError::InsecureSource);
        }
        if s.contains("://") {
            return Err(UpdateError::InsecureSource);
        }
        match s.split_once('/') {
            Some((owner, repo)) if valid_github_part(owner) && valid_github_part(repo) => {
                Ok(Some(Self::GitHub {
                    owner: owner.to_owned(),
                    repo: repo.to_owned(),
                }))
            }
            _ => Err(UpdateError::InvalidSource),
        }
    }

    pub fn url(&self) -> String {
        match self {
            Self::GitHub { owner, repo } => {
                format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
            }
            Self::Url(u) => u.clone(),
        }
    }
}

/// Servidor (`host[:puerto]`, en minúsculas) de una dirección `https://` sin
/// usuario ni contraseña; `None` si no lo es.
pub fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(authority.to_ascii_lowercase())
}

/// Servidores cuyas páginas de versión se pueden abrir (E2, CMC-SEC-16,
/// D-091): `github.com` y el de la fuente de actualizaciones propia (la
/// guardada y la última consultada). `api.github.com` no sirve páginas.
pub fn release_page_hosts(saved_source: &str, last_checked_url: Option<&str>) -> Vec<String> {
    let mut hosts = vec!["github.com".to_owned()];
    if let Ok(Some(UpdateSource::Url(u))) = UpdateSource::parse(saved_source) {
        hosts.extend(https_host(&u));
    }
    hosts.extend(last_checked_url.and_then(https_host));
    hosts.retain(|h| h != "api.github.com");
    hosts.dedup();
    hosts
}

/// ¿Se puede abrir este enlace de versión? `https://`, sin espacios ni
/// caracteres de control, hasta 2048 caracteres y en uno de `hosts`.
pub fn release_page_allowed(url: &str, hosts: &[String]) -> bool {
    url.len() <= 2048
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && https_host(url).is_some_and(|h| hosts.contains(&h))
}

/// Versión publicada, leída de la respuesta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: semver::Version,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub published: Option<String>,
}

fn parse_version(raw: &str) -> Option<semver::Version> {
    let v = raw.trim();
    let v = v
        .strip_prefix('v')
        .or_else(|| v.strip_prefix('V'))
        .unwrap_or(v);
    semver::Version::parse(v).ok()
}

/// Lee la respuesta (formato de GitHub o el propio). Solo se aceptan enlaces
/// `https://` para no ofrecer abrir otra cosa.
pub fn parse_release(body: &[u8], url: &str) -> Result<Release, UpdateError> {
    let format = |reason: &str| UpdateError::Format {
        url: url.to_owned(),
        reason: reason.to_owned(),
    };
    let v: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format(&format!("no es JSON ({e})")))?;
    let obj = v
        .as_object()
        .ok_or_else(|| format("no es un objeto JSON"))?;
    let text = |k: &str| obj.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    if obj.get("draft").and_then(|x| x.as_bool()) == Some(true)
        || obj.get("prerelease").and_then(|x| x.as_bool()) == Some(true)
    {
        return Err(format(
            "la versión publicada es un borrador o una versión preliminar",
        ));
    }
    let raw = text("version")
        .or_else(|| text("tag_name"))
        .ok_or_else(|| format("falta «version» (o «tag_name»)"))?;
    let version = parse_version(&raw)
        .ok_or_else(|| format(&format!("«{raw}» no es un número de versión (X.Y.Z)")))?;
    let link = text("url")
        .or_else(|| text("html_url"))
        .filter(|u| u.starts_with("https://") && !u.contains(char::is_whitespace));
    let notes = text("notes").or_else(|| text("body")).map(|n| {
        let n = n.trim().to_owned();
        if n.chars().count() > MAX_NOTES {
            n.chars().take(MAX_NOTES).collect::<String>() + "…"
        } else {
            n
        }
    });
    Ok(Release {
        version,
        url: link,
        notes: notes.filter(|n| !n.is_empty()),
        published: text("published_at").or_else(|| text("date")),
    })
}

/// Resultado de una comprobación.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: String,
    /// La versión publicada es más nueva que la instalada.
    pub newer: bool,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub published: Option<String>,
    /// Dirección consultada.
    pub checked_url: String,
    pub checked_at: DateTime<Utc>,
}

pub fn compare(current: &str, r: Release, checked_url: String) -> UpdateCheck {
    let cur = parse_version(current);
    UpdateCheck {
        current: current.to_owned(),
        latest: r.version.to_string(),
        newer: cur.is_some_and(|c| r.version > c),
        url: r.url,
        notes: r.notes,
        published: r.published,
        checked_url,
        checked_at: Utc::now(),
    }
}

/// Consulta la fuente (una petición GET) y compara con la versión instalada.
pub async fn fetch(source: &UpdateSource, current: &str) -> Result<UpdateCheck, UpdateError> {
    let url = source.url();
    let net = |e: reqwest::Error| UpdateError::Network {
        url: url.clone(),
        reason: if e.is_timeout() {
            "no respondió en 15 s".to_owned()
        } else if e.is_connect() {
            format!("no se pudo conectar ({})", root_cause(&e))
        } else {
            root_cause(&e)
        },
    };
    let mut builder = reqwest::Client::builder();
    if url.starts_with("http://") {
        // Solo se admite http:// hacia esta máquina: nunca a través de un proxy.
        builder = builder.no_proxy();
    }
    let client = builder
        .user_agent(format!("container-manage-center/{current}"))
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            // Las redirecciones también deben ser https.
            if attempt.previous().len() > 5 {
                attempt.error("demasiadas redirecciones")
            } else if attempt.url().scheme() != "https" {
                attempt.error("redirección a una dirección que no es https")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(net)?;
    let mut req = client.get(&url).header("Accept", "application/json");
    if matches!(source, UpdateSource::GitHub { .. }) {
        req = req.header("Accept", "application/vnd.github+json");
    }
    let mut resp = req.send().await.map_err(net)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(UpdateError::Http {
            url,
            status: status.as_u16(),
        });
    }
    if resp.content_length().is_some_and(|n| n > MAX_BODY as u64) {
        return Err(UpdateError::TooLarge { url });
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(net)? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(UpdateError::TooLarge { url });
        }
        body.extend_from_slice(&chunk);
    }
    let release = parse_release(&body, &url)?;
    Ok(compare(current, release, url))
}

fn root_cause(e: &(dyn std::error::Error + 'static)) -> String {
    let mut cur = e;
    while let Some(next) = cur.source() {
        cur = next;
    }
    cur.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_validated() {
        assert_eq!(UpdateSource::parse("  ").unwrap(), None);
        // Los espacios y saltos de línea de los extremos (texto pegado) se quitan.
        assert!(UpdateSource::parse(" org/repo\n").unwrap().is_some());
        assert_eq!(
            UpdateSource::parse("mi-org/container-manage-center").unwrap(),
            Some(UpdateSource::GitHub {
                owner: "mi-org".into(),
                repo: "container-manage-center".into()
            })
        );
        assert_eq!(
            UpdateSource::parse("mi-org/cmc").unwrap().unwrap().url(),
            "https://api.github.com/repos/mi-org/cmc/releases/latest"
        );
        assert_eq!(
            UpdateSource::parse("https://descargas.ejemplo.es/cmc/latest.json").unwrap(),
            Some(UpdateSource::Url(
                "https://descargas.ejemplo.es/cmc/latest.json".into()
            ))
        );
        for local in [
            "http://127.0.0.1:8080/latest.json",
            "http://localhost/x.json",
            "http://[::1]:9000/v",
        ] {
            assert!(UpdateSource::parse(local).unwrap().is_some(), "{local}");
        }
        for insecure in [
            "http://ejemplo.es/latest.json",
            "ftp://ejemplo.es/x",
            "http://127.0.0.1.ejemplo.es/x",
            "file:///etc/passwd",
        ] {
            assert_eq!(
                UpdateSource::parse(insecure),
                Err(UpdateError::InsecureSource),
                "{insecure}"
            );
        }
        for bad in [
            "solo-nombre",
            "a/b/c",
            "../x",
            "a b/c",
            "https://",
            "https://usuario@host/x",
            "org/re\npo",
            "org/re\tpo",
            "https://h/ruta con espacios",
        ] {
            assert!(UpdateSource::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn github_and_manifest_formats() {
        let gh = br#"{"tag_name":"v0.10.0","html_url":"https://github.com/o/r/releases/tag/v0.10.0","body":"Novedades","published_at":"2026-12-01T10:00:00Z","draft":false,"prerelease":false}"#;
        let r = parse_release(gh, "u").unwrap();
        assert_eq!(r.version, semver::Version::new(0, 10, 0));
        assert_eq!(
            r.url.as_deref(),
            Some("https://github.com/o/r/releases/tag/v0.10.0")
        );
        assert_eq!(r.notes.as_deref(), Some("Novedades"));
        let c = compare("0.9.0", r, "u".into());
        assert!(c.newer);
        assert_eq!((c.current.as_str(), c.latest.as_str()), ("0.9.0", "0.10.0"));

        let own = br#"{"version":"0.9.0","url":"javascript:alert(1)","notes":"  "}"#;
        let r = parse_release(own, "u").unwrap();
        assert_eq!(r.url, None, "solo enlaces https");
        assert_eq!(r.notes, None);
        assert!(
            !compare("0.9.0", r.clone(), "u".into()).newer,
            "misma versión"
        );
        assert!(
            !compare("0.9.1", r, "u".into()).newer,
            "instalada más nueva"
        );

        // 0.9.0-rc.1 < 0.9.0 (orden de semver).
        let r = parse_release(br#"{"version":"0.9.0"}"#, "u").unwrap();
        assert!(compare("0.9.0-rc.1", r, "u".into()).newer);
    }

    #[test]
    fn bad_responses_are_explained() {
        for (body, needle) in [
            (&b"<html>"[..], "no es JSON"),
            (b"[1,2]", "no es un objeto"),
            (b"{\"name\":\"x\"}", "falta"),
            (b"{\"version\":\"latest\"}", "no es un número de versión"),
            (
                b"{\"tag_name\":\"v1.0.0\",\"prerelease\":true}",
                "preliminar",
            ),
        ] {
            let e = parse_release(body, "https://h/x").unwrap_err();
            assert!(e.to_string().contains(needle), "{e}");
            assert_eq!(e.code(), "updates.response");
        }
        let long = format!(
            "{{\"version\":\"1.0.0\",\"notes\":\"{}\"}}",
            "a".repeat(MAX_NOTES + 50)
        );
        let r = parse_release(long.as_bytes(), "u").unwrap();
        assert_eq!(r.notes.unwrap().chars().count(), MAX_NOTES + 1);
    }

    /// Servidor HTTP mínimo en 127.0.0.1 que responde siempre lo mismo.
    async fn serve(response: Vec<u8>) -> (String, tokio::task::JoinHandle<usize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let h = tokio::spawn(async move {
            let mut served = 0;
            while let Ok(Ok((mut c, _))) =
                tokio::time::timeout(Duration::from_secs(5), l.accept()).await
            {
                let mut buf = [0u8; 4096];
                let _ = c.read(&mut buf).await;
                let _ = c.write_all(&response).await;
                let _ = c.shutdown().await;
                served += 1;
            }
            served
        });
        (format!("http://{addr}"), h)
    }

    fn http(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut r = format!("HTTP/1.1 {status}\r\nConnection: close\r\n{headers}\r\n").into_bytes();
        r.extend_from_slice(body);
        r
    }

    fn src(url: &str) -> UpdateSource {
        UpdateSource::parse(url).unwrap().unwrap()
    }

    #[tokio::test]
    async fn real_requests_against_a_local_server() {
        let body =
            br#"{"version":"0.9.3","url":"https://ejemplo.es/v0.9.3","notes":"Correcciones"}"#;
        let (base, _h) = serve(http(
            "200 OK",
            &format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            ),
            body,
        ))
        .await;
        let c = fetch(&src(&format!("{base}/latest.json")), "0.9.0")
            .await
            .unwrap();
        assert!(c.newer);
        assert_eq!(c.latest, "0.9.3");
        assert_eq!(c.checked_url, format!("{base}/latest.json"));
        assert_eq!(c.url.as_deref(), Some("https://ejemplo.es/v0.9.3"));

        let (base, _h) = serve(http("404 Not Found", "Content-Length: 0\r\n", b"")).await;
        let e = fetch(&src(&format!("{base}/x")), "0.9.0")
            .await
            .unwrap_err();
        assert!(matches!(e, UpdateError::Http { status: 404, .. }), "{e}");

        // Más de 1 MiB, declarado y sin declarar.
        let (base, _h) = serve(http("200 OK", "Content-Length: 2000000\r\n", b"{")).await;
        let e = fetch(&src(&format!("{base}/x")), "0.9.0")
            .await
            .unwrap_err();
        assert!(matches!(e, UpdateError::TooLarge { .. }), "{e}");
        let big = vec![b' '; MAX_BODY + 10];
        let (base, _h) = serve(http("200 OK", "", &big)).await;
        let e = fetch(&src(&format!("{base}/x")), "0.9.0")
            .await
            .unwrap_err();
        assert!(matches!(e, UpdateError::TooLarge { .. }), "{e}");

        // Una redirección a algo que no es https no se sigue.
        let (base, _h) = serve(http(
            "302 Found",
            "Location: http://127.0.0.1:9/otra\r\nContent-Length: 0\r\n",
            b"",
        ))
        .await;
        let e = fetch(&src(&format!("{base}/x")), "0.9.0")
            .await
            .unwrap_err();
        assert!(e.to_string().contains("https"), "{e}");
        assert_eq!(e.code(), "updates.network");

        // Nada escuchando.
        let free = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let e = fetch(&src(&format!("http://{free}/x")), "0.9.0")
            .await
            .unwrap_err();
        assert!(e.to_string().contains("no se pudo conectar"), "{e}");
    }

    /// HTTPS real (TLS con los certificados del sistema y el proxy del entorno).
    /// Solo con CMC_NET_TESTS=1: las pruebas normales no salen a Internet.
    #[tokio::test]
    async fn real_https_when_allowed() {
        if std::env::var("CMC_NET_TESTS").as_deref() != Ok("1") {
            eprintln!("omitida: define CMC_NET_TESTS=1 para consultar un servidor HTTPS real");
            return;
        }
        let url = std::env::var("CMC_NET_TEST_URL")
            .unwrap_or_else(|_| "https://registry.npmjs.org/@tauri-apps%2fcli/latest".into());
        let c = fetch(&src(&url), "0.0.1").await.unwrap();
        assert!(c.newer, "{c:?}");
        assert!(semver::Version::parse(&c.latest).is_ok());
        eprintln!("HTTPS real: {url} → {}", c.latest);
    }

    #[test]
    fn release_pages_only_from_github_or_the_own_source() {
        let gh = release_page_hosts(
            "org/repo",
            Some("https://api.github.com/repos/org/repo/releases/latest"),
        );
        assert_eq!(gh, vec!["github.com".to_owned()]);
        assert!(release_page_allowed(
            "https://github.com/org/repo/releases/tag/v1.2.0",
            &gh
        ));
        for bad in [
            "https://evil.example/x",
            "https://github.com.evil.example/x",
            "https://user@github.com/x",
            "http://github.com/x",
            "https://github.com/a b",
            "file:///etc/passwd",
            "javascript:alert(1)",
        ] {
            assert!(!release_page_allowed(bad, &gh), "{bad}");
        }
        let own = release_page_hosts(
            "https://descargas.ejemplo.es/cmc/latest.json",
            Some("https://otra.ejemplo.es/latest.json"),
        );
        assert!(release_page_allowed(
            "https://descargas.ejemplo.es/cmc/1.2.0",
            &own
        ));
        assert!(release_page_allowed(
            "https://DESCARGAS.ejemplo.es/cmc/1.2.0",
            &own
        ));
        assert!(
            release_page_allowed("https://otra.ejemplo.es/notas", &own),
            "última consultada"
        );
        assert!(!release_page_allowed("https://tercera.ejemplo.es/", &own));
    }
}
