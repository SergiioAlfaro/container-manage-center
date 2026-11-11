//! Fase 5: redes. Listado (cruzado con los contenedores), inspección, creación
//! con validación, eliminación, limpieza y conexión o desconexión de
//! contenedores. Todas son peticiones reales registradas en la actividad.
//!
//! Comportamiento de Docker comprobado (Docker 29.9 y 24.0, ver D-047):
//! - `GET /networks` no incluye los contenedores; `GET /networks/{id}` solo
//!   incluye los que están en marcha. Los detenidos que tienen la red
//!   configurada se obtienen de la lista de contenedores (por nombre de red).
//! - `DELETE /networks/{id}` falla (403 "has active endpoints") si hay
//!   contenedores en marcha, pero **elimina** la red aunque la tengan
//!   configurada contenedores detenidos, que después no pueden arrancar.
//! - Docker acepta nombres de red con espacios; las operaciones usan siempre
//!   el ID y el nombre solo se muestra.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;

use bollard::models::{
    EndpointIpamConfig, EndpointSettings, Ipam, IpamConfig, NetworkConnectRequest,
    NetworkCreateRequest, NetworkDisconnectRequest,
};
use dcc_activity::{quote::render_argv, Category};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::build::http;
use crate::client::{ApiOp, Engine};
use crate::containers::ContainerSummary;
use crate::error::EngineError;
use crate::resources::{path_with_query, validate_ref};
use crate::spec::{issue, KeyValue, Severity, SpecIssue};

/// Redes que crea Docker y no se pueden eliminar.
pub const PREDEFINED: [&str; 3] = ["bridge", "host", "none"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkMember {
    pub id: String,
    pub name: String,
    /// Estado del contenedor (running, exited…); vacío si Docker no lo indica.
    pub state: String,
    /// Dirección con prefijo (`172.18.0.2/16`); solo contenedores en marcha.
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub mac: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkSummary {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub created: Option<String>,
    pub internal: bool,
    pub attachable: bool,
    pub ingress: bool,
    pub enable_ipv4: bool,
    pub enable_ipv6: bool,
    pub subnets: Vec<String>,
    pub gateways: Vec<String>,
    pub labels: BTreeMap<String, String>,
    /// `bridge`, `host` o `none`: las crea Docker y no se pueden eliminar.
    pub predefined: bool,
    pub compose_project: Option<String>,
    /// Contenedores (de cualquier estado) que tienen esta red configurada.
    pub containers: Vec<NetworkMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IpamPoolInfo {
    pub subnet: String,
    pub gateway: String,
    pub ip_range: String,
    pub auxiliary: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkDetails {
    pub summary: NetworkSummary,
    pub ipam_driver: String,
    pub pools: Vec<IpamPoolInfo>,
    pub options: BTreeMap<String, String>,
    /// JSON completo devuelto por Docker.
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreatedNetwork {
    pub id: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct IpamPool {
    pub subnet: String,
    pub gateway: String,
    pub ip_range: String,
}

impl IpamPool {
    fn is_empty(&self) -> bool {
        self.subnet.trim().is_empty()
            && self.gateway.trim().is_empty()
            && self.ip_range.trim().is_empty()
    }
}

/// Red nueva. Fuente única de la validación, del cuerpo de
/// `POST /networks/create` y del comando `docker network create` equivalente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct NetworkSpec {
    pub name: String,
    pub driver: String,
    pub internal: bool,
    pub attachable: bool,
    pub enable_ipv6: bool,
    pub ipv4: IpamPool,
    pub ipv6: IpamPool,
    pub labels: Vec<KeyValue>,
    pub options: Vec<KeyValue>,
}

impl Default for NetworkSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            driver: "bridge".into(),
            internal: false,
            attachable: false,
            enable_ipv6: false,
            ipv4: IpamPool::default(),
            ipv6: IpamPool::default(),
            labels: vec![],
            options: vec![],
        }
    }
}

/// Opciones de `docker network connect`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ConnectOptions {
    pub aliases: Vec<String>,
    pub ipv4: String,
    pub ipv6: String,
}

// ------------------------------------------------------------------ CIDR

/// Subred `dirección/prefijo` ya validada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    pub addr: IpAddr,
    pub prefix: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CidrError {
    Invalid,
    /// La dirección tiene bits de equipo: se indica la dirección de red correcta.
    HostBits(String),
}

fn bits(a: IpAddr) -> (u128, u8) {
    match a {
        IpAddr::V4(v) => (u32::from(v) as u128, 32),
        IpAddr::V6(v) => (u128::from(v), 128),
    }
}

fn mask(prefix: u8, width: u8) -> u128 {
    if prefix == 0 {
        return 0;
    }
    let all = if width == 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    };
    let host = width - prefix;
    (all >> host) << host
}

impl Cidr {
    pub fn parse(s: &str) -> Result<Self, CidrError> {
        let (a, p) = s.trim().split_once('/').ok_or(CidrError::Invalid)?;
        let addr: IpAddr = a.parse().map_err(|_| CidrError::Invalid)?;
        let prefix: u8 = p.parse().map_err(|_| CidrError::Invalid)?;
        let (v, width) = bits(addr);
        if prefix > width || p.starts_with('+') || (p.len() > 1 && p.starts_with('0')) {
            return Err(CidrError::Invalid);
        }
        let net = v & mask(prefix, width);
        if net != v {
            let fixed = match addr {
                IpAddr::V4(_) => IpAddr::V4((net as u32).into()),
                IpAddr::V6(_) => IpAddr::V6(net.into()),
            };
            return Err(CidrError::HostBits(format!("{fixed}/{prefix}")));
        }
        Ok(Self { addr, prefix })
    }

