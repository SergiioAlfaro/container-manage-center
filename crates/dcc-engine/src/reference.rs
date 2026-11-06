//! Referencias de imagen (`[registro/]repositorio[:etiqueta][@digest]`).
//!
//! Sigue la gramática de `distribution/reference`, que es la que usan el CLI y
//! el daemon de Docker: así la aplicación rechaza lo mismo que rechazaría
//! `docker pull` y normaliza igual (`nginx` → `docker.io/library/nginx:latest`).

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const DOCKER_HUB: &str = "docker.io";
/// Clave con la que `docker login` guarda las credenciales de Docker Hub.
pub const DOCKER_HUB_AUTH_KEY: &str = "https://index.docker.io/v1/";
const MAX_NAME: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageRef {
    /// Registro (`docker.io`, `registry.example.com:5000`…).
    pub registry: String,
    /// Repositorio completo dentro del registro (`library/nginx`).
    pub repository: String,
    pub tag: Option<String>,
    pub digest: Option<String>,
}

/// Motivo por el que una referencia no es válida (código estable para la interfaz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RefError {
    #[error("la referencia está vacía")]
    Empty,
    #[error("la referencia es demasiado larga (máximo 255 caracteres)")]
    TooLong,
    #[error("el nombre del registro no es válido")]
    InvalidRegistry,
    #[error(
        "el nombre del repositorio no es válido: solo minúsculas, números y separadores . _ -"
    )]
    InvalidRepository,
    #[error(
        "la etiqueta no es válida: letras, números, _ . - (máximo 128, sin empezar por . o -)"
    )]
    InvalidTag,
    #[error("el digest no es válido (sha256:<64 caracteres hexadecimales>)")]
    InvalidDigest,
}

fn is_alnum_lower(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// `[a-z0-9]+(?:(?:[._]|__|[-]+)[a-z0-9]+)*`
pub fn valid_path_component(s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    if b.is_empty() || !is_alnum_lower(b[0]) || !is_alnum_lower(*b.last().unwrap()) {
        return false;
    }
    let mut i = 0;
    while i < b.len() {
        if is_alnum_lower(b[i]) {
            i += 1;
            continue;
        }
        // Separador: ".", "_", "__" o uno o más "-", siempre seguido de alfanumérico.
        let start = i;
        match b[i] {
            '.' => i += 1,
            '_' => {
                i += 1;
                if i < b.len() && b[i] == '_' {
                    i += 1;
                }
            }
            '-' => {
                while i < b.len() && b[i] == '-' {
                    i += 1;
                }
            }
            _ => return false,
        }
        if i == start || i >= b.len() || !is_alnum_lower(b[i]) {
            return false;
        }
    }
    true
}

fn valid_domain_component(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_alphanumeric()
        && b[b.len() - 1].is_ascii_alphanumeric()
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
}

pub fn valid_registry(s: &str) -> bool {
    let (host, port) = match s.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (s, None),
    };
    if let Some(p) = port {
        if p.is_empty()
            || !p.bytes().all(|c| c.is_ascii_digit())
            || p.parse::<u32>().map_or(true, |n| n > 65535)
        {
            return false;
        }
    }
    !host.is_empty() && host.split('.').all(valid_domain_component)
}

/// `[\w][\w.-]{0,127}`
pub fn valid_tag(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 128
        && (b[0].is_ascii_alphanumeric() || b[0] == b'_')
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

pub fn valid_digest(s: &str) -> bool {
    match s.split_once(':') {
        Some(("sha256", hex)) => {
            hex.len() == 64 && hex.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
        }
        Some(("sha512", hex)) => {
            hex.len() == 128 && hex.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
        }
        _ => false,
    }
}

/// Repositorio (sin registro): componentes separados por `/`.
pub fn valid_repository(s: &str) -> bool {
    !s.is_empty() && s.split('/').all(valid_path_component)
}

impl ImageRef {
    /// Analiza y normaliza una referencia como lo hace Docker.
    pub fn parse(input: &str) -> Result<Self, RefError> {
        let s = input.trim();
        if s.is_empty() {
            return Err(RefError::Empty);
        }
        if s.len() > MAX_NAME + 72 {
            return Err(RefError::TooLong);
        }
        let (rest, digest) = match s.split_once('@') {
            Some((r, d)) => {
                if !valid_digest(d) {
                    return Err(RefError::InvalidDigest);
                }
                (r, Some(d.to_owned()))
            }
            None => (s, None),
        };
        // La etiqueta va tras el último ":" que no forme parte del registro.
        let last_slash = rest.rfind('/');
        let (name, tag) = match rest.rfind(':') {
            Some(i) if last_slash.map_or(true, |j| i > j) => {
                (&rest[..i], Some(rest[i + 1..].to_owned()))
            }
            _ => (rest, None),
        };
        if let Some(t) = &tag {
            if !valid_tag(t) {
                return Err(RefError::InvalidTag);
            }
        }
        if name.is_empty() {
            return Err(RefError::InvalidRepository);
        }
        // Igual que splitDockerDomain: el primer componente es un registro si
        // contiene "." o ":", es "localhost" o tiene mayúsculas.
        let (registry, mut repository) = match name.split_once('/') {
            Some((first, path))
                if first.contains(['.', ':'])
                    || first == "localhost"
                    || first.to_lowercase() != first =>
            {
                if !valid_registry(first) {
                    return Err(RefError::InvalidRegistry);
                }
                (first.to_owned(), path.to_owned())
            }
            _ => (DOCKER_HUB.to_owned(), name.to_owned()),
        };
        if registry == "index.docker.io" {
            return ImageRef::parse(&s.replacen("index.docker.io", DOCKER_HUB, 1));
        }
        if !valid_repository(&repository) {
            return Err(RefError::InvalidRepository);
        }
        if registry == DOCKER_HUB && !repository.contains('/') {
            repository = format!("library/{repository}");
        }
        if registry.len() + 1 + repository.len() > MAX_NAME {
            return Err(RefError::TooLong);
        }
        Ok(Self {
            registry,
            repository,
            tag,
            digest,
        })
    }

    /// Nombre completo sin etiqueta (`docker.io/library/nginx`), como `fromImage`.
    pub fn name(&self) -> String {
        format!("{}/{}", self.registry, self.repository)
    }

    /// Nombre corto como lo muestra Docker (`nginx`, `usuario/app`, `registro:5000/app`).
    pub fn familiar_name(&self) -> String {
        if self.registry == DOCKER_HUB {
            self.repository
                .strip_prefix("library/")
                .unwrap_or(&self.repository)
                .to_owned()
        } else {
            self.name()
        }
    }

    /// Etiqueta o digest que se envía como `tag` al descargar (por defecto `latest`).
    pub fn api_tag(&self) -> String {
        self.digest
            .clone()
            .or_else(|| self.tag.clone())
            .unwrap_or_else(|| "latest".into())
    }

    /// Clave de las credenciales en la configuración de Docker.
    pub fn auth_key(&self) -> String {
        auth_key_for(&self.registry)
    }

    /// Referencia corta completa, con la etiqueta `latest` implícita añadida.
    pub fn familiar(&self) -> String {
        let mut s = self.familiar_name();
        match (&self.tag, &self.digest) {
            (Some(t), Some(d)) => s.push_str(&format!(":{t}@{d}")),
            (Some(t), None) => s.push_str(&format!(":{t}")),
            (None, Some(d)) => s.push_str(&format!("@{d}")),
            (None, None) => s.push_str(":latest"),
        }
        s
    }
}

pub fn auth_key_for(registry: &str) -> String {
    if registry == DOCKER_HUB {
        DOCKER_HUB_AUTH_KEY.to_owned()
    } else {
        registry.to_owned()
    }
}

impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.familiar())
    }
}