    pub fn is_v4(&self) -> bool {
        self.addr.is_ipv4()
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let (v, w) = bits(self.addr);
        let (x, wx) = bits(ip);
        w == wx && (x & mask(self.prefix, w)) == v
    }

    pub fn contains_cidr(&self, other: &Cidr) -> bool {
        other.prefix >= self.prefix && self.contains(other.addr)
    }
}

// ------------------------------------------------------------------ validación

fn valid_name(s: &str) -> bool {
    validate_ref("red", s).is_ok()
}

fn valid_driver(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':' | '/'))
}

fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 255
        && !k.contains('=')
        && !k.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Alias de red: nombre DNS (letras, dígitos, `-`, `_`, `.`).
pub fn valid_alias(a: &str) -> bool {
    !a.is_empty()
        && a.len() <= 253
        && a.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && a.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn kv_issues(v: &mut Vec<SpecIssue>, items: &[KeyValue], field: &str, code: &str) {
    let mut seen = HashSet::new();
    for (i, kv) in items.iter().enumerate() {
        let k = kv.key.trim();
        if k.is_empty() && kv.value.trim().is_empty() {
            continue;
        }
        if !valid_key(k) {
            v.push(issue(
                Severity::Error,
                format!("{field}.{i}.key"),
                &format!("{code}.keyInvalid"),
                &[("key", k.to_owned())],
            ));
        } else if !seen.insert(k.to_owned()) {
            v.push(issue(
                Severity::Error,
                format!("{field}.{i}.key"),
                &format!("{code}.duplicate"),
                &[("key", k.to_owned())],
            ));
        }
    }
}

fn pool_issues(v: &mut Vec<SpecIssue>, pool: &IpamPool, family: &str, want_v4: bool) {
    let subnet = pool.subnet.trim();
    let gateway = pool.gateway.trim();
    let range = pool.ip_range.trim();
    let sub = if subnet.is_empty() {
        if !gateway.is_empty() || !range.is_empty() {
            v.push(issue(
                Severity::Error,
                format!("{family}.subnet"),
                "subnet.required",
                &[],
            ));
        }
        None
    } else {
        match Cidr::parse(subnet) {
            Ok(c) if c.is_v4() != want_v4 => {
                v.push(issue(
                    Severity::Error,
                    format!("{family}.subnet"),
                    "subnet.family",
                    &[("value", subnet.to_owned())],
                ));
                None
            }
            Ok(c) => Some(c),
            Err(CidrError::HostBits(fixed)) => {
                v.push(issue(
                    Severity::Error,
                    format!("{family}.subnet"),
                    "subnet.hostBits",
                    &[("value", subnet.to_owned()), ("fixed", fixed)],
                ));
                None
            }
            Err(CidrError::Invalid) => {
                v.push(issue(
                    Severity::Error,
                    format!("{family}.subnet"),
                    "subnet.invalid",
                    &[("value", subnet.to_owned())],
                ));
                None
            }
        }
    };
    if !gateway.is_empty() {
        match gateway.parse::<IpAddr>() {
            Ok(ip) if ip.is_ipv4() != want_v4 => v.push(issue(
                Severity::Error,
                format!("{family}.gateway"),
                "gateway.family",
                &[("value", gateway.to_owned())],
            )),
            Ok(ip) => {
                if let Some(c) = sub {
                    if !c.contains(ip) {
                        v.push(issue(
                            Severity::Error,
                            format!("{family}.gateway"),
                            "gateway.outside",
                            &[("value", gateway.to_owned()), ("subnet", subnet.to_owned())],
                        ));
                    }
                }
            }
            Err(_) => v.push(issue(
                Severity::Error,
                format!("{family}.gateway"),
                "gateway.invalid",
                &[("value", gateway.to_owned())],
            )),
        }
    }
    if !range.is_empty() {
        match Cidr::parse(range) {
            Ok(r) if r.is_v4() != want_v4 => v.push(issue(
                Severity::Error,
                format!("{family}.ipRange"),
                "ipRange.family",
                &[("value", range.to_owned())],
            )),
            Ok(r) => {
                if let Some(c) = sub {
                    if !c.contains_cidr(&r) {
                        v.push(issue(
                            Severity::Error,
                            format!("{family}.ipRange"),
                            "ipRange.outside",
                            &[("value", range.to_owned()), ("subnet", subnet.to_owned())],
                        ));
                    }
                }
            }
            Err(CidrError::HostBits(fixed)) => v.push(issue(
                Severity::Error,
                format!("{family}.ipRange"),
                "ipRange.hostBits",
                &[("value", range.to_owned()), ("fixed", fixed)],
            )),
            Err(CidrError::Invalid) => v.push(issue(
                Severity::Error,
                format!("{family}.ipRange"),
                "ipRange.invalid",
                &[("value", range.to_owned())],
            )),
        }
    }
}

fn kv_map(items: &[KeyValue]) -> HashMap<String, String> {
    items
        .iter()
        .filter(|kv| !kv.key.trim().is_empty())
        .map(|kv| (kv.key.trim().to_owned(), kv.value.clone()))
        .collect()
}

fn kv_sorted(items: &[KeyValue]) -> Vec<(String, String)> {
    let mut v: Vec<_> = items
        .iter()
        .filter(|kv| !kv.key.trim().is_empty())
        .map(|kv| (kv.key.trim().to_owned(), kv.value.clone()))
        .collect();
    v.sort();
    v
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

impl NetworkSpec {
    pub fn validate(&self) -> Vec<SpecIssue> {
        use Severity::*;
        let mut v = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            v.push(issue(Error, "name", "name.required", &[]));
        } else if !valid_name(name) {
            v.push(issue(
                Error,
                "name",
                "name.invalid",
                &[("value", name.to_owned())],
            ));
        } else if PREDEFINED.contains(&name) {
            v.push(issue(
                Error,
                "name",
                "name.reserved",
                &[("value", name.to_owned())],
            ));
        }
        let driver = self.driver.trim();
        if !valid_driver(driver) {
            v.push(issue(
                Error,
                "driver",
                "driver.invalid",
                &[("value", driver.to_owned())],
            ));
        } else {
            match driver {
                "host" | "null" => v.push(issue(
                    Error,
                    "driver",
                    "driver.singleton",
                    &[("value", driver.to_owned())],
                )),
                "overlay" => v.push(issue(Warning, "driver", "driver.swarm", &[])),
                "macvlan" | "ipvlan" if !self.options.iter().any(|o| o.key.trim() == "parent") => {
                    v.push(issue(Warning, "options", "driver.noParent", &[]))
                }
                _ => {}
            }
        }
        pool_issues(&mut v, &self.ipv4, "ipv4", true);
        if !self.ipv6.is_empty() {
            if !self.enable_ipv6 {
                v.push(issue(Error, "ipv6.subnet", "ipv6.disabled", &[]));
            }
            pool_issues(&mut v, &self.ipv6, "ipv6", false);
        }
        kv_issues(&mut v, &self.labels, "labels", "label");
        kv_issues(&mut v, &self.options, "options", "option");
        v
    }

    pub fn has_errors(issues: &[SpecIssue]) -> bool {
        issues.iter().any(|i| i.severity == Severity::Error)
    }

    fn pools(&self) -> Vec<IpamConfig> {
        let mut out = Vec::new();
        for p in [&self.ipv4, &self.ipv6] {
            if p.is_empty() || (std::ptr::eq(p, &self.ipv6) && !self.enable_ipv6) {
                continue;
            }
            out.push(IpamConfig {
                subnet: non_empty(&p.subnet),
                ip_range: non_empty(&p.ip_range),
                gateway: non_empty(&p.gateway),
                auxiliary_addresses: None,
            });
        }
        out
    }

    /// Cuerpo de `POST /networks/create`: solo los campos que el usuario indicó.
    pub fn to_create_body(&self) -> NetworkCreateRequest {
        let pools = self.pools();
        let labels = kv_map(&self.labels);
        let options = kv_map(&self.options);
        NetworkCreateRequest {
            name: self.name.trim().to_owned(),
            driver: Some(self.driver.trim().to_owned()),
            internal: self.internal.then_some(true),
            attachable: self.attachable.then_some(true),
            enable_ipv6: self.enable_ipv6.then_some(true),
            ipam: (!pools.is_empty()).then(|| Ipam {
                driver: Some("default".into()),
                config: Some(pools),
                options: None,
            }),
            options: (!options.is_empty()).then_some(options),
            labels: (!labels.is_empty()).then_some(labels),
            ..Default::default()
        }
    }

    /// Cuerpo que se envía de verdad: el de [`Self::to_create_body`] con
    /// `CheckDuplicate: true`, como hace el CLI. Docker < 25 (API < 1.44) crea
    /// redes con nombre repetido sin ese campo (solo con un aviso); Docker 25 y
    /// posteriores siempre lo comprueban e ignoran el campo (D-048).
    pub fn wire_body(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self.to_create_body()).unwrap_or_default();
        v["CheckDuplicate"] = serde_json::Value::Bool(true);
        v
    }

    /// Argumentos de `docker network create …` (sin el programa).
    pub fn cli_args(&self) -> Vec<String> {
        let mut a: Vec<String> = vec!["network".into(), "create".into()];
        let mut push = |k: &str, v: String| {
            a.push(k.into());
            a.push(v);
        };
        push("--driver", self.driver.trim().to_owned());
        for p in [&self.ipv4, &self.ipv6] {
            if p.is_empty() || (std::ptr::eq(p, &self.ipv6) && !self.enable_ipv6) {
                continue;
            }
            if let Some(s) = non_empty(&p.subnet) {
                push("--subnet", s);
            }
            if let Some(s) = non_empty(&p.ip_range) {
                push("--ip-range", s);
            }
            if let Some(s) = non_empty(&p.gateway) {
                push("--gateway", s);
            }
        }
        for (k, v) in kv_sorted(&self.labels) {
            push("--label", format!("{k}={v}"));
        }
        for (k, v) in kv_sorted(&self.options) {
            push("--opt", format!("{k}={v}"));
        }
        if self.enable_ipv6 {
            a.push("--ipv6".into());
        }
        if self.internal {
            a.push("--internal".into());
        }
        if self.attachable {
            a.push("--attachable".into());
        }
        a.push(self.name.trim().to_owned());
        a
    }

    pub fn command(&self) -> String {
        render_argv("docker", &self.cli_args())
    }
}