/// Valida el destino de `docker tag` (repositorio y etiqueta por separado).
pub fn validate_tag_target(repo: &str, tag: &str) -> Result<ImageRef, RefError> {
    if !valid_tag(tag) {
        return Err(RefError::InvalidTag);
    }
    let r = ImageRef::parse(&format!("{repo}:{tag}"))?;
    if r.digest.is_some() {
        return Err(RefError::InvalidRepository);
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> ImageRef {
        ImageRef::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    #[test]
    fn normalizes_like_docker() {
        let r = p("nginx");
        assert_eq!(
            (r.registry.as_str(), r.repository.as_str(), r.tag.as_deref()),
            ("docker.io", "library/nginx", None)
        );
        assert_eq!(r.name(), "docker.io/library/nginx");
        assert_eq!(r.api_tag(), "latest");
        assert_eq!(r.familiar(), "nginx:latest");
        assert_eq!(r.auth_key(), DOCKER_HUB_AUTH_KEY);

        let r = p("usuario/app:1.0");
        assert_eq!(r.repository, "usuario/app");
        assert_eq!(r.familiar(), "usuario/app:1.0");

        let r = p("localhost:5000/equipo/app:v2");
        assert_eq!(
            (r.registry.as_str(), r.repository.as_str()),
            ("localhost:5000", "equipo/app")
        );
        assert_eq!(r.familiar_name(), "localhost:5000/equipo/app");
        assert_eq!(r.auth_key(), "localhost:5000");

        let r = p("127.0.0.1:41234/dcc-test/pulled:1");
        assert_eq!(r.registry, "127.0.0.1:41234");

        let r = p("localhost/app");
        assert_eq!(r.registry, "localhost");

        let r = p("index.docker.io/library/alpine:3");
        assert_eq!(r.familiar(), "alpine:3");

        let d = format!("sha256:{}", "a".repeat(64));
        let r = p(&format!("ghcr.io/o/app@{d}"));
        assert_eq!(r.api_tag(), d);
        assert_eq!(r.familiar(), format!("ghcr.io/o/app@{d}"));
    }

    #[test]
    fn rejects_what_docker_rejects() {
        assert_eq!(ImageRef::parse(""), Err(RefError::Empty));
        assert_eq!(ImageRef::parse("Nginx"), Err(RefError::InvalidRepository));
        assert_eq!(ImageRef::parse("nginx:"), Err(RefError::InvalidTag));
        assert_eq!(ImageRef::parse("nginx:-x"), Err(RefError::InvalidTag));
        assert_eq!(
            ImageRef::parse(&format!("nginx:{}", "a".repeat(129))),
            Err(RefError::InvalidTag)
        );
        assert_eq!(ImageRef::parse("a..b"), Err(RefError::InvalidRepository));
        assert_eq!(ImageRef::parse("a/-b"), Err(RefError::InvalidRepository));
        assert_eq!(
            ImageRef::parse("reg-.io/app"),
            Err(RefError::InvalidRegistry)
        );
        assert_eq!(
            ImageRef::parse("reg.io:99999/app"),
            Err(RefError::InvalidRegistry)
        );
        assert_eq!(
            ImageRef::parse("app@sha256:abc"),
            Err(RefError::InvalidDigest)
        );
        assert_eq!(ImageRef::parse("a b"), Err(RefError::InvalidRepository));
        assert!(ImageRef::parse("nginx;rm -rf /").is_err());
    }

    #[test]
    fn path_components_follow_the_grammar() {
        for ok in ["a", "a.b", "a_b", "a__b", "a-b", "a---b", "a1.b2_c3"] {
            assert!(valid_path_component(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "a___b", "a..b", "a._b", "A", "a/b"] {
            assert!(!valid_path_component(bad), "{bad}");
        }
    }

    #[test]
    fn tag_target_is_validated() {
        assert_eq!(
            validate_tag_target("miapp", "v1").unwrap().familiar(),
            "miapp:v1"
        );
        assert_eq!(
            validate_tag_target("miapp", "v 1"),
            Err(RefError::InvalidTag)
        );
        assert_eq!(
            validate_tag_target("Mi", "v1"),
            Err(RefError::InvalidRepository)
        );
    }
}