impl ConnectOptions {
    /// Valida alias e IPs; `subnets` son las subredes de la red de destino.
    pub fn validate(&self, subnets: &[String]) -> Result<(), EngineError> {
        for a in self.aliases.iter().filter(|a| !a.trim().is_empty()) {
            if !valid_alias(a.trim()) {
                return Err(EngineError::InvalidInput {
                    message: format!("alias no válido: {a:?}"),
                });
            }
        }
        let cidrs: Vec<Cidr> = subnets.iter().filter_map(|s| Cidr::parse(s).ok()).collect();
        for (ip, want_v4, label) in [(&self.ipv4, true, "IPv4"), (&self.ipv6, false, "IPv6")] {
            let ip = ip.trim();
            if ip.is_empty() {
                continue;
            }
            let addr: IpAddr = ip.parse().map_err(|_| EngineError::InvalidInput {
                message: format!("dirección {label} no válida: {ip:?}"),
            })?;
            if addr.is_ipv4() != want_v4 {
                return Err(EngineError::InvalidInput {
                    message: format!("{ip} no es una dirección {label}"),
                });
            }
            let same_family: Vec<&Cidr> = cidrs.iter().filter(|c| c.is_v4() == want_v4).collect();
            if !same_family.is_empty() && !same_family.iter().any(|c| c.contains(addr)) {
                return Err(EngineError::InvalidInput {
                    message: format!(
                        "{ip} no está en ninguna subred de la red ({})",
                        same_family
                            .iter()
                            .map(|c| format!("{}/{}", c.addr, c.prefix))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }
        }
        Ok(())
    }

    fn aliases(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.aliases
            .iter()
            .map(|a| a.trim().to_owned())
            .filter(|a| !a.is_empty() && seen.insert(a.clone()))
            .collect()
    }

    fn endpoint(&self) -> Option<EndpointSettings> {
        let aliases = self.aliases();
        let ipam = (!self.ipv4.trim().is_empty() || !self.ipv6.trim().is_empty()).then(|| {
            EndpointIpamConfig {
                ipv4_address: non_empty(&self.ipv4),
                ipv6_address: non_empty(&self.ipv6),
                link_local_ips: None,
            }
        });
        if aliases.is_empty() && ipam.is_none() {
            return None;
        }
        Some(EndpointSettings {
            aliases: (!aliases.is_empty()).then_some(aliases),
            ipam_config: ipam,
            ..Default::default()
        })
    }

    fn cli_args(&self, network: &str, container: &str) -> Vec<String> {
        let mut a: Vec<String> = vec!["network".into(), "connect".into()];
        for al in self.aliases() {
            a.push("--alias".into());
            a.push(al);
        }
        if let Some(ip) = non_empty(&self.ipv4) {
            a.push("--ip".into());
            a.push(ip);
        }
        if let Some(ip) = non_empty(&self.ipv6) {
            a.push("--ip6".into());
            a.push(ip);
        }
        a.push(network.into());
        a.push(container.into());
        a
    }
}

// ------------------------------------------------------------------ operaciones

fn display<'a>(r: &'a str, display: Option<&'a str>) -> &'a str {
    display.filter(|d| !d.is_empty()).unwrap_or(r)
}

fn summary_from(
    id: String,
    name: String,
    n: &NetworkCommon,
    members: Vec<NetworkMember>,
) -> NetworkSummary {
    let pools = n
        .ipam
        .as_ref()
        .and_then(|i| i.config.clone())
        .unwrap_or_default();
    let labels: BTreeMap<String, String> = n.labels.clone().into_iter().collect();
    NetworkSummary {
        predefined: PREDEFINED.contains(&name.as_str())
            && matches!(
                (name.as_str(), n.driver.as_str()),
                ("bridge", "bridge") | ("host", "host") | ("none", "null")
            ),
        compose_project: labels.get("com.docker.compose.project").cloned(),
        subnets: pools.iter().filter_map(|p| p.subnet.clone()).collect(),
        gateways: pools
            .iter()
            .filter_map(|p| p.gateway.clone())
            .filter(|g| !g.is_empty())
            .collect(),
        id,
        name,
        driver: n.driver.clone(),
        scope: n.scope.clone(),
        created: n.created.clone(),
        internal: n.internal,
        attachable: n.attachable,
        ingress: n.ingress,
        enable_ipv4: n.enable_ipv4,
        enable_ipv6: n.enable_ipv6,
        labels,
        containers: members,
    }
}

/// Campos comunes de `Network` (listado) y `NetworkInspect`.
struct NetworkCommon {
    driver: String,
    scope: String,
    created: Option<String>,
    internal: bool,
    attachable: bool,
    ingress: bool,
    enable_ipv4: bool,
    enable_ipv6: bool,
    ipam: Option<Ipam>,
    labels: HashMap<String, String>,
}

/// Contenedores de la lista que tienen configurada la red `name`.
fn members_from_list(name: &str, containers: &[ContainerSummary]) -> Vec<NetworkMember> {
    let mut v: Vec<NetworkMember> = containers
        .iter()
        .filter(|c| c.networks.iter().any(|n| n == name))
        .map(|c| NetworkMember {
            id: c.id.clone(),
            name: c.name.clone(),
            state: c.state.clone(),
            ipv4: None,
            ipv6: None,
            mac: None,
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

fn validate_network_ref(r: &str) -> Result<(), EngineError> {
    validate_ref("red", r)
}

fn opt(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.is_empty())
}

impl Engine {
    /// Lista las redes y las cruza con la lista de contenedores para indicar
    /// cuáles las usan. Son dos peticiones reales y ambas quedan registradas.
    pub async fn list_networks(
        &self,
        containers: Option<&[ContainerSummary]>,
    ) -> Result<Vec<NetworkSummary>, EngineError> {
        let op = ApiOp::get(Category::Network, "network.list", "/networks")
            .equivalent(["network", "ls"]);
        let d = self.docker().clone();
        let list = self
            .call(
                op,
                async move {
                    d.list_networks(None::<bollard::query_parameters::ListNetworksOptions>)
                        .await
                },
                |v| Some(format!("{} redes", v.len())),
            )
            .await?;
        let owned;
        let containers = match containers {
            Some(c) => c,
            None => {
                owned = self.list_containers().await?;
                &owned
            }
        };
        let mut v: Vec<NetworkSummary> = list
            .into_iter()
            .map(|n| {
                let name = n.name.clone().unwrap_or_default();
                let common = NetworkCommon {
                    driver: n.driver.unwrap_or_default(),
                    scope: n.scope.unwrap_or_default(),
                    created: n.created,
                    internal: n.internal.unwrap_or(false),
                    attachable: n.attachable.unwrap_or(false),
                    ingress: n.ingress.unwrap_or(false),
                    enable_ipv4: n.enable_ipv4.unwrap_or(true),
                    enable_ipv6: n.enable_ipv6.unwrap_or(false),
                    ipam: n.ipam,
                    labels: n.labels.unwrap_or_default(),
                };
                let members = members_from_list(&name, containers);
                summary_from(n.id.unwrap_or_default(), name, &common, members)
            })
            .collect();
        v.sort_by(|a, b| (!a.predefined, &a.name).cmp(&(!b.predefined, &b.name)));
        Ok(v)
    }

    /// `docker network inspect`, completado con los contenedores detenidos que
    /// tienen la red configurada (Docker solo incluye los que están en marcha).
    pub async fn network_details(
        &self,
        id: &str,
        display_name: Option<&str>,
    ) -> Result<NetworkDetails, EngineError> {
        let r = id.trim();
        validate_network_ref(r)?;
        let shown = display(r, display_name).to_owned();
        let op = ApiOp {
            category: Category::Network,
            kind: "network.inspect",
            target: Some(shown.clone()),
            method: "GET",
            path: format!("/networks/{r}"),
            body: None,
            equivalent: Some(vec!["network".into(), "inspect".into(), shown]),
            timeout: None,
        };
        let d = self.docker().clone();
        let key = r.to_owned();
        let n = self
            .call(
                op,
                async move {
                    d.inspect_network(
                        &key,
                        None::<bollard::query_parameters::InspectNetworkOptions>,
                    )
                    .await
                },
                |n| {
                    Some(format!(
                        "{} contenedores en marcha conectados",
                        n.containers.as_ref().map_or(0, |c| c.len())
                    ))
                },
            )
            .await?;
        let raw = serde_json::to_string_pretty(&n).unwrap_or_default();
        let name = n.name.clone().unwrap_or_default();
        let containers = self.list_containers().await?;
        let mut members = members_from_list(&name, &containers);
        let by_id: HashMap<&str, &ContainerSummary> =
            containers.iter().map(|c| (c.id.as_str(), c)).collect();
        for (cid, e) in n.containers.clone().unwrap_or_default() {
            let m = NetworkMember {
                id: cid.clone(),
                name: e.name.clone().unwrap_or_default(),
                state: by_id
                    .get(cid.as_str())
                    .map(|c| c.state.clone())
                    .unwrap_or_else(|| "running".into()),
                ipv4: opt(e.ipv4_address),
                ipv6: opt(e.ipv6_address),
                mac: opt(e.mac_address),
            };
            match members.iter_mut().find(|x| x.id == cid) {
                Some(x) => *x = m,
                None => members.push(m),
            }
        }
        members.sort_by(|a, b| a.name.cmp(&b.name));
        let ipam = n.ipam.clone();
        let common = NetworkCommon {
            driver: n.driver.clone().unwrap_or_default(),
            scope: n.scope.clone().unwrap_or_default(),
            created: n.created.clone(),
            internal: n.internal.unwrap_or(false),
            attachable: n.attachable.unwrap_or(false),
            ingress: n.ingress.unwrap_or(false),
            enable_ipv4: n.enable_ipv4.unwrap_or(true),
            enable_ipv6: n.enable_ipv6.unwrap_or(false),
            ipam: ipam.clone(),
            labels: n.labels.clone().unwrap_or_default(),
        };
        let summary = summary_from(n.id.clone().unwrap_or_default(), name, &common, members);
        Ok(NetworkDetails {
            summary,
            ipam_driver: ipam
                .as_ref()
                .and_then(|i| i.driver.clone())
                .unwrap_or_default(),
            pools: ipam
                .and_then(|i| i.config)
                .unwrap_or_default()
                .into_iter()
                .map(|p| IpamPoolInfo {
                    subnet: p.subnet.unwrap_or_default(),
                    gateway: p.gateway.unwrap_or_default(),
                    ip_range: p.ip_range.unwrap_or_default(),
                    auxiliary: p
                        .auxiliary_addresses
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                })
                .collect(),
            options: n.options.unwrap_or_default().into_iter().collect(),
            raw,
        })
    }

    /// `docker network create`. Valida antes de enviar nada.
    pub async fn create_network(&self, spec: &NetworkSpec) -> Result<CreatedNetwork, EngineError> {
        let issues = spec.validate();
        if NetworkSpec::has_errors(&issues) {
            return Err(EngineError::InvalidSpec { issues });
        }
        let body = spec.wire_body();
        let name = spec.name.trim().to_owned();
        let op = ApiOp {
            category: Category::Network,
            kind: "network.create",
            target: Some(name),
            method: "POST",
            path: "/networks/create".into(),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(spec.cli_args()),
            timeout: Some(std::time::Duration::from_secs(120)),
        };
        let dialer = self.dialer();
        let json = serde_json::to_vec(&body).unwrap_or_default();
        let r = self
            .call(
                op,
                async move { http::post_json(&dialer, "/networks/create", json).await },
                |r| {
                    Some(format!(
                        "Red creada: {}",
                        r["Id"]
                            .as_str()
                            .unwrap_or_default()
                            .chars()
                            .take(12)
                            .collect::<String>()
                    ))
                },
            )
            .await?;
        let id = r["Id"].as_str().unwrap_or_default().to_owned();
        if id.is_empty() {
            return Err(EngineError::InvalidResponse {
                message: format!("Docker no devolvió el ID de la red: {r}"),
            });
        }
        Ok(CreatedNetwork {
            id,
            warning: r["Warning"]
                .as_str()
                .filter(|w| !w.is_empty())
                .map(str::to_owned),
        })
    }

    /// `docker network rm`. Docker rechaza (403) la eliminación si hay
    /// contenedores en marcha conectados; si solo la tienen configurada
    /// contenedores detenidos, la elimina y esos contenedores no podrán
    /// arrancar (la interfaz avisa antes).
    pub async fn remove_network(
        &self,
        id: &str,
        display_name: Option<&str>,
        confirmed: bool,
    ) -> Result<(), EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        let r = id.trim();
        validate_network_ref(r)?;
        let _guard = self.locks().acquire(&format!("network:{r}"))?;
        let shown = display(r, display_name).to_owned();
        let op = ApiOp {
            category: Category::Network,
            kind: "network.remove",
            target: Some(shown.clone()),
            method: "DELETE",
            path: format!("/networks/{r}"),
            body: None,
            equivalent: Some(vec!["network".into(), "rm".into(), shown.clone()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let key = r.to_owned();
        self.call(op, async move { d.remove_network(&key).await }, move |_| {
            Some(format!("Red eliminada: {shown}"))
        })
        .await
    }

    /// `docker network prune --force`: elimina las redes creadas por el usuario
    /// que no usa ningún contenedor (en marcha o no). `label` limita a las que
    /// tienen esa etiqueta (`clave=valor`); las pruebas lo usan.
    pub async fn prune_networks(
        &self,
        label: Option<&str>,
        confirmed: bool,
    ) -> Result<Vec<String>, EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        let mut eq = vec![
            "network".to_owned(),
            "prune".to_owned(),
            "--force".to_owned(),
        ];
        let mut filters = HashMap::new();
        if let Some(l) = label {
            if l.is_empty() || l.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(EngineError::InvalidInput {
                    message: format!("filtro de etiqueta no válido: {l:?}"),
                });
            }
            filters.insert("label".to_owned(), vec![l.to_owned()]);
            eq.push("--filter".into());
            eq.push(format!("label={l}"));
        }
        let opts = bollard::query_parameters::PruneNetworksOptions {
            filters: (!filters.is_empty()).then_some(filters),
        };
        let op = ApiOp {
            category: Category::Network,
            kind: "network.prune",
            target: None,
            method: "POST",
            path: path_with_query("/networks/prune", &opts),
            body: None,
            equivalent: Some(eq),
            timeout: None,
        };
        let d = self.docker().clone();
        let r = self
            .call(op, async move { d.prune_networks(Some(opts)).await }, |r| {
                Some(format!(
                    "{} redes eliminadas",
                    r.networks_deleted.as_ref().map_or(0, |v| v.len())
                ))
            })
            .await?;
        Ok(r.networks_deleted.unwrap_or_default())
    }

    /// `docker network connect [--alias A] [--ip IP] [--ip6 IP] RED CONTENEDOR`.
    /// `subnets` son las subredes de la red, para validar la IP fija.
    #[allow(clippy::too_many_arguments)]
    pub async fn connect_network(
        &self,
        network: &str,
        network_name: Option<&str>,
        container: &str,
        container_name: Option<&str>,
        options: &ConnectOptions,
        subnets: &[String],
    ) -> Result<(), EngineError> {
        let (n, c) = (network.trim(), container.trim());
        validate_network_ref(n)?;
        validate_ref("contenedor", c)?;
        options.validate(subnets)?;
        let _guard = self.locks().acquire(&format!("container:{c}"))?;
        let (nshown, cshown) = (display(n, network_name), display(c, container_name));
        let body = NetworkConnectRequest {
            container: c.to_owned(),
            endpoint_config: options.endpoint(),
        };
        let op = ApiOp {
            category: Category::Network,
            kind: "network.connect",
            target: Some(format!("{cshown} → {nshown}")),
            method: "POST",
            path: format!("/networks/{n}/connect"),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(options.cli_args(nshown, cshown)),
            timeout: None,
        };
        let d = self.docker().clone();
        let key = n.to_owned();
        let summary = format!("{cshown} conectado a {nshown}");
        self.call(
            op,
            async move { d.connect_network(&key, body).await },
            move |_| Some(summary),
        )
        .await
    }

    /// `docker network disconnect [--force] RED CONTENEDOR`. Corta la
    /// conectividad del contenedor por esa red: requiere confirmación.
    #[allow(clippy::too_many_arguments)]
    pub async fn disconnect_network(
        &self,
        network: &str,
        network_name: Option<&str>,
        container: &str,
        container_name: Option<&str>,
        force: bool,
        confirmed: bool,
    ) -> Result<(), EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        let (n, c) = (network.trim(), container.trim());
        validate_network_ref(n)?;
        validate_ref("contenedor", c)?;
        let _guard = self.locks().acquire(&format!("container:{c}"))?;
        let (nshown, cshown) = (display(n, network_name), display(c, container_name));
        let body = NetworkDisconnectRequest {
            container: c.to_owned(),
            force: force.then_some(true),
        };
        let mut eq = vec!["network".to_owned(), "disconnect".to_owned()];
        if force {
            eq.push("--force".into());
        }
        eq.push(nshown.into());
        eq.push(cshown.into());
        let op = ApiOp {
            category: Category::Network,
            kind: "network.disconnect",
            target: Some(format!("{cshown} → {nshown}")),
            method: "POST",
            path: format!("/networks/{n}/disconnect"),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(eq),
            timeout: None,
        };
        let d = self.docker().clone();
        let key = n.to_owned();
        let summary = format!("{cshown} desconectado de {nshown}");
        self.call(
            op,
            async move { d.disconnect_network(&key, body).await },
            move |_| Some(summary),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(s: &NetworkSpec) -> Vec<String> {
        s.validate().into_iter().map(|i| i.code).collect()
    }

    fn spec(name: &str) -> NetworkSpec {
        NetworkSpec {
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn cidr_parsing_and_containment() {
        let c = Cidr::parse("10.231.0.0/24").unwrap();
        assert!(c.is_v4());
        assert!(c.contains("10.231.0.50".parse().unwrap()));
        assert!(!c.contains("10.231.1.1".parse().unwrap()));
        assert!(!c.contains("fd00::1".parse().unwrap()));
        assert!(c.contains_cidr(&Cidr::parse("10.231.0.128/25").unwrap()));
        assert!(!c.contains_cidr(&Cidr::parse("10.231.0.0/23").unwrap()));
        assert!(!c.contains_cidr(&Cidr::parse("10.244.0.0/25").unwrap()));
        // Mismo mensaje que Docker: "it should be 10.241.0.0/24".
        assert_eq!(
            Cidr::parse("10.241.0.5/24"),
            Err(CidrError::HostBits("10.241.0.0/24".into()))
        );
        let v6 = Cidr::parse("fd00:dcc::/64").unwrap();
        assert!(!v6.is_v4());
        assert!(v6.contains("fd00:dcc::5".parse().unwrap()));
        assert_eq!(
            Cidr::parse("fd00:dcc::1/64"),
            Err(CidrError::HostBits("fd00:dcc::/64".into()))
        );
        assert!(Cidr::parse("0.0.0.0/0").is_ok());
        assert!(Cidr::parse("::/0").is_ok());
        assert!(Cidr::parse("10.0.0.1/32").is_ok());
        for bad in [
            "10.0.0.0",
            "10.0.0.0/33",
            "10.0.0/24",
            "x/24",
            "10.0.0.0/-1",
            "10.0.0.0/+8",
            "10.0.0.0/08",
            "fd00::/129",
            "",
            "/24",
        ] {
            assert_eq!(Cidr::parse(bad), Err(CidrError::Invalid), "{bad}");
        }
    }

    #[test]
    fn name_and_driver_validation() {
        assert!(codes(&spec("app_net-1.prod")).is_empty());
        assert_eq!(codes(&spec("")), ["name.required"]);
        assert_eq!(codes(&spec("  ")), ["name.required"]);
        for bad in ["mi red", "-x", ".x", "a/b", "ñ", "a?b"] {
            assert_eq!(codes(&spec(bad)), ["name.invalid"], "{bad}");
        }
        for r in PREDEFINED {
            assert_eq!(codes(&spec(r)), ["name.reserved"]);
        }
        let mut s = spec("n");
        s.driver = "".into();
        assert_eq!(codes(&s), ["driver.invalid"]);
        s.driver = "mi driver".into();
        assert_eq!(codes(&s), ["driver.invalid"]);
        s.driver = "host".into();
        assert_eq!(codes(&s), ["driver.singleton"]);
        s.driver = "null".into();
        assert_eq!(codes(&s), ["driver.singleton"]);
        s.driver = "overlay".into();
        assert_eq!(codes(&s), ["driver.swarm"]);
        assert!(!NetworkSpec::has_errors(&s.validate()));
        s.driver = "macvlan".into();
        assert_eq!(codes(&s), ["driver.noParent"]);
        s.options = vec![KeyValue::new("parent", "eth0")];
        assert!(codes(&s).is_empty());
        s.driver = "vieux/sshfs:latest".into();
        s.options.clear();
        assert!(codes(&s).is_empty());
    }

    #[test]
    fn ipam_validation_matches_docker_rules() {
        let mut s = spec("n");
        s.ipv4 = IpamPool {
            subnet: "10.240.0.0/24".into(),
            gateway: "10.240.0.1".into(),
            ip_range: "10.240.0.128/25".into(),
        };
        assert!(codes(&s).is_empty());
        s.ipv4.subnet = "10.241.0.5/24".into();
        let i = s.validate();
        assert_eq!(i[0].code, "subnet.hostBits");
        assert_eq!(i[0].params["fixed"], "10.241.0.0/24");
        s.ipv4 = IpamPool {
            subnet: "10.232.0.0/24".into(),
            gateway: "10.233.0.1".into(),
            ip_range: "10.244.0.0/25".into(),
        };
        assert_eq!(codes(&s), ["gateway.outside", "ipRange.outside"]);
        s.ipv4 = IpamPool {
            subnet: "".into(),
            gateway: "10.242.0.1".into(),
            ip_range: "".into(),
        };
        assert_eq!(codes(&s), ["subnet.required"]);
        s.ipv4 = IpamPool {
            subnet: "fd00::/64".into(),
            gateway: "x".into(),
            ip_range: "10.0.0.0/8".into(),
        };
        assert_eq!(codes(&s), ["subnet.family", "gateway.invalid"]);
        s.ipv4 = IpamPool {
            subnet: "10.0.0.0/8".into(),
            gateway: "fd00::1".into(),
            ip_range: "fd00::/64".into(),
        };
        assert_eq!(codes(&s), ["gateway.family", "ipRange.family"]);
        s.ipv4 = IpamPool::default();
        s.ipv6.subnet = "fd00:dcc::/64".into();
        assert_eq!(codes(&s), ["ipv6.disabled"]);
        s.enable_ipv6 = true;
        assert!(codes(&s).is_empty());
        s.ipv6.subnet = "10.0.0.0/8".into();
        assert_eq!(codes(&s), ["subnet.family"]);
        assert_eq!(s.validate()[0].field, "ipv6.subnet");
    }

    #[test]
    fn labels_and_options_validation() {
        let mut s = spec("n");
        s.labels = vec![
            KeyValue::new("app", "web"),
            KeyValue::new("", ""),
            KeyValue::new("app", "otra"),
            KeyValue::new("a b", "x"),
            KeyValue::new("", "huérfano"),
        ];
        s.options = vec![
            KeyValue::new("com.docker.network.bridge.name", "br-x"),
            KeyValue::new("k=v", ""),
        ];
        let i = s.validate();
        let got: Vec<(String, String)> = i
            .iter()
            .map(|i| (i.field.clone(), i.code.clone()))
            .collect();
        assert_eq!(
            got,
            [
                ("labels.2.key".into(), "label.duplicate".into()),
                ("labels.3.key".into(), "label.keyInvalid".into()),
                ("labels.4.key".into(), "label.keyInvalid".into()),
                ("options.1.key".into(), "option.keyInvalid".into()),
            ]
        );
    }

    #[test]
    fn body_and_command_are_minimal_and_consistent() {
        let s = spec("dcc-net");
        assert_eq!(
            serde_json::to_value(s.to_create_body()).unwrap(),
            serde_json::json!({ "Name": "dcc-net", "Driver": "bridge" })
        );
        assert_eq!(s.command(), "docker network create --driver bridge dcc-net");
        assert_eq!(
            s.wire_body(),
            serde_json::json!({ "Name": "dcc-net", "Driver": "bridge", "CheckDuplicate": true })
        );

        let s = NetworkSpec {
            name: " dcc-net ".into(),
            driver: "bridge".into(),
            internal: true,
            attachable: true,
            enable_ipv6: true,
            ipv4: IpamPool {
                subnet: "10.240.0.0/24".into(),
                gateway: "10.240.0.1".into(),
                ip_range: "10.240.0.128/25".into(),
            },
            ipv6: IpamPool {
                subnet: "fd00:dcc::/64".into(),
                ..Default::default()
            },
            labels: vec![KeyValue::new("z", "1"), KeyValue::new("app", "mi web")],
            options: vec![KeyValue::new("com.docker.network.bridge.name", "br-dcc")],
        };
        assert!(s.validate().is_empty());
        assert_eq!(
            serde_json::to_value(s.to_create_body()).unwrap(),
            serde_json::json!({
                "Name": "dcc-net",
                "Driver": "bridge",
                "Internal": true,
                "Attachable": true,
                "EnableIPv6": true,
                "IPAM": {
                    "Driver": "default",
                    "Config": [
                        { "Subnet": "10.240.0.0/24", "IPRange": "10.240.0.128/25", "Gateway": "10.240.0.1" },
                        { "Subnet": "fd00:dcc::/64" }
                    ]
                },
                "Options": { "com.docker.network.bridge.name": "br-dcc" },
                "Labels": { "z": "1", "app": "mi web" }
            })
        );
        assert_eq!(
            s.command(),
            "docker network create --driver bridge --subnet 10.240.0.0/24 --ip-range 10.240.0.128/25 \
             --gateway 10.240.0.1 --subnet fd00:dcc::/64 --label 'app=mi web' --label z=1 \
             --opt com.docker.network.bridge.name=br-dcc --ipv6 --internal --attachable dcc-net"
        );
        // Sin IPv6 activado, la subred IPv6 no se envía (además es un error).
        let mut t = s.clone();
        t.enable_ipv6 = false;
        let body = serde_json::to_value(t.to_create_body()).unwrap();
        assert_eq!(body["IPAM"]["Config"].as_array().unwrap().len(), 1);
        assert!(body.get("EnableIPv6").is_none());
        assert!(!t.command().contains("fd00"));
    }

    #[test]
    fn connect_options() {
        let subnets = vec!["10.231.0.0/24".to_owned()];
        let o = ConnectOptions {
            aliases: vec![" web ".into(), "api".into(), "web".into(), "".into()],
            ipv4: "10.231.0.50".into(),
            ipv6: "".into(),
        };
        assert!(o.validate(&subnets).is_ok());
        assert_eq!(
            serde_json::to_value(o.endpoint()).unwrap(),
            serde_json::json!({ "IPAMConfig": { "IPv4Address": "10.231.0.50" }, "Aliases": ["web", "api"] })
        );
        assert_eq!(
            render_argv("docker", &o.cli_args("dcc-net", "web-1")),
            "docker network connect --alias web --alias api --ip 10.231.0.50 dcc-net web-1"
        );
        assert_eq!(ConnectOptions::default().endpoint(), None);
        let bad = |o: ConnectOptions| o.validate(&subnets).unwrap_err().to_string();
        assert!(bad(ConnectOptions {
            aliases: vec!["mi web".into()],
            ..Default::default()
        })
        .contains("alias"));
        assert!(bad(ConnectOptions {
            ipv4: "10.99.0.5".into(),
            ..Default::default()
        })
        .contains("no está en ninguna subred de la red (10.231.0.0/24)"));
        assert!(bad(ConnectOptions {
            ipv4: "fd00::1".into(),
            ..Default::default()
        })
        .contains("no es una dirección IPv4"));
        assert!(bad(ConnectOptions {
            ipv6: "10.0.0.1".into(),
            ..Default::default()
        })
        .contains("no es una dirección IPv6"));
        assert!(bad(ConnectOptions {
            ipv4: "10.0.0".into(),
            ..Default::default()
        })
        .contains("no válida"));
        // Sin subredes conocidas de esa familia, decide Docker.
        assert!(ConnectOptions {
            ipv6: "fd00::9".into(),
            ..Default::default()
        }
        .validate(&subnets)
        .is_ok());
    }

    #[test]
    fn members_come_from_the_container_list_by_network_name() {
        let c = |id: &str, name: &str, state: &str, nets: &[&str]| ContainerSummary {
            id: id.into(),
            name: name.into(),
            image: String::new(),
            image_id: String::new(),
            command: String::new(),
            created: 0,
            state: state.into(),
            status: String::new(),
            ports: vec![],
            networks: nets.iter().map(|s| s.to_string()).collect(),
            mounts: 0,
            mount_points: vec![],
            compose_project: None,
            labels: Default::default(),
        };
        let list = [
            c("2", "web", "running", &["front", "bridge"]),
            c("1", "db", "exited", &["front"]),
            c("3", "x", "running", &["bridge"]),
        ];
        let m = members_from_list("front", &list);
        assert_eq!(
            m.iter()
                .map(|m| (m.name.as_str(), m.state.as_str()))
                .collect::<Vec<_>>(),
            [("db", "exited"), ("web", "running")]
        );
        assert!(members_from_list("nada", &list).is_empty());
    }
}
